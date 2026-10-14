use crate::app::StructViewApp;
use crate::app::i18n::TextKey;
use crate::app::visualization::VisualizationMode;

impl StructViewApp {
    pub(in crate::app) fn show_graph_animation_window(&mut self, ctx: &egui::Context) {
        if !self.graph_animation_window_open || self.visualization != VisualizationMode::Graph {
            self.graph_animation_window_open = false;
            self.graph_animation_window_docking.reset_if_active();
            return;
        }

        let locale = self.locale;
        let mut docking = std::mem::take(&mut self.graph_animation_window_docking);
        docking.begin();
        let mut show_content = |ui: &mut egui::Ui| self.show_graph_animation_settings(ui);
        let action = if docking.detached {
            crate::app::docking::show_detached_viewport(
                ctx,
                &mut docking,
                egui::ViewportId::from_hash_of("graph_animation_settings"),
                locale.text(TextKey::GraphAnimationApi),
                egui::vec2(480.0, 560.0),
                locale.text(TextKey::DockWindow),
                |ui| show_content(ui),
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(locale.text(TextKey::GraphAnimationApi))
                .id(docking.area_id("graph_animation_settings"))
                .open(&mut open)
                .default_width(480.0)
                .resizable(true);
            if let Some(size) = docking.embedded_size() {
                window = window.default_size(size);
            }
            if let Some(position) = docking.take_embedded_position() {
                window = window.current_pos(position);
            }
            let mut detach_requested = false;
            let response = window.show(ctx, |ui| {
                detach_requested = crate::app::docking::show_docking_controls(
                    ui,
                    false,
                    locale.text(TextKey::DetachWindow),
                    locale.text(TextKey::WindowOptions),
                );
                show_content(ui);
            });
            if !open {
                docking.reset_if_active();
                crate::app::docking::DetachedViewportAction::Closed
            } else {
                if let Some(response) = response {
                    docking.remember_embedded_size(response.response.rect.size());
                    if detach_requested {
                        let viewport_origin =
                            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min));
                        docking.detach(
                            ctx.viewport_rect(),
                            response.response.rect,
                            viewport_origin,
                        );
                        ctx.request_repaint();
                    }
                }
                crate::app::docking::DetachedViewportAction::KeepOpen
            }
        };
        drop(show_content);
        self.graph_animation_window_docking = docking;
        if action == crate::app::docking::DetachedViewportAction::Closed {
            self.graph_animation_window_open = false;
        }
    }
}
