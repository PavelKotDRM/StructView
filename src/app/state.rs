//! Состояние приложения и жизненный цикл [`eframe::App`].
//!
//! Здесь хранится всё, что переживает отдельный кадр отрисовки: разобранное
//! JSON-дерево, состояние поиска, метаданные файла и настройки темы.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::clipboard::{
    ClipboardEntry, copy_to_clipboard, decode_structures, encode_structures, read_from_clipboard,
};
use crate::diff::{Difference, compare_values};
use crate::parser::{DataFormat, JsonNode, JsonValueType, ParseError, parse_data, serialize_data};
use crate::search::SearchState;

use super::edit::{
    add_typed_child_at_path, edit_child_at_path, find_node, is_object_child, node_to_value,
    paste_structures_at_path, selected_structures,
};
use super::i18n::{Locale, TextKey};
use super::tree::{AddChildRequest, EditFieldRequest, SelectionRequest, VisibleRows};

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

/// Документ, загруженный в режим сравнения.
#[derive(Debug)]
pub(super) struct ComparisonDocument {
    /// Путь к файлу.
    pub(super) path: PathBuf,
    /// Размер файла в байтах.
    pub(super) size_bytes: u64,
    /// Время загрузки и разбора файла в миллисекундах.
    pub(super) load_time_ms: u128,
    /// Формат файла.
    pub(super) format: DataFormat,
}

/// Состояние отображения отличий нескольких документов.
#[derive(Debug)]
pub(super) struct ComparisonState {
    /// Загруженные документы в порядке колонок таблицы.
    pub(super) documents: Vec<ComparisonDocument>,
    /// Отличия между значениями документов.
    pub(super) differences: Vec<Difference>,
}

/// Режим работы приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum AppMode {
    /// Только просмотр данных без изменения значений.
    #[default]
    View,
    /// Разрешено редактирование значений JSON.
    Edit,
}

/// Цель конструктора поля.
#[derive(Debug, Clone)]
pub(super) enum FieldDialogTarget {
    /// Добавление поля или элемента в контейнер.
    Add {
        parent_path: String,
        is_object: bool,
    },
    /// Редактирование существующего узла.
    Edit { path: String, key_editable: bool },
}

/// Состояние конструктора поля объекта или элемента массива.
#[derive(Debug)]
pub(super) struct FieldDialog {
    target: FieldDialogTarget,
    key: String,
    value_type: JsonValueType,
    value: String,
    error: Option<String>,
}

