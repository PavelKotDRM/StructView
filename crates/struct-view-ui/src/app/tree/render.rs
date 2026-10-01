use std::ops::Range;

use egui::{RichText, Ui};

use struct_view_core::parser::{JsonNode, JsonValueType};

use super::super::edit::apply_primitive_edit;
use super::super::i18n::Locale;
use super::super::state::AppMode;
use super::super::theme::SyntaxColors;
use super::super::widgets::single_line_text;
use super::model::{InlineEditEvent, RenderOptions, TreeOutcome, VisibleRows};
use super::{container_context_menu, context_menu, selection_request};

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
#[derive(Debug, Clone, Copy)]
struct Highlight {
    /// Узел входит в список совпадений.
    is_match: bool,
    /// Узел является текущим активным совпадением.
    is_active: bool,
}

impl Highlight {
    /// Вычислить подсветку для пути узла.
    fn for_node(options: &RenderOptions<'_>, path: &str) -> Self {
        let search = options.search;
        if search.query.is_empty() {
            return Self {
                is_match: false,
                is_active: false,
            };
        }
        Self {
            is_match: options.matching_paths.contains(path),
            is_active: search.is_active(path),
        }
    }

    /// Применить цвет подсветки к тексту, если узел найден поиском.
    ///
    /// Если узел не является совпадением, используется `default_color`.
    fn apply(self, text: RichText, default_color: egui::Color32, colors: SyntaxColors) -> RichText {
        if self.is_active {
            text.color(colors.active_match).strong()
        } else if self.is_match {
            text.color(colors.matched).strong()
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
        ui.selectable_label(selected, header_text)
            .on_hover_text(&node.display_value)
    });
    let header_response = row_response.inner;

    if scroll_to_match {
        header_response.scroll_to_me(Some(egui::Align::Center));
    }

    if header_response.clicked() {
        outcome.selection_request = Some(selection_request(ui, &node.path));
    }

    // Считываем актуальное состояние (пользователь мог кликнуть по заголовку)
    node.expanded = state.is_open();
    state.store(ui.ctx());
    if node.expanded != previous_expanded {
        outcome.expansion_changed = true;
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
    let colors = SyntaxColors::new(ui.visuals());
    let row_response = ui.horizontal(|ui| {
        let row_height = tree_row_height(ui);
        ui.set_min_height(row_height);
        add_tree_indent(ui, depth);

        if let Some(key) = &node.key {
            let key_text = highlight.apply(
                RichText::new(format!("{}: ", single_line_text(key))),
                colors.key,
                colors,
            );
            let key_resp = ui.selectable_label(selected, key_text).on_hover_text(key);
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
            render_value_editor(ui, node, options, outcome);
        } else {
            let value_text = highlight.apply(
                RichText::new(single_line_text(&node.display_value)),
                colors.value_color(&node.value_type),
                colors,
            );
            let value_resp = ui
                .selectable_label(selected, value_text)
                .on_hover_text(&node.display_value);
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
                | JsonValueType::Float
                | JsonValueType::Bool
                | JsonValueType::Null
        )
}

/// Отрисовать однострочное поле правки значения и применить изменение при потере фокуса.
fn render_value_editor(
    ui: &mut Ui,
    node: &mut JsonNode,
    options: &RenderOptions<'_>,
    outcome: &mut TreeOutcome,
) {
    let before_value_type = node.value_type.clone();
    let before_display_value = node.display_value.clone();
    let edit_resp = ui.add(
        egui::TextEdit::singleline(&mut node.display_value)
            .id(value_editor_id(&node.path))
            .desired_width(EDIT_FIELD_WIDTH)
            .font(egui::TextStyle::Monospace)
            .text_color(SyntaxColors::new(ui.visuals()).value_color(&node.value_type)),
    );

    if edit_resp.changed() {
        outcome.tree_changed = true;
    }

    let mut valid = true;
    if edit_resp.lost_focus() {
        let edited = node.display_value.clone();
        match apply_primitive_edit(node, &edited, options.format) {
            Ok(()) => {
                if node.value_type != before_value_type
                    || node.display_value != before_display_value
                {
                    outcome.tree_changed = true;
                }
            }
            Err(err) => {
                node.value_type = before_value_type.clone();
                node.display_value = before_display_value.clone();
                valid = false;
                outcome.edit_error = Some(err);
            }
        }
    }

    if edit_resp.changed() || edit_resp.lost_focus() {
        outcome.inline_edit_events.push(InlineEditEvent {
            path: node.path.clone(),
            before_value_type,
            before_display_value,
            changed: edit_resp.changed(),
            finished: edit_resp.lost_focus(),
            valid,
        });
    }

    if edit_resp.clicked() {
        outcome.selection_request = Some(selection_request(ui, &node.path));
    }
    edit_resp.context_menu(|ui| {
        context_menu(
            ui,
            node,
            AppMode::Edit,
            options.locale,
            options.selected_paths,
            outcome,
        );
    });
}

/// Сформировать текст заголовка для объекта/массива с учётом подсветки поиска.
pub(in crate::app) fn value_editor_id(path: &str) -> egui::Id {
    egui::Id::new(("tree-value", path))
}

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
