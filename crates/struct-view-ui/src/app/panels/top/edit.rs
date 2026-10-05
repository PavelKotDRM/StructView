use super::*;

impl StructViewApp {
    pub(super) fn show_edit_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::EditMenu), |ui| {
            if ui.button(locale.text(TextKey::SearchWindow)).clicked() {
                self.search_window_open = true;
                ui.close();
            }
            if self.visualization == VisualizationMode::Structure {
                return;
            }
            if self.root.is_some() {
                ui.menu_button(locale.text(TextKey::Mode), |ui| {
                    let can_edit = self
                        .file_state
                        .format
                        .is_none_or(DataFormat::is_serializable);
                    if ui
                        .selectable_value(
                            &mut self.mode,
                            AppMode::View,
                            locale.text(TextKey::ViewMode),
                        )
                        .changed()
                    {
                        ui.close();
                    }
                    if can_edit
                        && ui
                            .selectable_value(
                                &mut self.mode,
                                AppMode::Edit,
                                locale.text(TextKey::EditMode),
                            )
                            .changed()
                    {
                        ui.close();
                    }
                });
                ui.separator();
            }

            if ui
                .add_enabled(
                    self.can_undo(),
                    egui::Button::new(locale.text(TextKey::Undo)),
                )
                .clicked()
            {
                self.undo_requested = true;
                ui.close();
            }
            if ui
                .add_enabled(
                    self.can_redo(),
                    egui::Button::new(locale.text(TextKey::Redo)),
                )
                .clicked()
            {
                self.redo_requested = true;
                ui.close();
            }
            ui.separator();

            let copy_enabled = !self.selected_paths.is_empty();
            if ui
                .add_enabled(
                    copy_enabled,
                    egui::Button::new(locale.text(TextKey::CopySelectedStructures)),
                )
                .clicked()
            {
                self.copy_structures_requested = true;
                ui.close();
            }

            let paste_enabled = self.mode == AppMode::Edit && self.can_paste_into_selected();
            if ui
                .add_enabled(
                    paste_enabled,
                    egui::Button::new(locale.text(TextKey::PasteSelectedContainer)),
                )
                .clicked()
            {
                self.paste_requested = true;
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    self.can_delete_selected(),
                    egui::Button::new(locale.text(TextKey::DeleteSelectedStructures)),
                )
                .clicked()
            {
                self.delete_requested = true;
                ui.close();
            }
        });
    }
}
