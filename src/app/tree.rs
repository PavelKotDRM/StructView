//! Виртуализированная отрисовка дерева JSON и контекстное меню узла.

use std::collections::BTreeSet;
use std::ops::Range;

use egui::{RichText, Ui};

use crate::parser::{JsonNode, JsonValueType};
use crate::search::SearchState;

use super::edit::apply_primitive_edit;
use super::i18n::{Locale, TextKey};
use super::state::AppMode;
use super::theme::{COLOR_ACTIVE_MATCH, COLOR_KEY, COLOR_MATCH, value_color};

/// Ширина поля ввода при редактировании примитивного значения.
const EDIT_FIELD_WIDTH: f32 = 300.0;

/// Действия, отложенные до завершения отрисовки дерева.
///
/// Во время обхода дерева удерживается `&mut JsonNode`, поэтому изменить
/// состояние приложения напрямую нельзя. Запросы накапливаются здесь
/// и обрабатываются вызывающей стороной.
#[derive(Debug, Default)]
pub(super) struct TreeOutcome {
    /// Текст, который пользователь попросил скопировать в буфер обмена.
    pub(super) copy_request: Option<String>,
    /// Пути структур, которые пользователь попросил скопировать.
    pub(super) copy_structure_paths: Option<Vec<String>>,
    /// Путь контейнера, в который пользователь попросил вставить структуры.
    pub(super) paste_target_path: Option<String>,
    /// Последнее изменение выбора узла.
    pub(super) selection_request: Option<SelectionRequest>,
    /// Сообщение об ошибке правки значения.
    pub(super) edit_error: Option<String>,
    /// Признак того, что дерево было изменено и поиск нужно пересчитать.
    pub(super) tree_changed: bool,
    /// Признак изменения раскрытия контейнера.
    pub(super) expansion_changed: bool,
    /// Запрос на открытие диалога добавления поля или элемента.
    pub(super) add_child_request: Option<AddChildRequest>,
    /// Запрос на открытие конструктора существующего поля.
    pub(super) edit_field_request: Option<EditFieldRequest>,
}

/// Запрос на выбор узла дерева.
#[derive(Debug, Clone)]
pub(super) struct SelectionRequest {
    /// Путь узла, по которому кликнул пользователь.
    pub(super) path: String,
    /// Добавить узел к текущему выбору вместо замены.
    pub(super) additive: bool,
}

/// Неизменяемый контекст одного обхода дерева.
pub(super) struct RenderOptions<'a> {
    /// Состояние поиска.
    pub(super) search: &'a SearchState,
    /// Текущий режим приложения.
    pub(super) mode: AppMode,
    /// Путь совпадения, к которому нужно прокрутить дерево.
    pub(super) scroll_to_path: Option<&'a str>,
    /// Пути выбранных узлов.
    pub(super) selected_paths: &'a BTreeSet<String>,
    /// Язык интерфейса.
    pub(super) locale: Locale,
}

/// Контейнер, в который пользователь хочет добавить данные.
#[derive(Debug, Clone)]
pub(super) struct AddChildRequest {
    /// Путь контейнера в дереве.
    pub(super) parent_path: String,
    /// `true`, если контейнер является объектом и требуется имя поля.
    pub(super) is_object: bool,
}

/// Запрос на редактирование существующего узла через конструктор.
#[derive(Debug, Clone)]
pub(super) struct EditFieldRequest {
    /// Путь редактируемого узла.
    pub(super) path: String,
}

/// Высота одной строки дерева без вертикального промежутка между строками.
///
/// `ScrollArea::show_rows` использует фиксированную высоту, поэтому значения
/// контролов дерева должны согласовываться с высотой строки egui.
pub(super) fn tree_row_height(ui: &Ui) -> f32 {
    ui.spacing().interact_size.y
}

/// Компактный индекс строк, видимых при текущем состоянии раскрытия.
///
/// В индексах хранятся позиции дочерних узлов, а не копии строковых путей.
/// Это позволяет быстро находить строку при прокрутке больших массивов без
/// повторного обхода всех предшествующих узлов.
#[derive(Debug, Default)]
pub(super) struct VisibleRows {
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
    pub(super) fn from_root(root: &JsonNode) -> Self {
        let mut rows = Vec::new();
        let mut path_indices = Vec::new();
        let mut path = Vec::new();
        collect_visible_rows(root, &mut path, &mut rows, &mut path_indices);
        Self { rows, path_indices }
    }

    /// Вернуть число строк в индексе.
    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Получить путь и уровень вложенности строки.
    fn row(&self, index: usize) -> Option<(&[usize], usize)> {
        let row = self.rows.get(index)?;
        let path = &self.path_indices[row.path_start..row.path_start + row.depth];
        Some((path, row.depth))
    }
}

