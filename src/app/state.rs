//! Состояние приложения и жизненный цикл [`eframe::App`].
//!
//! Здесь хранится всё, что переживает отдельный кадр отрисовки: разобранное
//! JSON-дерево, состояние поиска, метаданные файла и настройки темы.

use std::path::PathBuf;
use std::time::Instant;

use crate::parser::{DataFormat, JsonNode, ParseError, parse_data, serialize_data};
use crate::search::SearchState;

use super::edit::{add_child_at_path, node_to_value};
use super::tree::AddChildRequest;

/// Метаданные загруженного файла, отображаемые в статус-баре.
#[derive(Debug, Default)]
pub(super) struct FileState {
    /// Путь к файлу на диске.
    pub(super) path: Option<PathBuf>,
    /// Размер файла в байтах.
    pub(super) size_bytes: u64,
    /// Время загрузки и разбора файла в миллисекундах.
    pub(super) load_time_ms: u128,
    /// Формат открытого файла.
    pub(super) format: Option<DataFormat>,
}

/// Режим работы приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum AppMode {
    /// Только просмотр данных без изменения значений.
    #[default]
    View,
    /// Разрешено редактирование примитивных значений JSON.
    Edit,
}

/// Состояние диалога добавления поля объекта или элемента массива.
#[derive(Debug)]
pub(super) struct AddChildDialog {
    parent_path: String,
    is_object: bool,
    key: String,
    value: String,
    error: Option<String>,
}

impl From<AddChildRequest> for AddChildDialog {
    fn from(request: AddChildRequest) -> Self {
        Self {
            parent_path: request.parent_path,
            is_object: request.is_object,
            key: String::new(),
            value: String::new(),
            error: None,
        }
    }
}

/// Основное состояние приложения JSON Viewer.
///
/// Хранит разобранное JSON-дерево, параметры поиска, информацию о файле
/// и временные сообщения для пользователя (уведомления, ошибки).
pub struct JsonViewerApp {
    /// Корневой узел разобранного JSON-дерева. `None` если файл ещё не загружен.
    pub(super) root: Option<JsonNode>,
    /// Ошибка последнего парсинга. `None` если файл разобран успешно.
    pub(super) parse_error: Option<ParseError>,
    /// Состояние поиска.
    pub(super) search: SearchState,
    /// Буфер для строки поиска в UI.
    pub(super) search_query_buf: String,
    /// Мета-информация о загруженном файле.
    pub(super) file_state: FileState,
    /// Временное уведомление (например, «Скопировано») и момент его показа.
    pub(super) toast: Option<(String, Instant)>,
    /// Флаг тёмной темы.
    pub(super) dark_mode: bool,
    /// Текущий режим работы приложения.
    pub(super) mode: AppMode,
    /// Открытый диалог добавления поля или элемента.
    pub(super) add_child_dialog: Option<AddChildDialog>,
}

impl Default for JsonViewerApp {
    fn default() -> Self {
        Self {
            root: None,
            parse_error: None,
            search: SearchState::default(),
            search_query_buf: String::new(),
            file_state: FileState::default(),
            toast: None,
            dark_mode: true,
            mode: AppMode::default(),
            add_child_dialog: None,
        }
    }
}

impl JsonViewerApp {
    /// Создать новый экземпляр приложения.
    ///
    /// Инициализирует тему оформления на основе системных предпочтений,
    /// если они доступны через [`eframe::CreationContext`].
    ///
    /// # Arguments
    ///
    /// * `cc` — контекст создания `eframe`, предоставляющий доступ к [`egui::Context`].
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // В eframe 0.35 IntegrationInfo больше не содержит system_theme;
        // читаем начальную тему из текущих визуальных настроек egui
        // (egui сам определяет системную тему при инициализации).
        let dark_mode = cc.egui_ctx.theme() == egui::Theme::Dark;

