use super::*;

impl StructViewApp {
    pub(super) fn show_graph_export_menu(&mut self, ui: &mut Ui) {
        let mut requested = None;
        ui.add_enabled_ui(self.graph_calculation.result().is_some(), |ui| {
            requested = crate::app::views::diagram::export_controls(ui, self.locale);
        });
        if let Some((png, dark)) = requested
            && let Some(calculation) = self.graph_calculation.result()
        {
            let result = export_graph_image(
                &calculation.graph,
                &calculation.routing,
                if png {
                    crate::app::views::GraphExportFormat::Png
                } else {
                    crate::app::views::GraphExportFormat::Svg
                },
                if dark {
                    crate::app::views::GraphExportStyle::DarkOpaque
                } else {
                    crate::app::views::GraphExportStyle::LightTransparent
                },
                self.locale,
            );
            match result {
                Ok(true) => self.show_toast(self.locale.text(TextKey::GraphExported)),
                Ok(false) => {}
                Err(error) => self.show_error(&self.locale.save_error(&error.to_string())),
            }
        }
    }
}