/// Найти индекс видимой строки по пути узла.
pub(super) fn visible_row_index(root: &JsonNode, target_path: &str) -> Option<usize> {
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

/// Отрисовать диапазон строк раскрытого дерева.
///
/// Вызов обычно выполняется из [`egui::ScrollArea::show_rows`]. Узлы
/// разрешаются по компактному индексу, а виджеты создаются исключительно
/// для строк из диапазона.
pub(super) fn render_visible_rows(
    ui: &mut Ui,
    root: &mut JsonNode,
    visible_rows: &VisibleRows,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
    row_range: Range<usize>,
) {
    for row_index in row_range {
        let Some((path, depth)) = visible_rows.row(row_index) else {
            break;
        };
        let Some(node) = node_at_path_mut(root, path) else {
            break;
        };
        render_node_row(ui, node, depth, options, outcome);
    }
}

/// Найти узел по компактному пути индексов.
fn node_at_path_mut<'a>(root: &'a mut JsonNode, path: &[usize]) -> Option<&'a mut JsonNode> {
    let mut node = root;
    for &index in path {
        node = node.children.get_mut(index)?;
    }
    Some(node)
}

/// Отрисовать одну строку дерева с учётом её уровня вложенности.
fn render_node_row(
    ui: &mut Ui,
    node: &mut JsonNode,
    depth: usize,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
) {
    let highlight = Highlight::for_node(options.search, &node.path);
    let scroll_to_match = options.scroll_to_path.is_some_and(|path| path == node.path);

    match node.value_type {
        JsonValueType::Object | JsonValueType::Array => render_container_row(
            ui,
            node,
            depth,
            options,
            outcome,
            highlight,
            scroll_to_match,
        ),
        _ => render_leaf(
            ui,
            node,
            depth,
            options,
            outcome,
            highlight,
            scroll_to_match,
        ),
    }
}

/// Свернуть нерелевантные ветки и раскрыть контейнеры на пути к совпадению.
pub(super) fn focus_match_path(node: &mut JsonNode, target_path: &str) -> bool {
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

/// Подсветка узла в зависимости от результатов поиска.
#[derive(Debug, Clone, Copy)]
struct Highlight {
    /// Узел входит в список совпадений.
    is_match: bool,
    /// Узел является текущим активным совпадением.
    is_active: bool,
}

impl Highlight {
    /// Вычислить подсветку для пути узла.
    fn for_node(search: &SearchState, path: &str) -> Self {
        if search.query.is_empty() {
            return Self {
                is_match: false,
                is_active: false,
            };
        }
        Self {
            is_match: search.is_match(path),
            is_active: search.is_active(path),
        }
    }

    /// Применить цвет подсветки к тексту, если узел найден поиском.
    ///
    /// Если узел не является совпадением, используется `default_color`.
    fn apply(self, text: RichText, default_color: egui::Color32) -> RichText {
        if self.is_active {
            text.color(COLOR_ACTIVE_MATCH).strong()
        } else if self.is_match {
            text.color(COLOR_MATCH).strong()
        } else {
            text.color(default_color)
        }
    }
}

/// Отрисовать строку объекта или массива с кнопкой раскрытия.
fn render_container_row(
    ui: &mut Ui,
    node: &mut JsonNode,
    depth: usize,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
    highlight: Highlight,
    scroll_to_match: bool,
) {
    let header_text = make_header_text(node, highlight, options.locale);
    let selected = options.selected_paths.contains(&node.path);
    let previous_expanded = node.expanded;

    let id = ui.make_persistent_id(&node.path);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        node.expanded,
    );

    // Если Expand All / Collapse All изменили node.expanded — принудительно
    // обновляем персистентное состояние egui.
    if state.is_open() != node.expanded {
        state.set_open(node.expanded);
        state.store(ui.ctx());
    }

    let row_response = ui.horizontal(|ui| {
        let row_height = tree_row_height(ui);
        ui.set_min_height(row_height);
        add_tree_indent(ui, depth);
        state
            .show_header(ui, |ui| ui.selectable_label(selected, header_text))
            .body_unindented(|_| {})
    });
    let (_toggle_response, header_inner, _body) = row_response.inner;

    if scroll_to_match {
        header_inner
            .response
            .scroll_to_me(Some(egui::Align::Center));
    }

    if header_inner.inner.clicked() {
        outcome.selection_request = Some(selection_request(ui, &node.path));
    }

    // Считываем актуальное состояние (пользователь мог кликнуть по заголовку)
    let updated = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        node.expanded,
    );
    node.expanded = updated.is_open();
    if node.expanded != previous_expanded {
        outcome.expansion_changed = true;
    }

    header_inner.inner.context_menu(|ui| {
        container_context_menu(
            ui,
            node,
            options.mode,
            options.locale,
            options.selected_paths,
            outcome,
        );
    });
}

