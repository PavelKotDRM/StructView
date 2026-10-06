//! Состояние приложения и жизненный цикл [`eframe::App`].
//!
//! Здесь хранится всё, что переживает отдельный кадр отрисовки: разобранное
//! дерево данных, состояние поиска, метаданные файла и настройки темы.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use crate::clipboard::{
    ClipboardEntry, copy_to_clipboard, decode_structures, encode_structures, read_from_clipboard,
};
use struct_view_core::diff::compare_values;
use struct_view_core::files::write_text_atomic;
use struct_view_core::parser::{
    DataFormat, JsonNode, JsonValueType, ParseError, comment_input, node_to_value, parse_data,
    serialize_node,
};
use struct_view_core::search::SearchState;

use super::docking::DockingState;
use super::edit::{
    DeleteError, add_typed_child_at_path, apply_primitive_edit, delete_selected_structures,
    edit_child_at_path, find_node, find_node_mut, is_object_child, paste_structures_at_path,
    selected_structures,
};
use super::i18n::{Locale, TextKey};
use super::theme::SyntaxColors;
use super::tree::{
    AddChildRequest, EditFieldRequest, InlineEditEvent, SelectionRequest, VisibleRows,
};
use super::views::{GraphCalculationState, GraphRoutingWorkerSetting};
use super::visualization::{VisualizationCache, VisualizationMode};

mod dialog;
mod dialog_helpers;
mod editing;
mod files;
mod history;
mod models;
mod save;

pub(super) use dialog_helpers::{default_field_value, field_value_types};
pub(super) use models::{
    AppMode, ComparisonDocument, ComparisonState, FieldDialog, FieldDialogTarget, FileState,
    LoadedDocument, PairDifferenceCache, PendingInlineEdit, PreviousDocumentState, Toast,
    ToastKind,
};

const HISTORY_LIMIT: usize = 100;
const TOAST_LIFETIME: Duration = Duration::from_secs(3);

impl Toast {
    pub(super) fn remaining(&self) -> Duration {
        TOAST_LIFETIME.saturating_sub(self.shown_at.elapsed())
    }
}

fn push_limited_snapshot(history: &mut Vec<JsonNode>, snapshot: JsonNode) {
    if history.len() >= HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(snapshot);
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
    /// Текст, который конструктор добавит в запрос как литерал.
    pub(super) regex_builder_literal: String,
    /// Открыто ли расширенное окно поиска.
    pub(super) search_window_open: bool,
    /// Размещение окна поиска внутри или вне главного окна.
    pub(super) search_window_docking: DockingState,
    /// Путь совпадения, к которому нужно прокрутить дерево в следующем кадре.
    pub(super) search_scroll_target: Option<String>,
    /// Отложенный запрос на сохранение текущего файла.
    pub(super) save_requested: bool,
    /// Мета-информация о загруженном файле.
    pub(super) file_state: FileState,
    /// Результат фонового чтения и разбора файла.
    pub(super) file_load_receiver: Option<Receiver<Result<LoadedDocument, ParseError>>>,
    /// Состояние сравнения нескольких файлов.
    pub(super) comparison: Option<ComparisonState>,
    /// Временное уведомление (например, «Скопировано») и момент его показа.
    pub(super) toast: Option<Toast>,
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
    /// Асинхронное построение модели и маршрутов графа связей.
    pub(super) graph_calculation: GraphCalculationState,
    pub(super) graph_routing_workers: GraphRoutingWorkerSetting,
    pub(super) structure_view: super::views::structure::StructureView,
    /// Открытый конструктор добавления или редактирования поля.
    pub(super) field_dialog: Option<FieldDialog>,
    /// Размещение конструктора внутри или вне главного окна.
    pub(super) field_dialog_docking: DockingState,
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
            regex_builder_literal: String::new(),
            search_window_open: false,
            search_window_docking: DockingState::default(),
            search_scroll_target: None,
            save_requested: false,
            file_state: FileState::default(),
            file_load_receiver: None,
            comparison: None,
            toast: None,
            dark_mode: true,
            mode: AppMode::default(),
            locale: Locale::default(),
            visualization: VisualizationMode::default(),
            visualization_cache: VisualizationCache::default(),
            graph_calculation: GraphCalculationState::default(),
            graph_routing_workers: GraphRoutingWorkerSetting::default(),
            structure_view: super::views::structure::StructureView::default(),
            field_dialog: None,
            field_dialog_docking: DockingState::default(),
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
            [path] => app.request_file_load(path.clone()),
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

impl eframe::App for StructViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_file_load(ui.ctx());
        self.graph_calculation.poll(ui.ctx());
        let previous_toast = self.toast.as_ref().map(|toast| toast.shown_at);
        self.show_top_panel(ui);
        self.show_bottom_panel(ui);
        self.show_central_panel(ui);
        self.show_field_dialog(ui.ctx());
        if let Some(toast) = &self.toast {
            if Some(toast.shown_at) != previous_toast {
                ui.ctx().request_repaint();
            }
            ui.ctx().request_repaint_after(toast.remaining());
        }
    }
}

/// Привести путь результата к расширению выбранного формата.
fn with_format_extension(mut path: PathBuf, format: DataFormat) -> PathBuf {
    path.set_extension(format.extension());
    path
}

#[cfg(test)]
mod tests;