        let app = Self {
            dark_mode,
            ..Default::default()
        };
        app.apply_theme(&cc.egui_ctx);
        app
    }

    /// Создать приложение и сразу загрузить файл, переданный через командную строку.
    ///
    /// # Arguments
    ///
    /// * `cc` — контекст создания `eframe`.
    /// * `path` — путь к JSON-файлу; `None` — стартовать с пустым состоянием.
    pub fn new_with_file(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut app = Self::new(cc);
        if let Some(path) = path {
            app.load_file(path);
        }
        app
    }

    /// Применить текущую тему оформления к контексту egui.
    pub(super) fn apply_theme(&self, ctx: &egui::Context) {
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
    }

    /// Загрузить файл поддерживаемого формата по указанному пути.
    ///
    /// Читает файл, измеряет время парсинга и сохраняет результат
    /// (корневой узел или ошибку) в состоянии приложения.
    ///
    /// # Errors
    ///
    /// Ошибки чтения файла и парсинга записываются в `self.parse_error`;
    /// метод не возвращает `Result` — ошибки отображаются в UI.
    pub(super) fn load_file(&mut self, path: PathBuf) {
        let t0 = Instant::now();
        match std::fs::read_to_string(&path) {
            Err(e) => {
                self.parse_error = Some(ParseError {
                    message: format!("Ошибка чтения файла: {}", e),
                    line: None,
                    column: None,
                });
                self.root = None;
            }
            Ok(content) => {
                let size = content.len() as u64;
                let format_hint = DataFormat::from_path(&path);
                match parse_data(&content, format_hint) {
                    Ok((node, format)) => {
                        self.root = Some(node);
                        self.parse_error = None;
                        self.file_state = FileState {
                            path: Some(path),
                            size_bytes: size,
                            load_time_ms: t0.elapsed().as_millis(),
                            format: Some(format),
                        };
                        // Сбрасываем поиск при загрузке нового файла
                        self.search = SearchState::default();
                        self.search_query_buf.clear();
                    }
                    Err(e) => {
                        self.parse_error = Some(e);
                        self.root = None;
                    }
                }
            }
        }
    }

    /// Открыть системный диалог выбора и загрузить выбранный файл.
    pub(super) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_file(path);
        }
    }

    /// Сохранить текущие данные в форматированном виде.
    ///
    /// Формат результата определяется по расширению выбранного файла.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    pub(super) fn save_pretty(&mut self) {
        let Some(root) = self.root.as_ref() else {
            return;
        };

        match node_to_value(root) {
            Ok(value) => {
                let current_format = self.file_state.format.unwrap_or(DataFormat::Json);
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
                    .add_filter("JSON", &["json", "json5"])
                    .add_filter("YAML", &["yaml", "yml"])
                    .add_filter("TOML", &["toml"]);
                if let Some(name) = self
                    .file_state
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                {
                    dialog = dialog.set_file_name(name);
                }
                if let Some(save_path) = dialog.save_file() {
                    let format = DataFormat::from_path(&save_path).unwrap_or(current_format);
                    match serialize_data(&value, format, false).and_then(|formatted| {
                        std::fs::write(&save_path, formatted)
                            .map_err(|error| format!("Ошибка сохранения: {}", error))
                    }) {
                        Ok(_) => self.show_toast("Файл сохранён"),
                        Err(error) => self.show_toast(&error),
                    }
                }
            }
            Err(err) => self.show_toast(&err),
        }
    }

    /// Показать кратковременное уведомление в статус-баре.
    pub(super) fn show_toast(&mut self, message: &str) {
        self.toast = Some((message.to_string(), Instant::now()));
    }

    /// Пересчитать результаты поиска по текущему запросу.
    ///
    /// Вызывается после правки дерева, чтобы подсветка оставалась актуальной.
    pub(super) fn refresh_search(&mut self) {
        if self.search_query_buf.is_empty() {
            return;
        }
        if let Some(root) = &self.root {
            let query = self.search_query_buf.clone();
            self.search.search(root, &query);
        }
    }

    /// Открыть диалог добавления данных в выбранный контейнер.
    pub(super) fn open_add_child_dialog(&mut self, request: AddChildRequest) {
        self.add_child_dialog = Some(request.into());
    }

    /// Отрисовать диалог и добавить новый узел после подтверждения.
    pub(super) fn show_add_child_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.add_child_dialog.take() else {
            return;
        };

        let mut submit = false;
        let mut cancel = false;
        let title = if dialog.is_object {
            "Добавить поле"
        } else {
            "Добавить элемент"
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                if dialog.is_object {
                    ui.label("Имя поля");
                    ui.add(egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0));
                }
                ui.label("Значение");
                ui.add(
                    egui::TextEdit::multiline(&mut dialog.value)
                        .desired_width(420.0)
                        .desired_rows(5)
                        .font(egui::TextStyle::Monospace),
                );
                if let Some(error) = &dialog.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui.button("Добавить").clicked() {
                        submit = true;
                    }
                    if ui.button("Отмена").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            return;
        }
        if !submit {
            self.add_child_dialog = Some(dialog);
            return;
        }

        let format = self.file_state.format.unwrap_or(DataFormat::Json);
        let result = self
            .root
            .as_mut()
            .ok_or_else(|| "Нет открытого документа".to_string())
            .and_then(|root| {
                add_child_at_path(
                    root,
                    &dialog.parent_path,
                    &dialog.key,
                    &dialog.value,
                    format,
                )
            });

        match result {
            Ok(()) => {
                self.refresh_search();
                self.show_toast("Данные добавлены");
            }
            Err(error) => {
                dialog.error = Some(error);
                self.add_child_dialog = Some(dialog);
            }
        }
    }
}

impl eframe::App for JsonViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Принудительное обновление, если показано уведомление (чтобы оно исчезло вовремя)
        if self.toast.is_some() {
            ui.ctx().request_repaint();
        }

        self.show_top_panel(ui);
        self.show_bottom_panel(ui);
        self.show_central_panel(ui);
    }
}