impl From<AddChildRequest> for FieldDialog {
    fn from(request: AddChildRequest) -> Self {
        Self {
            target: FieldDialogTarget::Add {
                parent_path: request.parent_path,
                is_object: request.is_object,
            },
            key: String::new(),
            value_type: JsonValueType::String,
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
    /// Путь совпадения, к которому нужно прокрутить дерево в следующем кадре.
    pub(super) search_scroll_target: Option<String>,
    /// Отложенный запрос на сохранение текущего файла.
    pub(super) save_requested: bool,
    /// Мета-информация о загруженном файле.
    pub(super) file_state: FileState,
    /// Состояние сравнения нескольких файлов.
    pub(super) comparison: Option<ComparisonState>,
    /// Временное уведомление (например, «Скопировано») и момент его показа.
    pub(super) toast: Option<(String, Instant)>,
    /// Флаг тёмной темы.
    pub(super) dark_mode: bool,
    /// Текущий режим работы приложения.
    pub(super) mode: AppMode,
    /// Текущий язык интерфейса.
    pub(super) locale: Locale,
    /// Открытый конструктор добавления или редактирования поля.
    pub(super) field_dialog: Option<FieldDialog>,
    /// Пути выбранных узлов дерева.
    pub(super) selected_paths: BTreeSet<String>,
    /// Индекс строк, видимых в текущем состоянии раскрытия дерева.
    pub(super) visible_rows: VisibleRows,
    /// Требуется ли перестроить индекс видимых строк перед отрисовкой.
    pub(super) visible_rows_dirty: bool,
    /// Последний успешно сформированный буфер структур внутри приложения.
    ///
    /// Нужен как запасной вариант, если системный буфер временно недоступен.
    pub(super) clipboard_payload: Option<Vec<ClipboardEntry>>,
    /// Отложенный запрос копирования выбранных структур.
    pub(super) copy_structures_requested: bool,
    /// Отложенный запрос вставки в выбранный контейнер.
    pub(super) paste_requested: bool,
}

impl Default for JsonViewerApp {
    fn default() -> Self {
        Self {
            root: None,
            parse_error: None,
            search: SearchState::default(),
            search_query_buf: String::new(),
            search_scroll_target: None,
            save_requested: false,
            file_state: FileState::default(),
            comparison: None,
            toast: None,
            dark_mode: true,
            mode: AppMode::default(),
            locale: Locale::default(),
            field_dialog: None,
            selected_paths: BTreeSet::new(),
            visible_rows: VisibleRows::default(),
            visible_rows_dirty: true,
            clipboard_payload: None,
            copy_structures_requested: false,
            paste_requested: false,
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
        Self::new_with_files(cc, path.into_iter().collect())
    }

    /// Создать приложение и открыть один файл или режим сравнения нескольких файлов.
    pub fn new_with_files(cc: &eframe::CreationContext<'_>, paths: Vec<PathBuf>) -> Self {
        let mut app = Self::new(cc);
        match paths.as_slice() {
            [] => {}
            [path] => app.load_file(path.clone()),
            _ => app.load_comparison(paths),
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
        self.comparison = None;
        self.selected_paths.clear();
        self.visible_rows_dirty = true;
        self.file_state = FileState::default();
        self.field_dialog = None;
        self.save_requested = false;
        self.copy_structures_requested = false;
        self.paste_requested = false;
        let t0 = Instant::now();
        match std::fs::read_to_string(&path) {
            Err(e) => {
                self.parse_error = Some(ParseError {
                    message: self.locale.file_read_error(&e.to_string()),
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
                        self.search_scroll_target = None;
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

    /// Открыть диалог выбора имени и создать новый пустой файл.
    ///
    /// Формат определяется по расширению выбранного пути. Новый документ
    /// содержит пустой объект, сразу открывается в режиме редактирования и
    /// записывается на диск, чтобы файл был создан до добавления данных.
    pub(super) fn open_new_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("untitled.json")
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("JSON5", &["json5"])
            .save_file()
        {
            let Some(format) = DataFormat::from_path(&path) else {
                self.show_toast(self.locale.text(TextKey::UnsupportedFileExtension));
                return;
            };
            self.create_new_file(path, format);
        }
    }

    /// Инициализировать новый документ с пустым объектом и сохранить его.
    fn create_new_file(&mut self, path: PathBuf, format: DataFormat) {
        let started_at = Instant::now();
        let root = match parse_data("{}", Some(DataFormat::Json)) {
            Ok((root, _)) => root,
            Err(error) => {
                self.show_toast(&format!(
                    "{} {}",
                    self.locale.text(TextKey::DataParseError),
                    error
                ));
                return;
            }
        };

        self.close_file();
        self.root = Some(root);
        self.mode = AppMode::Edit;
        self.file_state = FileState {
            path: Some(path.clone()),
            size_bytes: 0,
            load_time_ms: started_at.elapsed().as_millis(),
            format: Some(format),
        };

        match self.write_root_to_path(&path, format) {
            Ok(size_bytes) => {
                self.file_state.size_bytes = size_bytes;
                self.file_state.load_time_ms = started_at.elapsed().as_millis();
                self.show_toast(self.locale.text(TextKey::FileCreated));
            }
            Err(error) => {
                self.close_file();
                self.show_toast(&error);
            }
        }
    }

    /// Открыть системный диалог выбора нескольких файлов для сравнения.
    pub(super) fn open_comparison_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("All files", &["*"])
            .pick_files()
        {
            self.load_comparison(paths);
        }
    }

    /// Загрузить два или более файла и показать отличия между ними.
    pub(super) fn load_comparison(&mut self, paths: Vec<PathBuf>) {
        if paths.len() < 2 {
            self.show_toast(self.locale.text(TextKey::ComparisonRequiresFiles));
            return;
        }

        self.root = None;
        self.comparison = None;
        self.parse_error = None;
        self.file_state = FileState::default();
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.search = SearchState::default();
        self.search_query_buf.clear();
        self.search_scroll_target = None;
        self.save_requested = false;
        self.field_dialog = None;
        self.mode = AppMode::View;
        self.selected_paths.clear();
        self.copy_structures_requested = false;
        self.paste_requested = false;

        let locale = self.locale;
        let mut documents = Vec::with_capacity(paths.len());
        let mut values = Vec::with_capacity(paths.len());
        for path in paths {
            let started_at = Instant::now();
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(error) => {
                    self.parse_error = Some(ParseError {
                        message: locale.file_read_error(&format!("{}: {}", path.display(), error)),
                        line: None,
                        column: None,
                    });
                    return;
                }
            };
            let size_bytes = content.len() as u64;
            let format_hint = DataFormat::from_path(&path);
            let (node, format) = match parse_data(&content, format_hint) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.parse_error = Some(ParseError {
                        message: format!("{}: {}", path.display(), error),
                        ..error
                    });
                    return;
                }
            };
            let value = match node_to_value(&node) {
                Ok(value) => value,
                Err(error) => {
                    self.parse_error = Some(ParseError {
                        message: format!("{}: {}", path.display(), error),
                        line: None,
                        column: None,
                    });
                    return;
                }
            };

            documents.push(ComparisonDocument {
                path,
                size_bytes,
                load_time_ms: started_at.elapsed().as_millis(),
                format,
            });
            values.push(value);
        }

        self.comparison = Some(ComparisonState {
            documents,
            differences: compare_values(&values),
        });
    }

    /// Сохранить текущие данные в форматированном виде.
    ///
    /// Формат результата определяется по расширению выбранного файла.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    pub(super) fn save_pretty(&mut self) {
        if self.root.is_none() {
            return;
        }

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
            match self.write_root_to_path(&save_path, format) {
                Ok(_) => self.show_toast(self.locale.text(TextKey::FileSaved)),
                Err(error) => self.show_toast(&error),
            }
        }
    }

    /// Преобразовать открытый документ в выбранный формат и сохранить его
    /// отдельным файлом.
    ///
    /// Исходный документ остаётся открытым. Расширение результата определяется
    /// выбранным форматом, даже если пользователь ввёл в диалоге другое
    /// расширение.
    pub(super) fn convert_to_format(&mut self, format: DataFormat) {
        if self.root.is_none() {
            return;
        }

        let format_name = format.to_string();
        let mut dialog = rfd::FileDialog::new().add_filter(&format_name, format.extensions());
        let file_name = self.conversion_file_name(format);
        dialog = dialog.set_file_name(&file_name);

        if let Some(save_path) = dialog.save_file() {
            let save_path = with_format_extension(save_path, format);
            match self.write_root_to_path(&save_path, format) {
                Ok(_) => self.show_toast(self.locale.text(TextKey::FileConverted)),
                Err(error) => self.show_toast(&error),
            }
        }
    }

    /// Сохранить текущие данные в открытый файл без запроса нового пути.
    ///
    /// Если файл ещё не был сохранён, открывается диалог «Сохранить как…».
    pub(super) fn save_current(&mut self) {
        let Some(path) = self.file_state.path.clone() else {
            self.save_pretty();
            return;
        };

        let format = DataFormat::from_path(&path)
            .or(self.file_state.format)
            .unwrap_or(DataFormat::Json);
        match self.write_root_to_path(&path, format) {
            Ok(size_bytes) => {
                self.file_state.size_bytes = size_bytes;
                self.show_toast(self.locale.text(TextKey::FileSaved));
            }
            Err(error) => self.show_toast(&error),
        }
    }

    /// Отложить сохранение до завершения текущей отрисовки дерева.
    pub(super) fn request_save_current(&mut self) {
        self.save_requested = true;
    }

    /// Закрыть текущий документ и очистить связанные с ним состояния.
    pub(super) fn close_file(&mut self) {
        self.root = None;
        self.comparison = None;
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.parse_error = None;
        self.search = SearchState::default();
        self.search_query_buf.clear();
        self.search_scroll_target = None;
        self.save_requested = false;
        self.file_state = FileState::default();
        self.field_dialog = None;
        self.mode = AppMode::View;
        self.selected_paths.clear();
        self.copy_structures_requested = false;
        self.paste_requested = false;
    }

    /// Проверить, отображается ли сейчас режим сравнения.
    pub(super) fn is_comparing(&self) -> bool {
        self.comparison.is_some()
    }

    /// Сериализовать корень и записать его в указанный путь.
    fn write_root_to_path(&self, path: &Path, format: DataFormat) -> Result<u64, String> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
        let value = node_to_value(root)?;
        let formatted = serialize_data(&value, format, false)?;
        let size_bytes = formatted.len() as u64;
        std::fs::write(path, formatted)
            .map_err(|error| self.locale.save_error(&error.to_string()))?;
        Ok(size_bytes)
    }

    /// Сформировать имя результата преобразования рядом с именем открытого
    /// файла, заменив его расширение на расширение целевого формата.
    fn conversion_file_name(&self, format: DataFormat) -> String {
        let stem = self
            .file_state
            .path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("converted");
        format!("{}.{}", stem, format.extension())
    }

    /// Показать кратковременное уведомление в статус-баре.
    pub(super) fn show_toast(&mut self, message: &str) {
        self.toast = Some((message.to_string(), Instant::now()));
    }

    /// Применить к текущему выбору действие клика по узлу.
    pub(super) fn apply_selection_request(&mut self, request: SelectionRequest) {
        if !request.additive {
            self.selected_paths.clear();
            self.selected_paths.insert(request.path);
            return;
        }

        if !self.selected_paths.insert(request.path.clone()) {
            self.selected_paths.remove(&request.path);
        }
    }

    /// Проверить, можно ли вставить структуры в текущий выбор.
    pub(super) fn can_paste_into_selected(&self) -> bool {
        let Some(path) = self.selected_paths.iter().next() else {
            return false;
        };
        self.selected_paths.len() == 1
            && self
                .root
                .as_ref()
                .and_then(|root| find_node(root, path))
                .is_some_and(|node| {
                    matches!(
                        node.value_type,
                        JsonValueType::Object | JsonValueType::Array
                    )
                })
    }

    /// Скопировать структуры по указанным путям в системный буфер обмена.
    pub(super) fn copy_structures_at_paths(&mut self, paths: Vec<String>) {
        let selected_paths = paths.into_iter().collect::<BTreeSet<_>>();
        let entries = match self.root.as_ref() {
            Some(root) => selected_structures(root, &selected_paths),
            None => Err(self.locale.text(TextKey::NoDocument).to_string()),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };

        let encoded = match encode_structures(&entries) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };
        self.clipboard_payload = Some(entries.clone());
        match copy_to_clipboard(&encoded) {
            Ok(()) => self.show_toast(&self.locale.structures_copied(entries.len())),
            Err(error) => self.show_toast(&self.locale.system_copy_error(&error)),
        }
    }

