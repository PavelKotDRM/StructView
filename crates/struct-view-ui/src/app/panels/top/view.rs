use super::*;

impl StructViewApp {
    pub(super) fn show_view_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        let comparing = self.is_comparing();
        ui.menu_button(locale.text(TextKey::ViewMenu), |ui| {
            ui.menu_button(locale.text(TextKey::Visualization), |ui| {
                if comparing {
                    for (mode, text_key) in [
                        (VisualizationMode::Comparison, TextKey::ComparisonView),
                        (VisualizationMode::Diff, TextKey::DiffView),
                    ] {
                        if ui
                            .selectable_value(&mut self.visualization, mode, locale.text(text_key))
                            .changed()
                        {
                            ui.close();
                        }
                    }
                } else {
                    for (mode, text_key) in [
                        (VisualizationMode::Tree, TextKey::TreeView),
                        (VisualizationMode::Graph, TextKey::GraphView),
                        (VisualizationMode::Structure, TextKey::StructureView),
                        (VisualizationMode::Table, TextKey::TableView),
                        (VisualizationMode::Schema, TextKey::SchemaView),
                    ] {
                        if ui
                            .selectable_value(&mut self.visualization, mode, locale.text(text_key))
                            .changed()
                        {
                            self.refresh_search();
                            ui.close();
                        }
                    }
                }
            });

            if self.root.is_some() && self.visualization != VisualizationMode::Structure {
                ui.add_enabled_ui(self.graph_calculation.has_started(), |ui| {
                    ui.menu_button(locale.text(TextKey::GraphTimings), |ui| {
                        self.graph_calculation.show_progress(ui, locale);
                    });
                });
            }
            if self.root.is_some() || self.visualization == VisualizationMode::Structure {
                ui.menu_button(locale.text(TextKey::TreeActions), |ui| {
                    if ui.button(locale.text(TextKey::ExpandAll)).clicked() {
                        ui.close();
                        self.set_all_expanded(true);
                    }
                    if ui.button(locale.text(TextKey::CollapseAll)).clicked() {
                        ui.close();
                        self.set_all_expanded(false);
                    }
                });
            }

            if self.visualization == VisualizationMode::Structure {
                ui.separator();
                self.structure_view.view_menu(ui, locale);
            } else if self.visualization == VisualizationMode::Graph {
                ui.separator();
                ui.add_enabled_ui(self.graph_calculation.result().is_some(), |ui| {
                    if let Some(calculation) = self.graph_calculation.result() {
                        crate::app::views::graph_view_menu(ui, &calculation.routing, locale);
                    }
                });
            }
            ui.separator();
            let theme_label = if self.dark_mode {
                locale.text(TextKey::LightTheme)
            } else {
                locale.text(TextKey::DarkTheme)
            };
            if ui.button(theme_label).clicked() {
                ui.close();
                self.dark_mode = !self.dark_mode;
                let ctx = ui.ctx().clone();
                self.apply_theme(&ctx);
            }
        });
    }
}
