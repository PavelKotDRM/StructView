//! Состояние приложения и жизненный цикл [`eframe::App`].
//!
//! Здесь хранится всё, что переживает отдельный кадр отрисовки: разобранное
//! дерево данных, состояние поиска, метаданные файла и настройки темы.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::clipboard::{
    ClipboardEntry, copy_to_clipboard, decode_structures, encode_structures, read_from_clipboard,
};
use struct_view_core::diff::{Difference, compare_values};
use struct_view_core::parser::{
    DataFormat, JsonNode, JsonValueType, ParseError, comment_input, node_to_value, parse_data,
    serialize_node,
};
use struct_view_core::search::SearchState;

use super::edit::{
    DeleteError, add_typed_child_at_path, apply_primitive_edit, delete_selected_structures,
    edit_child_at_path, find_node, find_node_mut, is_object_child, paste_structures_at_path,
    selected_structures,
};
use super::i18n::{Locale, TextKey};
use super::theme::value_color;
use super::tree::{
    AddChildRequest, EditFieldRequest, InlineEditEvent, SelectionRequest, VisibleRows,
};
use super::visualization::{VisualizationCache, VisualizationMode};

mod editing;
mod files;
mod history;
mod save;

const HISTORY_LIMIT: usize = 100;

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
    /// Индекс выбранной первой версии в парном diff.
    pub(super) left_index: usize,
    /// Индекс выбранной второй версии в парном diff.
    pub(super) right_index: usize,
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

/// Полный снимок документа до незавершённого inline-редактирования.
#[derive(Debug)]
pub(super) struct PendingInlineEdit {
    path: String,
    root_before: JsonNode,
}

fn push_limited_snapshot(history: &mut Vec<JsonNode>, snapshot: JsonNode) {
    if history.len() >= HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(snapshot);
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

/// Основное состояние приложения StructView.
///
/// Хранит дерево структурированных данных, параметры поиска, информацию о файле
/// и временные сообщения для пользователя (уведомления, ошибки).
pub struct StructViewApp {
    /// Корневой узел разобранного документа. `None` если файл ещё не загружен.
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
    /// Выбранное представление документа.
    pub(super) visualization: VisualizationMode,
    /// Вычисляемые модели представлений текущего документа.
    pub(super) visualization_cache: VisualizationCache,
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
    /// Отложенный запрос удаления выбранных структур.
    pub(super) delete_requested: bool,
    /// Отложенный запрос вставки в выбранный контейнер.
    pub(super) paste_requested: bool,
    /// История снимков до последних изменений документа.
    undo_history: Vec<JsonNode>,
    /// Снимки состояний, отменённых командой Undo.
    redo_history: Vec<JsonNode>,
    /// Снимок, собираемый для текущего inline-редактирования.
    pending_inline_edit: Option<PendingInlineEdit>,
    /// Отложенная команда Undo до завершения текущей отрисовки.
    pub(super) undo_requested: bool,
    /// Отложенная команда Redo до завершения текущей отрисовки.
    pub(super) redo_requested: bool,
}

impl Default for StructViewApp {
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
            visualization: VisualizationMode::default(),
            visualization_cache: VisualizationCache::default(),
            field_dialog: None,
            selected_paths: BTreeSet::new(),
            visible_rows: VisibleRows::default(),
            visible_rows_dirty: true,
            clipboard_payload: None,
            copy_structures_requested: false,
            delete_requested: false,
            paste_requested: false,
            undo_history: Vec::new(),
            redo_history: Vec::new(),
            pending_inline_edit: None,
            undo_requested: false,
            redo_requested: false,
        }
    }
}

impl StructViewApp {
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
}

fn field_value_types(
    format: DataFormat,
    is_toml_root: bool,
    is_comment_edit: bool,
) -> Vec<JsonValueType> {
    if is_comment_edit {
        return vec![JsonValueType::Comment];
    }
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
    if format == DataFormat::Toml {
        types.insert(1, JsonValueType::DateTime);
        types.insert(3, JsonValueType::Float);
    } else {
        types.insert(3, JsonValueType::Null);
    }
    if format != DataFormat::Json {
        types.push(JsonValueType::Comment);
    }
    if format == DataFormat::Yaml {
        types.push(JsonValueType::Metadata);
    }
    types
}

fn field_type_label(locale: Locale, value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::String => locale.text(TextKey::TypeString),
        JsonValueType::DateTime => locale.text(TextKey::TypeDateTime),
        JsonValueType::Comment => locale.text(TextKey::TypeComment),
        JsonValueType::Metadata => locale.text(TextKey::TypeMetadata),
        JsonValueType::Number => locale.text(TextKey::TypeNumber),
        JsonValueType::Float => locale.text(TextKey::TypeFloat),
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
        | JsonValueType::DateTime
        | JsonValueType::Number
        | JsonValueType::Float
        | JsonValueType::Null
        | JsonValueType::Object
        | JsonValueType::Array
        | JsonValueType::Comment
        | JsonValueType::Metadata => String::new(),
    }
}

impl eframe::App for StructViewApp {
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
mod tests;
