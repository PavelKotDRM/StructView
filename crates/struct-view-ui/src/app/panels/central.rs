use super::super::widgets::{column_label, single_line_text};
use super::*;

mod tree;

mod comparison;
mod export;

impl StructViewApp {
    fn handle_dropped_files(&mut self, ui: &Ui) {
        let dropped_paths = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        match dropped_paths.as_slice() {
            [] => {}
            [path] => self.request_file_load(path.clone()),
            _ => self.load_comparison(dropped_paths),
        }
    }
}

/// Отрисовать подсказку, показываемую, пока файл не открыт.
fn show_placeholder(ui: &mut Ui, locale: Locale) {
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new(locale.text(TextKey::DropFilePlaceholder))
                .size(18.0)
                .color(Color32::GRAY),
        );
    });
}

/// Отрисовать сообщение об ошибке разбора данных.
fn show_parse_error(ui: &mut Ui, message: &str, locale: Locale) {
    ui.add_space(8.0);
    ui.colored_label(
        SyntaxColors::new(ui.visuals()).error,
        locale.text(TextKey::DataParseError),
    );
    ui.add_space(4.0);
    egui::ScrollArea::both().show(ui, |ui| {
        ui.label(RichText::new(message).monospace());
    });
}