/// Отрисовать листовой узел: ключ и значение (или поле ввода в режиме правки).
fn render_leaf(
    ui: &mut Ui,
    node: &mut JsonNode,
    depth: usize,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
    highlight: Highlight,
    scroll_to_match: bool,
) {
    let selected = options.selected_paths.contains(&node.path);
    let row_response = ui.horizontal(|ui| {
        let row_height = tree_row_height(ui);
        ui.set_min_height(row_height);
        add_tree_indent(ui, depth);

        if let Some(key) = &node.key {
            let key_text = highlight.apply(RichText::new(format!("{}: ", key)), COLOR_KEY);
            let key_resp = ui.selectable_label(selected, key_text);
            if key_resp.clicked() {
                outcome.selection_request = Some(selection_request(ui, &node.path));
            }
            key_resp.context_menu(|ui| {
                context_menu(
                    ui,
                    node,
                    options.mode,
                    options.locale,
                    options.selected_paths,
                    outcome,
                );
            });
        }

        if is_editable(node, options.mode) {
            render_value_editor(ui, node, options.locale, options.selected_paths, outcome);
        } else {
            let value_text = highlight.apply(
                RichText::new(&node.display_value),
                value_color(&node.value_type),
            );
            let value_resp = ui.selectable_label(selected, value_text);
            if value_resp.clicked() {
                outcome.selection_request = Some(selection_request(ui, &node.path));
            }
            value_resp.context_menu(|ui| {
                context_menu(
                    ui,
                    node,
                    options.mode,
                    options.locale,
                    options.selected_paths,
                    outcome,
                );
            });
        }
    });

    if scroll_to_match {
        row_response
            .response
            .scroll_to_me(Some(egui::Align::Center));
    }
}

/// Добавить отступ, соответствующий уровню узла в плоском списке строк.
fn add_tree_indent(ui: &mut Ui, depth: usize) {
    let indent = ui.spacing().indent * depth as f32;
    if indent > 0.0 {
        ui.add_space(indent);
    }
}

/// Проверить, доступно ли значение узла для правки в текущем режиме.
fn is_editable(node: &JsonNode, mode: AppMode) -> bool {
    mode == AppMode::Edit
        && matches!(
            node.value_type,
            JsonValueType::String
                | JsonValueType::Number
                | JsonValueType::Bool
                | JsonValueType::Null
        )
}

/// Отрисовать однострочное поле правки значения и применить изменение при потере фокуса.
fn render_value_editor(
    ui: &mut Ui,
    node: &mut JsonNode,
    locale: Locale,
    selected_paths: &BTreeSet<String>,
    outcome: &mut TreeOutcome,
) {
    let edit_resp = ui.add(
        egui::TextEdit::singleline(&mut node.display_value)
            .desired_width(EDIT_FIELD_WIDTH)
            .font(egui::TextStyle::Monospace),
    );

    if edit_resp.changed() {
        outcome.tree_changed = true;
    }

    if edit_resp.lost_focus() {
        let previous_type = node.value_type.clone();
        let previous_display = node.display_value.clone();
        let edited = node.display_value.clone();
        match apply_primitive_edit(node, &edited) {
            Ok(()) => {
                if node.value_type != previous_type || node.display_value != previous_display {
                    outcome.tree_changed = true;
                }
            }
            Err(err) => {
                // Откатываем поле ввода, поскольку `apply_primitive_edit` не изменяет
                // узел при ошибке, а `TextEdit` уже записал невалидный текст напрямую
                // в `node.display_value`.
                node.value_type = previous_type;
                node.display_value = previous_display;
                outcome.edit_error = Some(err);
            }
        }
    }

    if edit_resp.clicked() {
        outcome.selection_request = Some(selection_request(ui, &node.path));
    }
    edit_resp.context_menu(|ui| {
        context_menu(ui, node, AppMode::Edit, locale, selected_paths, outcome);
    });
}

fn selection_request(ui: &Ui, path: &str) -> SelectionRequest {
    SelectionRequest {
        path: path.to_string(),
        additive: ui.input(|input| input.modifiers.command),
    }
}

/// Сформировать текст заголовка для объекта/массива с учётом подсветки поиска.
fn make_header_text(node: &JsonNode, highlight: Highlight, locale: Locale) -> RichText {
    let display_value = match node.value_type {
        JsonValueType::Object => locale.object_count(node.children.len()),
        JsonValueType::Array => locale.array_count(node.children.len()),
        _ => node.display_value.clone(),
    };
    let label = match &node.key {
        Some(k) => format!("{}: {}", k, display_value),
        None => display_value,
    };
    highlight.apply(RichText::new(label), COLOR_KEY)
}

