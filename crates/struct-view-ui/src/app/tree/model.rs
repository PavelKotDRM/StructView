use std::collections::BTreeSet;

use struct_view_core::parser::{JsonNode, JsonValueType};
use struct_view_core::search::SearchState;

use super::super::i18n::Locale;
use super::super::state::AppMode;

/// Действия, отложенные до завершения отрисовки дерева.
///
/// Во время обхода дерева удерживается `&mut JsonNode`, поэтому изменить
/// состояние приложения напрямую нельзя. Запросы накапливаются здесь
/// и обрабатываются вызывающей стороной.
#[derive(Debug, Default)]
pub(in crate::app) struct TreeOutcome {
    /// Текст, который пользователь попросил скопировать в буфер обмена.
    pub(in crate::app) copy_request: Option<String>,
    /// Пути структур, которые пользователь попросил скопировать.
    pub(in crate::app) copy_structure_paths: Option<Vec<String>>,
    /// Путь контейнера, в который пользователь попросил вставить структуры.
    pub(in crate::app) paste_target_path: Option<String>,
    /// Последнее изменение выбора узла.
    pub(in crate::app) selection_request: Option<SelectionRequest>,
    /// Сообщение об ошибке правки значения.
    pub(in crate::app) edit_error: Option<String>,
    /// Признак того, что дерево было изменено и поиск нужно пересчитать.
    pub(in crate::app) tree_changed: bool,
    /// Признак изменения раскрытия контейнера.
    pub(in crate::app) expansion_changed: bool,
    /// Запрос на открытие диалога добавления поля или элемента.
    pub(in crate::app) add_child_request: Option<AddChildRequest>,
    /// Запрос на открытие конструктора существующего поля.
    pub(in crate::app) edit_field_request: Option<EditFieldRequest>,
    /// События inline-редактирования для группировки в одну команду истории.
    pub(in crate::app) inline_edit_events: Vec<InlineEditEvent>,
}

#[derive(Debug)]
pub(in crate::app) struct InlineEditEvent {
    pub(in crate::app) path: String,
    pub(in crate::app) before_value_type: JsonValueType,
    pub(in crate::app) before_display_value: String,
    pub(in crate::app) changed: bool,
    pub(in crate::app) finished: bool,
    pub(in crate::app) valid: bool,
}

/// Запрос на выбор узла дерева.
#[derive(Debug, Clone)]
pub(in crate::app) struct SelectionRequest {
    /// Путь узла, по которому кликнул пользователь.
    pub(in crate::app) path: String,
    /// Добавить узел к текущему выбору вместо замены.
    pub(in crate::app) additive: bool,
}

/// Неизменяемый контекст одного обхода дерева.
pub(in crate::app) struct RenderOptions<'a> {
    /// Состояние поиска.
    pub(in crate::app) search: &'a SearchState,
    /// Текущий режим приложения.
    pub(in crate::app) mode: AppMode,
    /// Путь совпадения, к которому нужно прокрутить дерево.
    pub(in crate::app) scroll_to_path: Option<&'a str>,
    /// Пути выбранных узлов.
    pub(in crate::app) selected_paths: &'a BTreeSet<String>,
    /// Язык интерфейса.
    pub(in crate::app) locale: Locale,
}

/// Контейнер, в который пользователь хочет добавить данные.
#[derive(Debug, Clone)]
pub(in crate::app) struct AddChildRequest {
    /// Путь контейнера в дереве.
    pub(in crate::app) parent_path: String,
    /// `true`, если контейнер является объектом и требуется имя поля.
    pub(in crate::app) is_object: bool,
}

/// Запрос на редактирование существующего узла через конструктор.
#[derive(Debug, Clone)]
pub(in crate::app) struct EditFieldRequest {
    /// Путь редактируемого узла.
    pub(in crate::app) path: String,
}

/// Компактный индекс строк, видимых при текущем состоянии раскрытия.
///
/// В индексах хранятся позиции дочерних узлов, а не копии строковых путей.
/// Это позволяет быстро находить строку при прокрутке больших массивов без
/// повторного обхода всех предшествующих узлов.
#[derive(Debug, Default)]
pub(in crate::app) struct VisibleRows {
    rows: Vec<VisibleRow>,
    path_indices: Vec<usize>,
}

#[derive(Debug)]
struct VisibleRow {
    path_start: usize,
    depth: usize,
}

impl VisibleRows {
    /// Построить индекс по текущему состоянию раскрытия дерева.
    pub(in crate::app) fn from_root(root: &JsonNode) -> Self {
        let mut rows = Vec::new();
        let mut path_indices = Vec::new();
        let mut path = Vec::new();
        collect_visible_rows(root, &mut path, &mut rows, &mut path_indices);
        Self { rows, path_indices }
    }

    /// Вернуть число строк в индексе.
    pub(in crate::app) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Получить путь и уровень вложенности строки.
    pub(super) fn row(&self, index: usize) -> Option<(&[usize], usize)> {
        let row = self.rows.get(index)?;
        let path = &self.path_indices[row.path_start..row.path_start + row.depth];
        Some((path, row.depth))
    }
}

/// Найти индекс видимой строки по пути узла.
pub(in crate::app) fn visible_row_index(root: &JsonNode, target_path: &str) -> Option<usize> {
    let mut row_index = 0;
    find_visible_row_index(root, target_path, &mut row_index)
}

/// Рекурсивно найти индекс узла в порядке отображения дерева.
fn find_visible_row_index(
    node: &JsonNode,
    target_path: &str,
    row_index: &mut usize,
) -> Option<usize> {
    let current_index = *row_index;
    *row_index += 1;
    if node.path == target_path {
        return Some(current_index);
    }

    if node.expanded {
        for child in &node.children {
            if let Some(index) = find_visible_row_index(child, target_path, row_index) {
                return Some(index);
            }
        }
    }
    None
}

/// Рекурсивно построить компактный индекс видимых строк.
fn collect_visible_rows(
    node: &JsonNode,
    path: &mut Vec<usize>,
    rows: &mut Vec<VisibleRow>,
    path_indices: &mut Vec<usize>,
) {
    let path_start = path_indices.len();
    path_indices.extend(path.iter().copied());
    rows.push(VisibleRow {
        path_start,
        depth: path.len(),
    });

    if node.expanded {
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            collect_visible_rows(child, path, rows, path_indices);
            path.pop();
        }
    }
}

/// Свернуть нерелевантные ветки и раскрыть контейнеры на пути к совпадению.
pub(in crate::app) fn focus_match_path(node: &mut JsonNode, target_path: &str) -> bool {
    let is_target = node.path == target_path;
    let mut contains_target = false;
    for child in &mut node.children {
        contains_target |= focus_match_path(child, target_path);
    }

    if !node.children.is_empty() {
        node.expanded = contains_target;
    }

    is_target || contains_target
}
