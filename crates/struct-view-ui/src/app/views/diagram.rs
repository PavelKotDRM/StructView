//! Shared menu controls; data and relationship models stay independent.

use egui::Ui;
use egui::{Key, Pos2, Vec2};

use super::graph::{GraphExportFormat, GraphExportStyle};
use crate::app::i18n::{Locale, TextKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum DiagramAction {
    ZoomOut,
    ZoomIn,
    ActualSize,
    Fit,
    SelectAll,
    ClearSelection,
}

pub(in crate::app) const ZOOM_STEP: f32 = 1.2;

pub(in crate::app) fn show_controls_help(ui: &mut Ui, locale: Locale) {
    ui.strong(locale.text(TextKey::HelpCommonControls));
    ui.label(locale.text(TextKey::HelpCanvasFocus));
    help_rows(
        ui,
        locale,
        "common-controls-help",
        &[
            (&["Drag", "↔"], TextKey::HelpPan),
            (&["Wheel", "↑", "↓", "Pinch"], TextKey::HelpZoom),
            (&["+", "−"], TextKey::HelpZoom),
            (&["Home"], TextKey::GraphFit),
            (&["↑", "↓"], TextKey::HelpPreviousNext),
            (&["Ctrl/Cmd", "F"], TextKey::SearchWindow),
        ],
    );
    ui.separator();
    ui.strong(locale.text(TextKey::StructureView));
    help_rows(
        ui,
        locale,
        "structure-controls-help",
        &[
            (&["←"], TextKey::HelpParentCollapse),
            (&["→"], TextKey::HelpChildExpand),
            (&["Enter", "Space"], TextKey::StructureToggle),
        ],
    );
    ui.separator();
    ui.strong(locale.text(TextKey::GraphView));
    help_rows(
        ui,
        locale,
        "graph-controls-help",
        &[
            (&["←", "→"], TextKey::HelpNeighbors),
            (&["Ctrl/Cmd", "Hover"], TextKey::HelpPinRouteTooltip),
            (&["Enter", "Space"], TextKey::HelpSelectNode),
            (&["Ctrl/Cmd", "Click"], TextKey::HelpToggleSelection),
            (&["Shift", "Drag"], TextKey::HelpRectangle),
            (&["Ctrl/Cmd", "Shift", "Drag"], TextKey::HelpAddRectangle),
        ],
    );
}

fn help_rows(ui: &mut Ui, locale: Locale, id: &str, rows: &[(&[&str], TextKey)]) {
    egui::Grid::new(id)
        .spacing([12.0, 6.0])
        .max_col_width(230.0)
        .show(ui, |ui| {
            for (gestures, action) in rows {
                ui.horizontal(|ui| {
                    for gesture in *gestures {
                        let text = match *gesture {
                            "Drag" => locale.text(TextKey::HelpDrag),
                            "Wheel" => locale.text(TextKey::HelpWheel),
                            "Pinch" => locale.text(TextKey::HelpPinch),
                            "Click" => locale.text(TextKey::HelpClick),
                            "Hover" => locale.text(TextKey::HelpHover),
                            _ => gesture,
                        };
                        egui::Frame::new()
                            .fill(ui.visuals().widgets.inactive.bg_fill)
                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .corner_radius(4)
                            .inner_margin(4)
                            .show(ui, |ui| {
                                ui.monospace(text);
                            });
                    }
                });
                ui.add(egui::Label::new(locale.text(*action)).wrap());
                ui.end_row();
            }
        });
}

pub(in crate::app) fn keyboard_activate(ui: &Ui) -> bool {
    ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space))
}

pub(in crate::app) fn fit_to(zoom: &mut f32, pan: &mut Vec2, viewport: Vec2, content: Vec2) {
    *zoom = (viewport.x / content.x.max(1.0))
        .min(viewport.y / content.y.max(1.0))
        .clamp(0.02, 1.0);
    *pan = (viewport - content * *zoom) * 0.5;
}

pub(in crate::app) fn zoom_at(zoom: &mut f32, pan: &mut Vec2, factor: f32, anchor: Pos2) {
    let old = *zoom;
    *zoom = (*zoom * factor).clamp(0.02, 4.0);
    *pan = anchor.to_vec2() - (anchor.to_vec2() - *pan) * (*zoom / old);
}

pub(in crate::app) fn wheel_zoom(ui: &Ui) -> f32 {
    ui.input(|i| {
        let pinch = i.zoom_delta();
        if pinch != 1.0 {
            pinch
        } else {
            (i.smooth_scroll_delta.y * 0.002).exp()
        }
    })
}

pub(in crate::app) fn keyboard_zoom(ui: &Ui) -> f32 {
    ui.input(|i| {
        if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
            ZOOM_STEP
        } else if i.key_pressed(Key::Minus) {
            1.0 / ZOOM_STEP
        } else {
            1.0
        }
    })
}

pub(in crate::app) fn view_controls(ui: &mut Ui, locale: Locale, zoom: f32) -> Vec<DiagramAction> {
    let mut actions = Vec::new();
    for (label, action) in [
        (locale.text(TextKey::GraphZoomOut), DiagramAction::ZoomOut),
        (locale.text(TextKey::GraphZoomIn), DiagramAction::ZoomIn),
        ("100%", DiagramAction::ActualSize),
        (locale.text(TextKey::GraphFit), DiagramAction::Fit),
    ] {
        if ui.button(label).clicked() {
            actions.push(action);
        }
    }
    ui.label(format!("{:.0}%", zoom * 100.0));
    ui.separator();
    for (label, action) in [
        (
            locale.text(TextKey::GraphSelectAll),
            DiagramAction::SelectAll,
        ),
        (
            locale.text(TextKey::GraphClearSelection),
            DiagramAction::ClearSelection,
        ),
    ] {
        if ui.button(label).clicked() {
            actions.push(action);
        }
    }
    actions
}

pub(in crate::app) fn export_controls(ui: &mut Ui, locale: Locale) -> Option<(bool, bool)> {
    let mut requested = None;
    ui.menu_button(locale.text(TextKey::DiagramExport), |ui| {
        for format in [GraphExportFormat::Svg, GraphExportFormat::Png] {
            ui.menu_button(format.label(), |ui| {
                for style in [
                    GraphExportStyle::LightTransparent,
                    GraphExportStyle::DarkOpaque,
                ] {
                    if ui.button(locale.text(style.text_key())).clicked() {
                        requested = Some((
                            format == GraphExportFormat::Png,
                            style == GraphExportStyle::DarkOpaque,
                        ));
                        ui.close();
                    }
                }
            });
        }
    });
    requested
}
