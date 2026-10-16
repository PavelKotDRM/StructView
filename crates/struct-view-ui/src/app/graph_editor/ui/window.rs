use super::dialog::text;
use super::*;

impl StructViewApp {
    pub(in crate::app) fn can_edit_graph(&self) -> bool {
        self.visualization == VisualizationMode::Graph
            && self.mode == AppMode::Edit
            && self.comparison.is_none()
            && self.root.as_ref().is_some_and(|root| {
                GraphDocument::read(root).is_ok_and(|document| document.is_some())
            })
    }

    pub(in crate::app) fn show_graph_edit_commands(&mut self, ui: &mut egui::Ui) {
        if !self.can_edit_graph() {
            return;
        }
        let document = match self.root.as_ref().map(GraphDocument::read).transpose() {
            Ok(Some(Some(document))) => document,
            Ok(_) => return,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };
        self.graph_editor
            .show_commands(ui, &document, self.locale, true);
    }

    pub(in crate::app) fn show_graph_editor(&mut self, ctx: &egui::Context) {
        if !self.can_edit_graph() || !self.graph_editor.open {
            self.graph_editor.docking.reset_if_active();
            return;
        }
        let document = match self.root.as_ref().map(GraphDocument::read).transpose() {
            Ok(Some(Some(document))) => document,
            Ok(_) => return,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };
        let locale = self.locale;
        let mut docking = std::mem::take(&mut self.graph_editor.docking);
        docking.begin();
        let mut show_content = |ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                self.graph_editor
                    .show_commands(ui, &document, locale, false);
            });
            ui.separator();
            ui.add_enabled_ui(!self.graph_editor.is_busy(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (index, key) in [
                        TextKey::GraphNodes,
                        TextKey::GraphEdges,
                        TextKey::GraphGroups,
                        TextKey::GraphSettings,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.selectable_value(&mut self.graph_editor.tab, index, locale.text(key));
                    }
                });
                ui.separator();
                match self.graph_editor.tab {
                    0 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-nodes")
                            .show(ui, |ui| {
                                for (index, record) in document.nodes.iter().enumerate() {
                                    if ui
                                        .button(format!(
                                            "{}  {}",
                                            text(record, "id"),
                                            text(record, "label")
                                        ))
                                        .clicked()
                                    {
                                        self.graph_editor
                                            .open(&document, Target::Node(Some(index)));
                                    }
                                }
                            });
                    }
                    1 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-edges")
                            .show(ui, |ui| {
                                for (index, record) in document.edges.iter().enumerate() {
                                    if ui
                                        .button(format!(
                                            "{}: {} -> {}  {}",
                                            text(record, "id"),
                                            text(record, "source"),
                                            text(record, "target"),
                                            text(record, "label")
                                        ))
                                        .clicked()
                                    {
                                        self.graph_editor
                                            .open(&document, Target::Edge(Some(index)));
                                    }
                                }
                            });
                    }
                    2 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-groups")
                            .show(ui, |ui| {
                                for (index, name) in document.partitions.iter().enumerate() {
                                    if ui.button(name).clicked() {
                                        self.graph_editor
                                            .open(&document, Target::Partition(Some(index)));
                                    }
                                }
                            });
                    }
                    _ => {
                        ui.label(text(&Value::Object(document.metadata.clone()), "name"));
                        ui.label(locale.text(TextKey::GraphSettingsHint));
                    }
                }
                ui.separator();
                ui.weak(locale.text(TextKey::GraphEditorHint));
                ui.weak(locale.text(TextKey::GraphAutomaticLayout));
            });
        };
        let action = if docking.detached {
            show_detached_viewport(
                ctx,
                &mut docking,
                egui::ViewportId::from_hash_of("graph-editor-window"),
                locale.text(TextKey::GraphEditor),
                egui::vec2(420.0, 440.0),
                locale.text(TextKey::DockWindow),
                &mut show_content,
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(locale.text(TextKey::GraphEditor))
                .id(docking.area_id("graph-editor-window"))
                .open(&mut open)
                .default_size(egui::vec2(420.0, 440.0))
                .collapsible(true)
                .resizable(true);
            if let Some(size) = docking.embedded_size() {
                window = window.default_size(size);
            }
            if let Some(position) = docking.take_embedded_position() {
                window = window.current_pos(position);
            }
            let mut detach = false;
            let response = window.show(ctx, |ui| {
                detach = show_docking_controls(
                    ui,
                    false,
                    locale.text(TextKey::DetachWindow),
                    locale.text(TextKey::WindowOptions),
                );
                show_content(ui);
            });
            if !open {
                docking.reset_if_active();
                DetachedViewportAction::Closed
            } else {
                if let Some(response) = response {
                    docking.remember_embedded_size(response.response.rect.size());
                    if detach {
                        let origin =
                            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min));
                        docking.detach(ctx.viewport_rect(), response.response.rect, origin);
                        ctx.request_repaint();
                    }
                }
                DetachedViewportAction::KeepOpen
            }
        };
        drop(show_content);
        self.graph_editor.docking = docking;
        if action == DetachedViewportAction::Closed {
            self.graph_editor.open = false;
        }
    }

    pub(in crate::app) fn handle_graph_canvas_edit(&mut self, ctx: &egui::Context) {
        let request = ctx.data_mut(|data| {
            let id = egui::Id::unique("graph-editor-canvas-request");
            let request = data.get_temp::<CanvasTarget>(id);
            data.remove::<CanvasTarget>(id);
            request
        });
        if self.mode != AppMode::Edit || self.graph_editor.is_busy() {
            return;
        }
        if let Some(request) = request {
            match self.root.as_ref().map(GraphDocument::read).transpose() {
                Ok(Some(Some(document))) => {
                    let target = match request {
                        CanvasTarget::Node(index) if index < document.nodes.len() => {
                            Target::Node(Some(index))
                        }
                        CanvasTarget::Edge(index) if index < document.edges.len() => {
                            Target::Edge(Some(index))
                        }
                        _ => return,
                    };
                    self.graph_editor.open(&document, target);
                }
                Err(error) => self.show_error(&error),
                _ => {}
            }
        }
    }
}
