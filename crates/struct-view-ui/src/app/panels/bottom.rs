use super::*;

impl StructViewApp {
    /// Отрисовать нижнюю панель (статус-бар) с информацией о файле и уведомлениями.
    pub(in crate::app) fn show_bottom_panel(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.parse_error {
                    ui.label(
                        RichText::new(format!("⚠ {}: {}", locale.text(TextKey::Error), err))
                            .color(COLOR_ERROR),
                    );
                } else if let Some(comparison) = &self.comparison {
                    ui.label(locale.comparison_status(
                        comparison.documents.len(),
                        comparison.differences.len(),
                    ));
                } else if let Some(path) = &self.file_state.path {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(locale.text(TextKey::UnknownFile));
                    let size_kb = self.file_state.size_bytes as f64 / 1024.0;
                    let format = self
                        .file_state
                        .format
                        .map(|format| format.to_string())
                        .unwrap_or_else(|| locale.text(TextKey::UnknownFormat).to_string());
                    ui.label(locale.loaded_file_status(
                        name,
                        &format,
                        size_kb,
                        self.file_state.load_time_ms,
                    ));
                } else {
                    ui.label(RichText::new(locale.text(TextKey::Placeholder)).color(Color32::GRAY));
                }

                self.show_toast_label(ui);
            });
        });
    }

    /// Отрисовать всплывающее уведомление, если оно ещё не устарело.
    fn show_toast_label(&mut self, ui: &mut Ui) {
        let Some((message, shown_at)) = &self.toast else {
            return;
        };
        if shown_at.elapsed().as_secs() >= TOAST_LIFETIME_SECS {
            self.toast = None;
            return;
        }
        let text = RichText::new(format!("✔ {}", message)).color(COLOR_SUCCESS);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(text);
        });
    }
}
