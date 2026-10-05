use std::ops::Range;

use egui::{RichText, Ui};

use struct_view_core::parser::{JsonNode, JsonValueType};

use super::super::edit::apply_primitive_edit;
use super::super::i18n::{Locale, TextKey};
use super::super::state::AppMode;
use super::super::theme::SyntaxColors;
use super::super::widgets::single_line_text;
use super::model::{AddChildRequest, InlineEditEvent, RenderOptions, TreeOutcome, VisibleRows};
use super::{container_context_menu, context_menu, selection_request};

mod content;

use content::{Highlight, render_leaf};

const EDIT_FIELD_WIDTH: f32 = 300.0;

/// Высота одной строки дерева без вертикального промежутка между строками.
///
/// `ScrollArea::show_rows` использует фиксированную высоту, поэтому значения
/// контролов дерева должны согласовываться с высотой строки egui.
pub(in crate::app) fn tree_row_height(ui: &Ui) -> f32 {
    ui.spacing().interact_size.y
}

/// Отрисовать диапазон строк раскрытого дерева.
///
/// Вызов обычно выполняется из [`egui::ScrollArea::show_rows`]. Узлы
/// разрешаются по компактному индексу, а виджеты создаются исключительно
/// для строк из диапазона.
pub(in crate::app) fn render_visible_rows(
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
        let id = egui::Id::new(("tree-row", &node.path));
        ui.push_id(id, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            render_node_row(ui, node, depth, options, outcome);
        });
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
    let highlight = Highlight::for_node(options, &node.path);
    let scroll_to_match = options.scroll_to_path.is_some_and(|path| path == node.path);

    match node.value_type {
        JsonValueType::Object | JsonValueType::Array | JsonValueType::Metadata => {
            render_container_row(
                ui,
                node,
                depth,
                options,
                outcome,
                highlight,
                scroll_to_match,
            )
        }
        _ if node
            .children
            .iter()
            .any(|child| child.value_type == JsonValueType::Comment) =>
        {
            render_container_row(
                ui,
                node,
                depth,
                options,
                outcome,
                highlight,
                scroll_to_match,
            )
        }
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

/// Подсветка узла в зависимости от результатов поиска.
fn render_container_row(
    ui: &mut Ui,
    node: &mut JsonNode,
    depth: usize,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
    highlight: Highlight,
    scroll_to_match: bool,
) {
    let colors = SyntaxColors::new(ui.visuals());
    let header_text = make_header_text(node, highlight, options.locale, colors);
    let selected = options.selected_paths.contains(&node.path);
    let previous_expanded = node.expanded;

    let id = ui.make_persistent_id(&node.path);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        node.expanded,
    );
    let add_child = add_child_request(node);

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
        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        let header_response = ui
            .selectable_label(selected, header_text)
            .on_hover_text(&node.display_value);
        let add_clicked = if options.mode == AppMode::Edit {
            add_child.as_ref().is_some_and(|(label, _)| {
                let button_padding = ui.spacing().button_padding;
                ui.spacing_mut().button_padding.y = 0.0;
                let response =
                    ui.add_sized([row_height, row_height], egui::Button::new("+").small());
                ui.spacing_mut().button_padding = button_padding;
                response
                    .on_hover_text(options.locale.text(*label))
                    .clicked()
            })
        } else {
            false
        };
        (header_response, add_clicked)
    });
    let (header_response, add_clicked) = row_response.inner;

    if scroll_to_match {
        header_response.scroll_to_me(Some(egui::Align::Center));
    }

    if header_response.clicked() {
        outcome.selection_request = Some(selection_request(ui, &node.path));
    }

    if add_clicked {
        state.set_open(true);
    }

    // Считываем актуальное состояние (пользователь мог кликнуть по заголовку)
    node.expanded = state.is_open();
    state.store(ui.ctx());
    if node.expanded != previous_expanded {
        outcome.expansion_changed = true;
    }
    if add_clicked && let Some((_, request)) = add_child {
        outcome.add_child_request = Some(request);
    }

    header_response.context_menu(|ui| {
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

pub(super) fn add_child_request(node: &JsonNode) -> Option<(TextKey, AddChildRequest)> {
    let (label, is_object) = match &node.value_type {
        JsonValueType::Object => (TextKey::AddField, true),
        JsonValueType::Array => (TextKey::AddElement, false),
        _ => return None,
    };
    Some((
        label,
        AddChildRequest {
            parent_path: node.path.clone(),
            is_object,
        },
    ))
}

/// Отрисовать листовой узел: ключ и значение (или поле ввода в режиме правки).
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
                | JsonValueType::Float
                | JsonValueType::Bool
                | JsonValueType::Null
        )
}

/// Отрисовать однострочное поле правки значения и применить изменение при потере фокуса.
fn make_header_text(
    node: &JsonNode,
    highlight: Highlight,
    locale: Locale,
    colors: SyntaxColors,
) -> RichText {
    let display_value = match node.value_type {
        JsonValueType::Object => locale.object_count(node.data_child_count()),
        JsonValueType::Array => locale.array_count(node.data_child_count()),
        _ => single_line_text(&node.display_value).into_owned(),
    };
    let label = match &node.key {
        Some(k) => format!("{}: {}", single_line_text(k), display_value),
        None => display_value,
    };
    highlight.apply(
        RichText::new(label),
        colors.value_color(&node.value_type),
        colors,
    )
}
