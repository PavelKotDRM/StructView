//! Рекурсивная отрисовка дерева JSON и контекстное меню узла.

use egui::{RichText, Ui};

use crate::parser::{JsonNode, JsonValueType};
use crate::search::SearchState;

use super::edit::apply_primitive_edit;
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
    /// Сообщение об ошибке правки значения.
    pub(super) edit_error: Option<String>,
    /// Признак того, что дерево было изменено и поиск нужно пересчитать.
    pub(super) tree_changed: bool,
    /// Запрос на открытие диалога добавления поля или элемента.
    pub(super) add_child_request: Option<AddChildRequest>,
}

/// Контейнер, в который пользователь хочет добавить данные.
#[derive(Debug, Clone)]
pub(super) struct AddChildRequest {
    /// Путь контейнера в дереве.
    pub(super) parent_path: String,
    /// `true`, если контейнер является объектом и требуется имя поля.
    pub(super) is_object: bool,
}

/// Рекурсивно отрисовать узел JSON в [`Ui`].
///
/// Объекты и массивы отображаются как раскрывающийся [`egui::collapsing_header::CollapsingState`].
/// Листовые узлы отображаются как строки с цветной подписью типа; в режиме
/// [`AppMode::Edit`] примитивные значения доступны для правки.
///
/// При правом клике на узел показывается контекстное меню с опциями копирования.
///
/// # Arguments
///
/// * `ui` — текущий [`Ui`]-контекст egui.
/// * `node` — узел для отрисовки.
/// * `search` — текущее состояние поиска (для подсветки).
/// * `mode` — режим просмотра или редактирования.
/// * `scroll_to_path` — путь узла, к которому нужно прокрутить дерево.
/// * `outcome` — накопитель отложенных действий.
pub(super) fn render_node(
    ui: &mut Ui,
    node: &mut JsonNode,
    search: &SearchState,
    mode: AppMode,
    scroll_to_path: Option<&str>,
    outcome: &mut TreeOutcome,
) {
    let highlight = Highlight::for_node(search, &node.path);
    let scroll_to_match = scroll_to_path.is_some_and(|path| path == node.path);

    match node.value_type {
        JsonValueType::Object | JsonValueType::Array => render_container(
            ui,
            node,
            search,
            mode,
            outcome,
            highlight,
            scroll_to_match,
            scroll_to_path,
        ),
        _ => render_leaf(ui, node, mode, outcome, highlight, scroll_to_match),
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

/// Отрисовать объект или массив как раскрывающийся блок с дочерними узлами.
fn render_container(
    ui: &mut Ui,
    node: &mut JsonNode,
    search: &SearchState,
    mode: AppMode,
    outcome: &mut TreeOutcome,
    highlight: Highlight,
    scroll_to_match: bool,
    scroll_to_path: Option<&str>,
) {
    let header_text = make_header_text(node, highlight);

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

    let (header_resp, _inner, _body) = state
        .show_header(ui, |ui| {
            ui.label(header_text);
        })
        .body(|ui| {
            for child in &mut node.children {
                render_node(ui, child, search, mode, scroll_to_path, outcome);
            }
        });

    if scroll_to_match {
        header_resp.scroll_to_me(Some(egui::Align::Center));
    }

    // Считываем актуальное состояние (пользователь мог кликнуть по заголовку)
    let updated = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        node.expanded,
    );
    node.expanded = updated.is_open();

    header_resp.context_menu(|ui| {
        container_context_menu(ui, node, mode, outcome);
    });
}

/// Отрисовать листовой узел: ключ и значение (или поле ввода в режиме правки).
fn render_leaf(
    ui: &mut Ui,
    node: &mut JsonNode,
    mode: AppMode,
    outcome: &mut TreeOutcome,
    highlight: Highlight,
    scroll_to_match: bool,
) {
    let row_response = ui.horizontal(|ui| {
        if let Some(key) = &node.key {
            let key_text = highlight.apply(RichText::new(format!("{}: ", key)), COLOR_KEY);
            let key_resp = ui.label(key_text);
            key_resp.context_menu(|ui| {
                context_menu(ui, node, &mut outcome.copy_request);
            });
        }

        if is_editable(node, mode) {
            render_value_editor(ui, node, outcome);
        } else {
            let value_text = highlight.apply(
                RichText::new(&node.display_value),
                value_color(&node.value_type),
            );
            let value_resp = ui.label(value_text);
            value_resp.context_menu(|ui| {
                context_menu(ui, node, &mut outcome.copy_request);
            });
        }
    });

    if scroll_to_match {
        row_response
            .response
            .scroll_to_me(Some(egui::Align::Center));
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
fn render_value_editor(ui: &mut Ui, node: &mut JsonNode, outcome: &mut TreeOutcome) {
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
            Err(err) => outcome.edit_error = Some(err),
        }
    }

    edit_resp.context_menu(|ui| {
        context_menu(ui, node, &mut outcome.copy_request);
    });
}

/// Сформировать текст заголовка для объекта/массива с учётом подсветки поиска.
fn make_header_text(node: &JsonNode, highlight: Highlight) -> RichText {
    let label = match &node.key {
        Some(k) => format!("{}: {}", k, node.display_value),
        None => node.display_value.clone(),
    };
    highlight.apply(RichText::new(label), COLOR_KEY)
}

/// Контекстное меню узла с опциями копирования значения, ключа и пути.
fn context_menu(ui: &mut Ui, node: &JsonNode, copy_request: &mut Option<String>) {
    if ui.button("📋  Копировать значение").clicked() {
        *copy_request = Some(node.display_value.clone());
        ui.close();
    }
    if let Some(key) = &node.key
        && ui.button("🔑  Копировать ключ").clicked()
    {
        *copy_request = Some(key.clone());
        ui.close();
    }
    if ui.button("📍  Копировать путь").clicked() {
        *copy_request = Some(node.path.clone());
        ui.close();
    }
}

/// Контекстное меню контейнера с командами копирования и добавления данных.
fn container_context_menu(ui: &mut Ui, node: &JsonNode, mode: AppMode, outcome: &mut TreeOutcome) {
    context_menu(ui, node, &mut outcome.copy_request);

    if mode != AppMode::Edit {
        return;
    }

    let is_object = node.value_type == JsonValueType::Object;
    let label = if is_object {
        "➕  Добавить поле…"
    } else {
        "➕  Добавить элемент…"
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
    use super::focus_match_path;
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
}
