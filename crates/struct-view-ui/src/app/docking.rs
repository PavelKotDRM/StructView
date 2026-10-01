#[derive(Debug, Default)]
pub(super) struct DockingState {
    pub(super) detached: bool,
    embedded_position: Option<egui::Pos2>,
    embedded_size: Option<egui::Vec2>,
    viewport_position: Option<egui::Pos2>,
    generation: u64,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DetachedViewportAction {
    KeepOpen,
    Docked,
    Closed,
}

pub(super) fn show_docking_controls(
    ui: &mut egui::Ui,
    detached: bool,
    action_label: &str,
    options_label: &str,
) -> bool {
    let mut action_requested = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if detached {
                action_requested = ui
                    .button(action_label)
                    .on_hover_text(action_label)
                    .clicked();
            } else {
                let button_size = ui.spacing().interact_size.y;
                let response = ui
                    .add_sized([button_size, button_size], egui::Button::new(""))
                    .on_hover_text(options_label);
                let icon_color = ui.visuals().text_color();
                for offset in [-4.0, 0.0, 4.0] {
                    ui.painter().circle_filled(
                        response.rect.center() + egui::vec2(0.0, offset),
                        1.5,
                        icon_color,
                    );
                }
                let _menu = egui::Popup::menu(&response).show(|ui| {
                    if ui.button(action_label).clicked() {
                        action_requested = true;
                        ui.close();
                    }
                });
            }
        });
    });
    ui.separator();
    action_requested
}

impl DockingState {
    pub(super) fn begin(&mut self) {
        self.active = true;
    }

    pub(super) fn area_id(&self, id_salt: &'static str) -> egui::Id {
        egui::Id::new((id_salt, self.generation))
    }

    pub(super) fn take_embedded_position(&mut self) -> Option<egui::Pos2> {
        self.embedded_position.take()
    }

    pub(super) fn embedded_size(&self) -> Option<egui::Vec2> {
        self.embedded_size
    }

    pub(super) fn remember_embedded_size(&mut self, size: egui::Vec2) {
        self.embedded_size = Some(size);
    }

    pub(super) fn detach(
        &mut self,
        viewport_rect: egui::Rect,
        window_rect: egui::Rect,
        viewport_origin: Option<egui::Pos2>,
    ) {
        self.detached = true;
        self.viewport_position =
            viewport_origin.map(|origin| origin + (window_rect.min - viewport_rect.min));
        self.embedded_position = None;
        self.embedded_size = Some(window_rect.size());
    }

    pub(super) fn take_viewport_position(&mut self) -> Option<egui::Pos2> {
        self.viewport_position.take()
    }

    pub(super) fn dock(&mut self, position: egui::Pos2) {
        self.detached = false;
        self.generation = self.generation.wrapping_add(1);
        self.embedded_position = Some(position);
        self.viewport_position = None;
    }

    pub(super) fn reset_if_active(&mut self) {
        if self.active {
            self.generation = self.generation.wrapping_add(1);
        }
        self.detached = false;
        self.embedded_position = None;
        self.embedded_size = None;
        self.viewport_position = None;
        self.active = false;
    }
}

pub(super) fn show_detached_viewport(
    ctx: &egui::Context,
    docking: &mut DockingState,
    viewport_id: egui::ViewportId,
    title: impl Into<String>,
    viewport_size: egui::Vec2,
    dock_label: &str,
    mut add_contents: impl FnMut(&mut egui::Ui),
) -> DetachedViewportAction {
    let main_local_rect = ctx.viewport_rect();
    let mut builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(viewport_size);
    if let Some(position) = docking.take_viewport_position() {
        builder = builder.with_position(position);
    }

    let mut window_closed = false;
    let mut dock_position = None;
    ctx.show_viewport_immediate(viewport_id, builder, |viewport_ui, class| {
        let viewport_ctx = viewport_ui.ctx().clone();
        let (close_requested, window_rect, main_rect) = viewport_ctx.input(|input| {
            let viewport = input.viewport();
            let main = input
                .raw
                .viewports
                .get(&egui::ViewportId::ROOT)
                .and_then(|viewport| viewport.inner_rect);
            (viewport.close_requested(), viewport.outer_rect, main)
        });
        if class != egui::ViewportClass::EmbeddedWindow && close_requested {
            viewport_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            window_closed = true;
            return;
        }

        let return_requested = if class == egui::ViewportClass::EmbeddedWindow {
            let return_requested = show_docking_controls(viewport_ui, true, dock_label, dock_label);
            add_contents(viewport_ui);
            return_requested
        } else {
            egui::CentralPanel::default()
                .show(viewport_ui, |ui| {
                    let return_requested = show_docking_controls(ui, true, dock_label, dock_label);
                    add_contents(ui);
                    return_requested
                })
                .inner
        };

        if return_requested {
            let position = docked_position(
                main_local_rect,
                main_rect,
                window_rect,
                docking.embedded_size.unwrap_or(viewport_size),
            );
            if class != egui::ViewportClass::EmbeddedWindow {
                viewport_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            dock_position = Some(position);
        }
    });

    if window_closed {
        docking.reset_if_active();
        ctx.request_repaint();
        DetachedViewportAction::Closed
    } else if let Some(position) = dock_position {
        docking.dock(position);
        ctx.request_repaint();
        DetachedViewportAction::Docked
    } else {
        DetachedViewportAction::KeepOpen
    }
}

fn docked_position(
    main_local_rect: egui::Rect,
    main_rect: Option<egui::Rect>,
    window_rect: Option<egui::Rect>,
    window_size: egui::Vec2,
) -> egui::Pos2 {
    let position = main_rect
        .zip(window_rect)
        .map(|(main, window)| main_local_rect.min + (window.min - main.min))
        .unwrap_or(main_local_rect.min + egui::vec2(24.0, 24.0));
    let max_x = (main_local_rect.right() - window_size.x).max(main_local_rect.left());
    let max_y = (main_local_rect.bottom() - window_size.y).max(main_local_rect.top());
    egui::pos2(
        position.x.clamp(main_local_rect.left(), max_x),
        position.y.clamp(main_local_rect.top(), max_y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_window_stays_attached_until_explicitly_detached() {
        let main_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let inside = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(40.0, 40.0));
        let screen_origin = egui::pos2(500.0, 200.0);
        let mut docking = DockingState::default();

        docking.begin();
        docking.remember_embedded_size(inside.size());
        assert!(!docking.detached);
        docking.detach(main_rect, inside, Some(screen_origin));

        assert!(docking.detached);
        assert_eq!(docking.viewport_position, Some(egui::pos2(510.0, 210.0)));
        assert_eq!(docking.embedded_size, Some(inside.size()));

        let dock_position = egui::pos2(20.0, 30.0);
        docking.dock(dock_position);
        assert!(!docking.detached);
        assert_eq!(docking.embedded_position, Some(dock_position));
    }

    #[test]
    fn docking_maps_native_position_to_main_window_coordinates() {
        let main_local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let main_rect =
            egui::Rect::from_min_size(egui::pos2(500.0, 200.0), egui::vec2(800.0, 600.0));
        let window_rect =
            egui::Rect::from_min_size(egui::pos2(650.0, 350.0), egui::vec2(300.0, 200.0));

        assert_eq!(
            docked_position(
                main_local_rect,
                Some(main_rect),
                Some(window_rect),
                egui::vec2(300.0, 200.0),
            ),
            egui::pos2(150.0, 150.0)
        );
    }
}