/// Контекстное меню узла с опциями копирования значения, ключа, пути и структуры.
fn context_menu(
    ui: &mut Ui,
    node: &JsonNode,
    mode: AppMode,
    locale: Locale,
    selected_paths: &BTreeSet<String>,
    outcome: &mut TreeOutcome,
) {
    if ui.button(locale.text(TextKey::CopyValue)).clicked() {
        outcome.copy_request = Some(node.display_value.clone());
        ui.close();
    }
    if let Some(key) = &node.key
        && ui.button(locale.text(TextKey::CopyKey)).clicked()
    {
        outcome.copy_request = Some(key.clone());
        ui.close();
    }
    if ui.button(locale.text(TextKey::CopyPath)).clicked() {
        outcome.copy_request = Some(node.path.clone());
        ui.close();
    }
    if ui.button(locale.text(TextKey::CopyStructure)).clicked() {
        outcome.copy_structure_paths = Some(vec![node.path.clone()]);
        ui.close();
    }
    if !selected_paths.is_empty() && ui.button(locale.text(TextKey::CopySelected)).clicked() {
        outcome.copy_structure_paths = Some(selected_paths.iter().cloned().collect());
        ui.close();
    }
    if mode == AppMode::Edit && ui.button(locale.text(TextKey::EditField)).clicked() {
        outcome.edit_field_request = Some(EditFieldRequest {
            path: node.path.clone(),
        });
        ui.close();
    }
}

/// Контекстное меню контейнера с командами копирования и добавления данных.
fn container_context_menu(
    ui: &mut Ui,
    node: &JsonNode,
    mode: AppMode,
    locale: Locale,
    selected_paths: &BTreeSet<String>,
    outcome: &mut TreeOutcome,
) {
    context_menu(ui, node, mode, locale, selected_paths, outcome);

    if mode != AppMode::Edit {
        return;
    }

    if ui.button(locale.text(TextKey::PasteHere)).clicked() {
        outcome.paste_target_path = Some(node.path.clone());
        ui.close();
    }

    let is_object = node.value_type == JsonValueType::Object;
    let label = if is_object {
        locale.text(TextKey::AddField)
    } else {
        locale.text(TextKey::AddElement)
    };
    if ui.button(label).clicked() {
        outcome.add_child_request = Some(AddChildRequest {
            parent_path: node.path.clone(),
            is_object,
        });
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{VisibleRows, focus_match_path, visible_row_index};
    use crate::parser::{parse_json, set_expanded_all};

    #[test]
    fn focus_match_path_reveals_target_and_collapses_other_branches() {
        let mut root = parse_json(
            r#"{"outer":{"inner":{"value":"needle"},"other":{"value":1}},"second":{"value":2}}"#,
        )
        .unwrap();
        set_expanded_all(&mut root, true);

        assert!(focus_match_path(&mut root, "outer.inner.value"));
        assert!(root.expanded);
        let outer = root
            .children
            .iter()
            .find(|node| node.key.as_deref() == Some("outer"))
            .unwrap();
        assert!(outer.expanded);
        assert!(
            outer
                .children
                .iter()
                .find(|node| node.key.as_deref() == Some("inner"))
                .unwrap()
                .expanded
        );
        assert!(
            !outer
                .children
                .iter()
                .find(|node| node.key.as_deref() == Some("other"))
                .unwrap()
                .expanded
        );
        assert!(
            !root
                .children
                .iter()
                .find(|node| node.key.as_deref() == Some("second"))
                .unwrap()
                .expanded
        );
    }

    #[test]
    fn focus_match_path_collapses_tree_for_missing_target() {
        let mut root = parse_json(r#"{"outer":{"value":1}}"#).unwrap();
        set_expanded_all(&mut root, true);

        assert!(!focus_match_path(&mut root, "missing"));
        assert!(!root.expanded);
        assert!(!root.children[0].expanded);
    }

    #[test]
    fn visible_rows_index_respects_expanded_branches() {
        let mut root = parse_json(r#"{"outer":{"value":1},"array":[{"value":2},3]}"#).unwrap();

        assert_eq!(VisibleRows::from_root(&root).len(), 1);

        root.expanded = true;
        assert_eq!(VisibleRows::from_root(&root).len(), 3);

        set_expanded_all(&mut root, true);
        assert_eq!(VisibleRows::from_root(&root).len(), 7);
        assert_eq!(visible_row_index(&root, "array[0].value"), Some(3));
        assert_eq!(visible_row_index(&root, "missing"), None);
    }
}
