use super::*;

impl StructViewApp {
    pub(in crate::app::panels::top) fn open_active_file_dialog(&mut self) {
        if self.visualization == VisualizationMode::Structure {
            self.structure_view.open_file_dialog();
            if let Some(path) = self.structure_view.requested_open.take() {
                self.request_document_replacement(
                    crate::app::state::DocumentReplacement::OpenStructure(path),
                );
            }
        } else {
            self.open_file_dialog();
        }
    }

    pub(super) fn show_file_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::FileMenu), |ui| {
            if self.visualization == VisualizationMode::Graph
                && !self.is_comparing()
                && ui.button(locale.text(TextKey::GraphNew)).clicked()
            {
                ui.close();
                self.request_new_graph();
            }
            if ui
                .add(
                    egui::Button::new(locale.text(TextKey::NewFile)).shortcut_text(
                        ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                            egui::Modifiers::COMMAND,
                            egui::Key::N,
                        )),
                    ),
                )
                .clicked()
            {
                ui.close();
                self.open_new_file_dialog();
            }
            if ui
                .add(egui::Button::new(locale.text(TextKey::Open)).shortcut_text(
                    ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND,
                        egui::Key::O,
                    )),
                ))
                .clicked()
            {
                ui.close();
                self.open_active_file_dialog();
            }
            if self.visualization == VisualizationMode::Structure {
                self.structure_view.file_menu(ui, locale);
                self.structure_view.export_menu(ui, locale);
            } else if self.visualization == VisualizationMode::Graph {
                self.show_graph_export_menu(ui);
            }
            if ui.button(locale.text(TextKey::CompareFiles)).clicked() {
                ui.close();
                self.open_comparison_dialog();
            }
            if self.visualization != VisualizationMode::Structure {
                if ui
                    .add_enabled(
                        self.root.is_some()
                            && self.visualization != VisualizationMode::Structure
                            && self
                                .file_state
                                .format
                                .is_none_or(DataFormat::is_serializable),
                        egui::Button::new(locale.text(TextKey::Save)).shortcut_text(
                            ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                                egui::Modifiers::COMMAND,
                                egui::Key::S,
                            )),
                        ),
                    )
                    .clicked()
                {
                    ui.close();
                    self.request_save_current();
                }
                if ui
                    .add_enabled(
                        self.root.is_some() && self.visualization != VisualizationMode::Structure,
                        egui::Button::new(locale.text(TextKey::SaveAs)).shortcut_text(
                            ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                                egui::Key::S,
                            )),
                        ),
                    )
                    .clicked()
                {
                    ui.close();
                    self.save_as();
                }
                ui.menu_button(locale.text(TextKey::ConvertTo), |ui| {
                    let current_format = self.file_state.format;
                    for format in DataFormat::ALL {
                        if current_format == Some(format) {
                            continue;
                        }

                        if ui
                            .add_enabled(
                                self.root.is_some()
                                    && self.visualization != VisualizationMode::Structure,
                                egui::Button::new(format.to_string()),
                            )
                            .clicked()
                        {
                            ui.close();
                            self.convert_to_format(format);
                        }
                    }
                });
            }
            if ui
                .add(
                    egui::Button::new(locale.text(TextKey::CloseFile)).shortcut_text(
                        ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                            egui::Modifiers::COMMAND,
                            egui::Key::W,
                        )),
                    ),
                )
                .clicked()
            {
                ui.close();
                self.request_close_file();
            }
            ui.separator();
            if ui.button(locale.text(TextKey::Exit)).clicked() {
                ui.close();
                self.request_exit(ui.ctx());
            }
        });
    }
}
