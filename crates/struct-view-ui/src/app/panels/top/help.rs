use super::*;

impl StructViewApp {
    pub(super) fn show_help_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::HelpMenu), |ui| {
            ui.menu_button(locale.text(TextKey::DiagramLegend), |ui| {
                ui.set_max_width(480.0);
                egui::ScrollArea::vertical()
                    .max_height(ui.ctx().content_rect().height() * 0.75)
                    .show(ui, |ui| {
                        crate::app::views::structure::show_legend(ui, locale);
                        ui.separator();
                        crate::app::views::diagram::show_controls_help(ui, locale);
                        ui.separator();
                        ui.label(locale.text(TextKey::StructureDisk));
                    });
            });
            ui.menu_button(locale.text(TextKey::BuildInformation), |ui| {
                ui.label(format!("StructView {}", build_info::VERSION));
                ui.separator();
                egui::Grid::new("about_build_info")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        for (name, value) in [
                            (locale.text(TextKey::BuildTime), build_info::BUILD_TIMESTAMP),
                            (
                                locale.text(TextKey::TargetPlatform),
                                build_info::TARGET_TRIPLE,
                            ),
                            (locale.text(TextKey::HostPlatform), build_info::HOST_TRIPLE),
                            (
                                locale.text(TextKey::OptimizationLevel),
                                build_info::OPT_LEVEL,
                            ),
                            (locale.text(TextKey::DebugBuild), build_info::DEBUG),
                            (
                                locale.text(TextKey::RustcCompiler),
                                build_info::RUSTC_SEMVER,
                            ),
                            (
                                locale.text(TextKey::RustcChannel),
                                build_info::RUSTC_CHANNEL,
                            ),
                        ] {
                            ui.label(name);
                            ui.label(RichText::new(value).monospace());
                            ui.end_row();
                        }
                    });
            });
        });
    }
}
