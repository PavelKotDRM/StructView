use super::*;

#[path = "edit.rs"]
mod edit;
#[path = "file.rs"]
mod file;
#[path = "graph.rs"]
mod graph;
#[path = "help.rs"]
mod help;
#[path = "settings.rs"]
mod settings;
#[path = "view.rs"]
mod view;

impl StructViewApp {
    pub(super) fn show_menu_bar(&mut self, ui: &mut Ui, compact: bool) {
        let locale = self.locale;
        if compact {
            ui.menu_button(locale.text(TextKey::MainMenu), |ui| {
                self.show_file_menu(ui);
                ui.separator();
                self.show_edit_menu(ui);
                ui.separator();
                self.show_view_menu(ui);
                ui.separator();
                self.show_settings_menu(ui);
                ui.separator();
                self.show_help_menu(ui);
            });
        } else {
            self.show_file_menu(ui);
            self.show_edit_menu(ui);
            self.show_view_menu(ui);
            self.show_settings_menu(ui);
            self.show_help_menu(ui);
        }
    }
}
