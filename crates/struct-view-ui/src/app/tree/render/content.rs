use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct Highlight {
    /// Узел входит в список совпадений.
    is_match: bool,
    /// Узел является текущим активным совпадением.
    is_active: bool,
}

impl Highlight {
    /// Вычислить подсветку для пути узла.
    pub(super) fn for_node(options: &RenderOptions<'_>, path: &str) -> Self {
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
    pub(super) fn apply(
        self,
        text: RichText,
        default_color: egui::Color32,
        colors: SyntaxColors,
    ) -> RichText {
        if self.is_active {
            text.color(colors.active_match).strong()
        } else if self.is_match {
            text.color(colors.matched).strong()
        } else {
            text.color(default_color)
        }
    }
}

pub(super) fn render_leaf(
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
fn value_editor_id(path: &str) -> egui::Id {
    egui::Id::new(("tree-value", path))
}