    /// Вставить структуры в единственный выбранный контейнер.
    pub(super) fn paste_into_selected(&mut self) {
        let Some(path) = self.selected_paths.iter().next().cloned() else {
            self.show_toast(self.locale.text(TextKey::SelectContainer));
            return;
        };
        if self.selected_paths.len() != 1 {
            self.show_toast(self.locale.text(TextKey::SelectOneContainer));
            return;
        }
        self.paste_into_path(path);
    }

    /// Вставить структуры в контейнер по пути.
    pub(super) fn paste_into_path(&mut self, target_path: String) {
        if self.mode != AppMode::Edit {
            self.show_toast(self.locale.text(TextKey::PasteEditOnly));
            return;
        }

        let entries = match read_from_clipboard() {
            Ok(text) => decode_structures(&text),
            Err(system_error) => self
                .clipboard_payload
                .clone()
                .ok_or_else(|| self.locale.clipboard_read_error(&system_error)),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };

        let result = self
            .root
            .as_mut()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())
            .and_then(|root| paste_structures_at_path(root, &target_path, &entries));

        match result {
            Ok(count) => {
                self.retain_valid_selected_paths();
                self.visible_rows_dirty = true;
                self.refresh_search();
                self.show_toast(&self.locale.structures_pasted(count));
            }
            Err(error) => self.show_toast(&self.locale.paste_error(&error)),
        }
    }

    fn retain_valid_selected_paths(&mut self) {
        if let Some(root) = &self.root {
            self.selected_paths
                .retain(|path| find_node(root, path).is_some());
        } else {
            self.selected_paths.clear();
        }
    }

    /// Пересчитать результаты поиска по текущему запросу.
    ///
    /// Вызывается после правки дерева, чтобы подсветка оставалась актуальной.
    pub(super) fn refresh_search(&mut self) {
        self.search_scroll_target = None;
        let Some(root) = &self.root else {
            return;
        };

        let query = self.search_query_buf.clone();
        self.search.search(root, &query);
    }

    /// Запланировать прокрутку к текущему совпадению, если оно существует.
    pub(super) fn request_search_scroll(&mut self) {
        self.search_scroll_target = self.search.current_match_path().map(str::to_owned);
    }

    /// Открыть диалог добавления данных в выбранный контейнер.
    pub(super) fn open_add_child_dialog(&mut self, request: AddChildRequest) {
        self.field_dialog = Some(request.into());
    }

    /// Открыть конструктор для редактирования существующего узла.
    pub(super) fn open_edit_field_dialog(&mut self, request: EditFieldRequest) {
        let result: Result<(String, JsonValueType, String, bool), String> = (|| {
            let root = self
                .root
                .as_ref()
                .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
            let node = find_node(root, &request.path)
                .ok_or_else(|| "Не удалось найти поле для редактирования".to_string())?;
            let value_type = node.value_type.clone();
            let value = match value_type {
                JsonValueType::String => serde_json::from_str::<String>(&node.display_value)
                    .map_err(|error| format!("Некорректная строка: {}", error))?,
                _ => node.display_value.clone(),
            };
            Ok((
                node.key.clone().unwrap_or_default(),
                node.value_type.clone(),
                value,
                is_object_child(root, &request.path),
            ))
        })();

        let (key, value_type, value, key_editable) = match result {
            Ok(data) => data,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };

        self.field_dialog = Some(FieldDialog {
            target: FieldDialogTarget::Edit {
                path: request.path,
                key_editable,
            },
            key,
            value_type,
            value,
            error: None,
        });
    }

    /// Отрисовать конструктор и добавить или изменить узел после подтверждения.
    pub(super) fn show_field_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.field_dialog.take() else {
            return;
        };

        let mut submit = false;
        let mut cancel = false;
        let locale = self.locale;
        let format = self.file_state.format.unwrap_or(DataFormat::Json);
        let is_toml_root = format == DataFormat::Toml
            && matches!(&dialog.target, FieldDialogTarget::Edit { path, .. } if path.is_empty());
        let is_edit = matches!(&dialog.target, FieldDialogTarget::Edit { .. });
        let show_key_input = match &dialog.target {
            FieldDialogTarget::Add { is_object, .. } => *is_object,
            FieldDialogTarget::Edit { key_editable, .. } => *key_editable,
        };
        let show_readonly_key = is_edit && !show_key_input && !dialog.key.is_empty();
        let title = match &dialog.target {
            FieldDialogTarget::Add {
                is_object: true, ..
            } => locale.text(TextKey::AddFieldTitle),
            FieldDialogTarget::Add {
                is_object: false, ..
            } => locale.text(TextKey::AddElementTitle),
            FieldDialogTarget::Edit { .. } => locale.text(TextKey::EditFieldTitle),
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                if show_key_input {
                    ui.label(locale.text(TextKey::FieldName));
                    ui.add(egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0));
                } else if show_readonly_key {
                    ui.horizontal(|ui| {
                        ui.label(locale.text(TextKey::FieldName));
                        ui.add_enabled(
                            false,
                            egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0),
                        );
                    });
                }

                ui.horizontal(|ui| {
                    ui.label(locale.text(TextKey::FieldType));
                    let previous_type = dialog.value_type.clone();
                    egui::ComboBox::from_id_salt("field_dialog_type")
                        .selected_text(field_type_label(locale, &dialog.value_type))
                        .show_ui(ui, |ui| {
                            for value_type in field_value_types(format, is_toml_root) {
                                ui.selectable_value(
                                    &mut dialog.value_type,
                                    value_type.clone(),
                                    field_type_label(locale, &value_type),
                                );
                            }
                        });
                    if dialog.value_type != previous_type {
                        dialog.value = default_field_value(&dialog.value_type);
                    }
                });

                match &dialog.value_type {
                    JsonValueType::String => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::multiline(&mut dialog.value)
                                .desired_width(420.0)
                                .desired_rows(4),
                        );
                    }
                    JsonValueType::Number => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.value)
                                .desired_width(320.0)
                                .font(egui::TextStyle::Monospace),
                        );
                    }
                    JsonValueType::Bool => {
                        ui.horizontal(|ui| {
                            ui.label(locale.text(TextKey::Value));
                            egui::ComboBox::from_id_salt("field_dialog_bool")
                                .selected_text(&dialog.value)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "true".to_string(),
                                        "true",
                                    );
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "false".to_string(),
                                        "false",
                                    );
                                });
                        });
                    }
                    JsonValueType::Null => {
                        ui.label(format!("{}: null", locale.text(TextKey::Value)));
                    }
                    JsonValueType::Object => {
                        ui.label(locale.text(TextKey::EmptyObject));
                    }
                    JsonValueType::Array => {
                        ui.label(locale.text(TextKey::EmptyArray));
                    }
                }

                if let Some(error) = &dialog.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    let action = if is_edit {
                        TextKey::Apply
                    } else {
                        TextKey::Add
                    };
                    if ui.button(locale.text(action)).clicked() {
                        submit = true;
                    }
                    if ui.button(locale.text(TextKey::Cancel)).clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            return;
        }
        if !submit {
            self.field_dialog = Some(dialog);
            return;
        }

        let target = dialog.target.clone();
        let result = self
            .root
            .as_mut()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())
            .and_then(|root| match target {
                FieldDialogTarget::Add { parent_path, .. } => add_typed_child_at_path(
                    root,
                    &parent_path,
                    &dialog.key,
                    &dialog.value_type,
                    &dialog.value,
                    format,
                ),
                FieldDialogTarget::Edit { path, key_editable } => edit_child_at_path(
                    root,
                    &path,
                    key_editable.then_some(dialog.key.as_str()),
                    &dialog.value_type,
                    &dialog.value,
                    format,
                ),
            });

        match result {
            Ok(()) => {
                if is_edit {
                    self.retain_valid_selected_paths();
                }
                self.visible_rows_dirty = true;
                self.refresh_search();
                let message = if is_edit {
                    TextKey::FieldUpdated
                } else {
                    TextKey::DataAdded
                };
                self.show_toast(self.locale.text(message));
            }
            Err(error) => {
                dialog.error = Some(error);
                self.field_dialog = Some(dialog);
            }
        }
    }
}

