use std::collections::BTreeSet;
use std::path::PathBuf;

use struct_view_core::diff::Difference;
use struct_view_core::parser::{DataFormat, JsonNode, JsonValueType};
use struct_view_core::search::SearchState;

use super::super::tree::{AddChildRequest, VisibleRows};
use super::super::visualization::VisualizationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ToastKind {
    Success,
    Error,
}

#[derive(Debug)]
pub(in crate::app) struct Toast {
    pub(in crate::app) message: String,
    pub(in crate::app) shown_at: std::time::Instant,
    pub(in crate::app) kind: ToastKind,
}

/// Метаданные загруженного файла, отображаемые в статус-баре.
#[derive(Debug, Default)]
pub(in crate::app) struct FileState {
    /// Путь к файлу на диске.
    pub(in crate::app) path: Option<PathBuf>,
    /// Размер файла в байтах.
    pub(in crate::app) size_bytes: u64,
    /// Время загрузки и разбора файла в миллисекундах.
    pub(in crate::app) load_time_ms: u128,
    /// Формат открытого файла.
    pub(in crate::app) format: Option<DataFormat>,
    /// Отпечаток последнего сохранённого содержимого.
    pub(in crate::app) saved_content_fingerprint: Option<u64>,
}

pub(in crate::app) struct LoadedDocument {
    pub(in crate::app) path: PathBuf,
    pub(in crate::app) root: JsonNode,
    pub(in crate::app) size_bytes: u64,
    pub(in crate::app) load_time_ms: u128,
    pub(in crate::app) format: DataFormat,
    pub(in crate::app) saved_content_fingerprint: Option<u64>,
    pub(in crate::app) visible_rows: VisibleRows,
}

/// Документ, загруженный в режим сравнения.
#[derive(Debug)]
pub(in crate::app) struct ComparisonDocument {
    /// Путь к файлу.
    pub(in crate::app) path: PathBuf,
    /// Размер файла в байтах.
    pub(in crate::app) size_bytes: u64,
    /// Время загрузки и разбора файла в миллисекундах.
    pub(in crate::app) load_time_ms: u128,
    /// Формат файла.
    pub(in crate::app) format: DataFormat,
}

/// Состояние открытого документа, временно скрытого режимом сравнения.
#[derive(Debug)]
pub(in crate::app) struct PreviousDocumentState {
    pub(in crate::app) root: JsonNode,
    pub(in crate::app) file_state: FileState,
    pub(in crate::app) search: SearchState,
    pub(in crate::app) search_query_buf: String,
    pub(in crate::app) regex_builder_literal: String,
    pub(in crate::app) search_window_open: bool,
    pub(in crate::app) search_scroll_target: Option<String>,
    pub(in crate::app) mode: AppMode,
    pub(in crate::app) visualization: VisualizationMode,
    pub(in crate::app) selected_paths: BTreeSet<String>,
    pub(in crate::app) undo_history: Vec<JsonNode>,
    pub(in crate::app) redo_history: Vec<JsonNode>,
}

/// Состояние отображения отличий нескольких документов.
#[derive(Debug)]
pub(in crate::app) struct ComparisonState {
    /// Загруженные документы в порядке колонок таблицы.
    pub(in crate::app) documents: Vec<ComparisonDocument>,
    /// Отличия между значениями документов.
    pub(in crate::app) differences: Vec<Difference>,
    /// Индекс выбранной первой версии в парном diff.
    pub(in crate::app) left_index: usize,
    /// Индекс выбранной второй версии в парном diff.
    pub(in crate::app) right_index: usize,
    pub(in crate::app) pair_cache: Option<PairDifferenceCache>,
    /// Документ, который нужно восстановить после закрытия сравнения.
    pub(in crate::app) previous_document: Option<PreviousDocumentState>,
}

#[derive(Debug)]
pub(in crate::app) struct PairDifferenceCache {
    pub(in crate::app) left_index: usize,
    pub(in crate::app) right_index: usize,
    pub(in crate::app) differences: Vec<Difference>,
}

/// Режим работы приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum AppMode {
    /// Только просмотр данных без изменения значений.
    #[default]
    View,
    /// Разрешено редактирование значений JSON.
    Edit,
}

/// Цель конструктора поля.
#[derive(Debug, Clone)]
pub(in crate::app) enum FieldDialogTarget {
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
pub(in crate::app) struct FieldDialog {
    pub(super) target: FieldDialogTarget,
    pub(super) key: String,
    pub(super) value_type: JsonValueType,
    pub(super) value: String,
    pub(super) error: Option<String>,
}

/// Полный снимок документа до незавершённого inline-редактирования.
#[derive(Debug)]
pub(in crate::app) struct PendingInlineEdit {
    pub(super) path: String,
    pub(super) root_before: JsonNode,
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
