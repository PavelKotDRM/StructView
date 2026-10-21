use super::*;

impl StructViewApp {
    pub(in crate::app::panels::top) fn handle_help_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.shortcuts_help_open = true;
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
        }
        if self.shortcuts_help_open
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.shortcuts_help_open = false;
        }
    }

    pub(in crate::app::panels::top) fn show_shortcuts_help(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_help_open {
            return;
        }
        let locale = self.locale;
        egui::Window::new(locale.text(TextKey::KeyboardShortcuts))
            .id(egui::Id::unique("keyboard-shortcuts-help"))
            .open(&mut self.shortcuts_help_open)
            .default_width(620.0)
            .default_height(600.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::app::views::diagram::show_command_shortcuts(ui, locale);
                    ui.separator();
                    crate::app::views::diagram::show_controls_help(ui, locale);
                });
            });
    }

    pub(super) fn show_help_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::HelpMenu), |ui| {
            if ui
                .add(egui::Button::new(locale.text(TextKey::KeyboardShortcuts)).shortcut_text("F1"))
                .clicked()
            {
                self.shortcuts_help_open = true;
                ui.close();
            }
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
