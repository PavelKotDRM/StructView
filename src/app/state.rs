//! Состояние приложения и жизненный цикл [`eframe::App`].
//!
//! Здесь хранится всё, что переживает отдельный кадр отрисовки: разобранное
//! JSON-дерево, состояние поиска, метаданные файла и настройки темы.

use std::path::PathBuf;
use std::time::Instant;

use crate::parser::{JsonNode, ParseError, parse_json};
use crate::search::SearchState;

use super::edit::node_to_value;

/// Метаданные загруженного файла, отображаемые в статус-баре.
#[derive(Debug, Default)]
pub(super) struct FileState {
    /// Путь к файлу на диске.
    pub(super) path: Option<PathBuf>,
    /// Размер файла в байтах.
    pub(super) size_bytes: u64,
    /// Время загрузки и разбора файла в миллисекундах.
    pub(super) load_time_ms: u128,
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

    /// Загрузить JSON-файл по указанному пути.
    ///
    /// Читает файл, измеряет время парсинга и сохраняет результат
    /// (корневой узел или ошибку) в состоянии приложения.
    ///
    /// # Errors
    ///
    /// Ошибки чтения файла и парсинга JSON записываются в `self.parse_error`;
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
                match parse_json(&content) {
                    Ok(node) => {
                        self.root = Some(node);
                        self.parse_error = None;
                        self.file_state = FileState {
                            path: Some(path),
                            size_bytes: size,
                            load_time_ms: t0.elapsed().as_millis(),
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

    /// Открыть системный диалог выбора файла и загрузить выбранный JSON.
    pub(super) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON files", &["json"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_file(path);
        }
    }

    /// Сохранить текущий JSON в форматированном виде (pretty-print).
    ///
    /// Открывает диалог сохранения и записывает JSON с отступами 2 пробела.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    pub(super) fn save_pretty(&mut self) {
        let Some(root) = self.root.as_ref() else {
            return;
        };

        match node_to_value(root).and_then(|value| {
            serde_json::to_string_pretty(&value).map_err(|e| format!("Ошибка сериализации: {}", e))
        }) {
            Ok(formatted) => {
                if let Some(save_path) = rfd::FileDialog::new()
                    .add_filter("JSON files", &["json"])
                    .save_file()
                {
                    match std::fs::write(&save_path, formatted) {
                        Ok(_) => self.show_toast("Файл сохранён"),
                        Err(e) => self.show_toast(&format!("Ошибка сохранения: {}", e)),
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
