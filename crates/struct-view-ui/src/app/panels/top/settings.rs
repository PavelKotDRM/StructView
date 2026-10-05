use super::*;

impl StructViewApp {
    pub(super) fn show_settings_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::SettingsMenu), |ui| {
            if self.visualization == VisualizationMode::Graph {
                let previous_setting = self.graph_routing_workers;
                let automatic_count = crate::app::views::graph_routing_worker_count(
                    usize::MAX,
                    crate::app::views::GraphRoutingWorkerSetting::Automatic,
                );
                ui.label(locale.text(TextKey::GraphRoutingWorkers));
                ui.selectable_value(
                    &mut self.graph_routing_workers,
                    crate::app::views::GraphRoutingWorkerSetting::Automatic,
                    format!(
                        "{} ({automatic_count})",
                        locale.text(TextKey::GraphRoutingAutomatic)
                    ),
                );
                let maximum = crate::app::views::available_graph_routing_workers();
                let mut manual_count = match self.graph_routing_workers {
                    crate::app::views::GraphRoutingWorkerSetting::Manual(count) => count,
                    crate::app::views::GraphRoutingWorkerSetting::Automatic => automatic_count,
                };
                ui.horizontal(|ui| {
                    let manual_selected = matches!(
                        self.graph_routing_workers,
                        crate::app::views::GraphRoutingWorkerSetting::Manual(_)
                    );
                    let manual_clicked = ui
                        .selectable_label(manual_selected, locale.text(TextKey::GraphRoutingCustom))
                        .clicked();
                    let count_changed = ui
                        .add(egui::DragValue::new(&mut manual_count).range(1..=maximum))
                        .changed();
                    if manual_clicked || count_changed {
                        self.graph_routing_workers =
                            crate::app::views::GraphRoutingWorkerSetting::Manual(manual_count);
                    }
                });
                if self.graph_routing_workers != previous_setting {
                    self.graph_calculation = crate::app::views::GraphCalculationState::default();
                }
                ui.separator();
            }
            if self.visualization == VisualizationMode::Structure {
                self.structure_view.settings_menu(ui, locale);
                ui.separator();
            }
            ui.label(locale.text(TextKey::Language));
            for available_locale in Locale::ALL {
                if ui
                    .selectable_label(locale == available_locale, available_locale.language_name())
                    .clicked()
                {
                    self.locale = available_locale;
                    ui.close();
                }
            }
        });
    }
}