fn field_value_types(format: DataFormat, is_toml_root: bool) -> Vec<JsonValueType> {
    if is_toml_root {
        return vec![JsonValueType::Object];
    }

    let mut types = vec![
        JsonValueType::String,
        JsonValueType::Number,
        JsonValueType::Bool,
        JsonValueType::Object,
        JsonValueType::Array,
    ];
    if format != DataFormat::Toml {
        types.insert(3, JsonValueType::Null);
    }
    types
}

fn field_type_label(locale: Locale, value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::String => locale.text(TextKey::TypeString),
        JsonValueType::Number => locale.text(TextKey::TypeNumber),
        JsonValueType::Bool => locale.text(TextKey::TypeBoolean),
        JsonValueType::Null => locale.text(TextKey::TypeNull),
        JsonValueType::Object => locale.text(TextKey::TypeObject),
        JsonValueType::Array => locale.text(TextKey::TypeArray),
    }
}

fn default_field_value(value_type: &JsonValueType) -> String {
    match value_type {
        JsonValueType::Bool => "true".to_string(),
        JsonValueType::String
        | JsonValueType::Number
        | JsonValueType::Null
        | JsonValueType::Object
        | JsonValueType::Array => String::new(),
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

/// Привести путь результата к расширению выбранного формата.
fn with_format_extension(mut path: PathBuf, format: DataFormat) -> PathBuf {
    path.set_extension(format.extension());
    path
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        AppMode, JsonViewerApp, edit_child_at_path, paste_structures_at_path, with_format_extension,
    };
    use crate::clipboard::ClipboardEntry;
    use crate::parser::{DataFormat, JsonValueType, parse_data};

    #[test]
    fn selection_survives_paste_and_edits_when_paths_remain_valid() {
        let (root, _) = parse_data(
            r#"{"target":{"value":1},"other":true}"#,
            Some(DataFormat::Json),
        )
        .unwrap();
        let mut app = JsonViewerApp {
            root: Some(root),
            selected_paths: BTreeSet::from([
                "target".to_string(),
                "target.value".to_string(),
                "other".to_string(),
            ]),
            ..JsonViewerApp::default()
        };
        let selected_before_change = app.selected_paths.clone();

        paste_structures_at_path(
            app.root.as_mut().unwrap(),
            "target",
            &[ClipboardEntry {
                key: Some("added".to_string()),
                value: serde_json::json!(2),
            }],
        )
        .unwrap();
        app.retain_valid_selected_paths();
        assert_eq!(app.selected_paths, selected_before_change);

        edit_child_at_path(
            app.root.as_mut().unwrap(),
            "target.value",
            None,
            &JsonValueType::Number,
            "3",
            DataFormat::Json,
        )
        .unwrap();
        app.retain_valid_selected_paths();
        assert_eq!(app.selected_paths, selected_before_change);

        edit_child_at_path(
            app.root.as_mut().unwrap(),
            "target",
            None,
            &JsonValueType::Number,
            "4",
            DataFormat::Json,
        )
        .unwrap();
        app.retain_valid_selected_paths();
        assert_eq!(
            app.selected_paths,
            BTreeSet::from(["other".to_string(), "target".to_string()])
        );
    }

    #[test]
    fn save_current_writes_updated_document_to_loaded_path() {
        let path =
            std::env::temp_dir().join(format!("json_viewer-save-test-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"value":1}"#).unwrap();

        let mut app = JsonViewerApp::default();
        app.load_file(path.clone());
        app.root
            .as_mut()
            .unwrap()
            .children
            .first_mut()
            .unwrap()
            .display_value = "2".to_string();

        app.save_current();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("\"value\": 2"));
        assert_eq!(app.file_state.size_bytes, saved.len() as u64);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn open_document_can_be_converted_to_every_other_format() {
        let input = std::env::temp_dir().join(format!(
            "json_viewer-convert-test-{}.json",
            std::process::id()
        ));
        std::fs::write(&input, r#"{"server":{"port":8080},"enabled":true}"#).unwrap();

        let mut app = JsonViewerApp::default();
        app.load_file(input.clone());

        for (index, format) in DataFormat::ALL.into_iter().enumerate() {
            if format == DataFormat::Json {
                continue;
            }

            let requested_path = std::env::temp_dir().join(format!(
                "json_viewer-convert-test-{}-{}.output",
                std::process::id(),
                index
            ));
            let output = with_format_extension(requested_path, format);
            let size_bytes = app.write_root_to_path(&output, format).unwrap();
            let content = std::fs::read_to_string(&output).unwrap();
            let (_, parsed_format) = parse_data(&content, Some(format)).unwrap();

            assert_eq!(parsed_format, format);
            assert_eq!(size_bytes, content.len() as u64);
            std::fs::remove_file(output).unwrap();
        }

        assert_eq!(app.file_state.path, Some(input.clone()));
        assert_eq!(app.file_state.format, Some(DataFormat::Json));
        std::fs::remove_file(input).unwrap();
    }

    #[test]
    fn close_file_clears_document_state() {
        let path = std::env::temp_dir().join(format!(
            "json_viewer-close-test-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, r#"{"value":1}"#).unwrap();

        let mut app = JsonViewerApp::default();
        app.load_file(path.clone());
        app.search_query_buf = "value".to_string();
        app.save_requested = true;
        app.close_file();

        assert!(app.root.is_none());
        assert!(app.file_state.path.is_none());
        assert!(app.search_query_buf.is_empty());
        assert!(!app.save_requested);
        assert_eq!(app.mode, super::AppMode::View);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn comparison_loads_all_documents_and_changed_paths() {
        let prefix =
            std::env::temp_dir().join(format!("json_viewer-compare-test-{}", std::process::id()));
        let first = prefix.with_extension("first.json");
        let second = prefix.with_extension("second.json");
        std::fs::write(&first, r#"{"value":1,"same":true}"#).unwrap();
        std::fs::write(&second, r#"{"value":2,"same":true}"#).unwrap();

        let mut app = JsonViewerApp::default();
        app.load_comparison(vec![first.clone(), second.clone()]);

        let comparison = app.comparison.as_ref().unwrap();
        assert_eq!(comparison.documents.len(), 2);
        assert_eq!(comparison.differences.len(), 1);
        assert_eq!(comparison.differences[0].path, "$.value");
        assert!(app.root.is_none());
        assert!(app.parse_error.is_none());

        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
    }

    #[test]
    fn creating_new_file_initializes_editable_document_for_all_formats() {
        let formats = [
            DataFormat::Json,
            DataFormat::Yaml,
            DataFormat::Toml,
            DataFormat::Json5,
        ];

        for (index, format) in formats.into_iter().enumerate() {
            let path = std::env::temp_dir().join(format!(
                "json_viewer-create-test-{}-{}.{}",
                std::process::id(),
                index,
                format.extension()
            ));
            let mut app = JsonViewerApp::default();
            app.create_new_file(path.clone(), format);

            assert_eq!(app.mode, AppMode::Edit);
            assert_eq!(app.file_state.path.as_deref(), Some(path.as_path()));
            assert_eq!(app.file_state.format, Some(format));
            assert_eq!(
                app.root.as_ref().map(|root| root.value_type.clone()),
                Some(JsonValueType::Object)
            );

            let content = std::fs::read_to_string(&path).unwrap();
            let (_, parsed_format) = parse_data(&content, Some(format)).unwrap();
            assert_eq!(parsed_format, format);
            assert_eq!(app.file_state.size_bytes, content.len() as u64);
            std::fs::remove_file(path).unwrap();
        }
    }
}
