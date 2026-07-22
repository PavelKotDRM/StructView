//! Точка входа приложения JSON Viewer.
//!
//! Инициализирует [`eframe`] окно и запускает [`json_viewer::app::JsonViewerApp`].

use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("JSON Viewer")
            .with_inner_size([1024.0, 720.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "JSON Viewer",
        options,
        Box::new(|cc| Ok(Box::new(json_viewer::app::JsonViewerApp::new(cc)))),
    )
}
