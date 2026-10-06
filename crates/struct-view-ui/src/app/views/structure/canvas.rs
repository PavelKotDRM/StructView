use super::*;

impl StructureView {
    pub(super) fn canvas(&mut self, ui: &mut Ui, locale: Locale) {
        let (_, viewport) = ui.allocate_space(ui.available_size().max(Vec2::splat(1.0)));
        let response = ui.interact(
            viewport,
            ui.id().with("structure-canvas"),
            Sense::click_and_drag(),
        );
        self.canvas_size = viewport.size();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                locale.text(TextKey::StructureView),
            )
        });
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.dragged() {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if response.contains_pointer() {
            let factor = super::super::diagram::wheel_zoom(ui);
            if let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
                && factor != 1.0
            {
                self.zoom_by(factor, pointer - viewport.min.to_vec2());
            }
        }
        if response.has_focus() {
            self.keyboard(ui);
        }
        self.ensure_layout();
        if self.fit {
            super::super::diagram::fit_to(
                &mut self.zoom,
                &mut self.pan,
                viewport.size(),
                self.layout.size,
            );
            self.fit = false;
        }
        if self.center_selected {
            if let Some((_, rect)) = self
                .layout
                .nodes
                .iter()
                .find(|(id, _)| *id == self.selected)
            {
                self.pan = viewport.size() * 0.5 - rect.center().to_vec2() * self.zoom;
            }
            self.center_selected = false;
        }
        let painter = ui.painter_at(viewport);
        let transform = |point: Pos2| viewport.min + self.pan + point.to_vec2() * self.zoom;
        for &(parent, child, points) in &self.layout.edges {
            let points = points.map(transform);
            if !Rect::from_points(&points).expand(3.0).intersects(viewport) {
                continue;
            }
            let highlighted = self.all_selected
                || (self.path.contains(&parent) && self.path.contains(&child))
                || (self.matched_paths.contains(&parent) && self.matched_paths.contains(&child));
            painter.add(egui::Shape::line(
                points.to_vec(),
                Stroke::new(
                    if highlighted { 2.5 } else { 1.3 },
                    if highlighted {
                        Color32::from_rgb(240, 170, 40)
                    } else {
                        ui.visuals().text_color()
                    },
                ),
            ));
        }
        let Some(doc) = &self.document else {
            return;
        };
        let mut clicked = None;
        let mut expansion_requested = None;
        let mut add_requested = None;
        let mut edit_requested = false;
        let mut delete_requested = false;
        let mut focused = None;
        for &(id, rect) in &self.layout.nodes {
            let rect = Rect::from_min_max(transform(rect.min), transform(rect.max));
            if !rect.intersects(viewport) {
                continue;
            }
            let node = &doc.nodes[id];
            let color = kind_color(node.kind);
            painter.rect_filled(
                rect,
                if node.kind == Kind::Array {
                    14.0 * self.zoom
                } else {
                    5.0 * self.zoom
                },
                Color32::from_rgb(23, 29, 40),
            );
            let highlight = self.all_selected
                || self.path.contains(&id)
                || self.matched_paths.contains(&id)
                || self.selected_nodes.contains(&id);
            painter.rect_stroke(
                rect,
                5.0 * self.zoom,
                Stroke::new(
                    if self.selected_nodes.contains(&id) {
                        3.0
                    } else {
                        1.5
                    },
                    if highlight {
                        Color32::from_rgb(255, 198, 64)
                    } else {
                        color
                    },
                ),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center_top() + Vec2::new(0.0, 8.0 * self.zoom),
                Align2::CENTER_TOP,
                shorten(&node.key, 24),
                FontId::proportional(14.0 * self.zoom),
                KEY_COLOR,
            );
            painter.text(
                rect.center_bottom() - Vec2::new(0.0, 10.0 * self.zoom),
                Align2::CENTER_BOTTOM,
                node_label(
                    node,
                    self.collapsed.contains(&id),
                    self.limits.get(&id).copied().unwrap_or(100),
                ),
                FontId::proportional(12.0 * self.zoom),
                color,
            );
            let hit = ui.interact(
                rect.intersect(viewport),
                ui.id().with(("structure-node", id)),
                Sense::click(),
            );
            hit.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    true,
                    self.all_selected || self.selected_nodes.contains(&id),
                    format!(
                        "{}: {} ({}, {})",
                        node.key,
                        node.value,
                        kind_name(node.kind, locale),
                        node.path
                    ),
                )
            });
            if !node.children.is_empty() {
                ui.ctx().accesskit_node_builder(hit.id, |item| {
                    item.set_expanded(!self.collapsed.contains(&id));
                    item.set_description(locale.text(TextKey::StructureToggle));
                });
            }
            let hit = hit.on_hover_text(format!("{}: {}\n{}", node.key, node.value, node.path));
            hit.context_menu(|ui| {
                if !self.editing || !self.can_edit() || node.parent.is_none() {
                    return;
                }
                if ui.button(locale.text(TextKey::EditField)).clicked() {
                    clicked = Some(id);
                    edit_requested = true;
                    ui.close();
                }
                if ui
                    .button(locale.text(TextKey::DeleteSelectedStructures))
                    .clicked()
                {
                    clicked = Some(id);
                    delete_requested = true;
                    ui.close();
                }
                if matches!(node.kind, Kind::Object | Kind::Array) {
                    let label = if node.kind == Kind::Object {
                        TextKey::AddField
                    } else {
                        TextKey::AddElement
                    };
                    if ui.button(locale.text(label)).clicked() {
                        clicked = Some(id);
                        add_requested = Some(id);
                        ui.close();
                    }
                }
            });
            if !node.children.is_empty() {
                let button_size = 18.0 * self.zoom;
                let button_rect = Rect::from_center_size(
                    rect.left_top() + Vec2::new(button_size * 0.75, button_size * 0.75),
                    Vec2::splat(button_size),
                );
                let button_color = if self.collapsed.contains(&id) {
                    ui.visuals().widgets.inactive.bg_fill
                } else {
                    ui.visuals().widgets.hovered.bg_fill
                };
                painter.circle_filled(button_rect.center(), button_size * 0.48, button_color);
                let center = button_rect.center();
                let arm = button_size * 0.22;
                let stroke = Stroke::new((button_size * 0.1).max(1.0), ui.visuals().text_color());
                painter.line_segment([center - Vec2::X * arm, center + Vec2::X * arm], stroke);
                if self.collapsed.contains(&id) {
                    painter.line_segment([center - Vec2::Y * arm, center + Vec2::Y * arm], stroke);
                }
                let expand = ui.interact(
                    button_rect.intersect(viewport),
                    ui.id().with(("structure-expand-node", id)),
                    Sense::click(),
                );
                expand.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        locale.text(TextKey::StructureToggle),
                    )
                });
                if expand
                    .on_hover_text(locale.text(TextKey::StructureToggle))
                    .clicked()
                {
                    expansion_requested = Some(id);
                    response.request_focus();
                }
            }
            if self.can_edit() && self.editing && matches!(node.kind, Kind::Object | Kind::Array) {
                let button_size = 18.0 * self.zoom;
                let button_rect = Rect::from_center_size(
                    rect.right_top() + Vec2::new(-button_size * 0.7, button_size * 0.7),
                    Vec2::splat(button_size),
                );
                painter.circle_filled(
                    button_rect.center(),
                    button_size * 0.48,
                    ui.visuals().widgets.active.bg_fill,
                );
                let center = button_rect.center();
                let arm = button_size * 0.22;
                let stroke = Stroke::new((button_size * 0.1).max(1.0), ui.visuals().text_color());
                painter.line_segment([center - Vec2::X * arm, center + Vec2::X * arm], stroke);
                painter.line_segment([center - Vec2::Y * arm, center + Vec2::Y * arm], stroke);
                let add = ui.interact(
                    button_rect.intersect(viewport),
                    ui.id().with(("structure-add-node", id)),
                    Sense::click(),
                );
                add.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        locale.text(if node.kind == Kind::Object {
                            TextKey::AddField
                        } else {
                            TextKey::AddElement
                        }),
                    )
                });
                if add.clicked() {
                    clicked = Some(id);
                    add_requested = Some(id);
                    response.request_focus();
                }
            }
            if hit.has_focus() {
                focused = Some(id);
            }
            if hit.clicked() && !super::super::diagram::keyboard_activate(ui) {
                clicked = Some(id);
                response.request_focus();
            }
        }
        if let Some(id) = clicked {
            let is_action = edit_requested || delete_requested || add_requested.is_some();
            if is_action {
                self.select(id, false);
            } else {
                self.select_canvas_node(
                    id,
                    ui.input(|input| input.modifiers.command || input.modifiers.ctrl),
                );
            }
            ui.ctx().request_repaint();
        }
        if let Some(id) = add_requested {
            self.select(id, false);
            self.open_add_dialog();
        } else if edit_requested {
            self.open_edit_dialog();
        } else if delete_requested {
            self.delete_selected(locale);
        }
        if let Some(id) = expansion_requested {
            self.toggle_expansion(id);
        }
        if let Some(id) = focused {
            self.selected = id;
            self.keyboard(ui);
            if ui.input(|i| {
                [
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::Enter,
                    egui::Key::Space,
                ]
                .iter()
                .any(|&key| i.key_pressed(key))
            }) {
                response.request_focus();
            }
        }
        if response.clicked() && clicked.is_none() && !super::super::diagram::keyboard_activate(ui)
        {
            self.selected_nodes.clear();
            self.source_preview = None;
        }
    }
}
