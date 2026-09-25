//! Native graphical interface and clipboard support for StructView.

#![deny(warnings)]

use std::path::PathBuf;

pub mod app;
pub mod clipboard;

/// Start the native GUI with zero or more files already opened.
pub fn run_native_gui(files: Vec<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("StructView")
            .with_inner_size([1024.0, 720.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "StructView",
        options,
        Box::new(move |cc| Ok(Box::new(app::StructViewApp::new_with_files(cc, files)))),
    )
    .map_err(|error| error.to_string())
}
