//! Window-free operations shared with the CLI, plus the GUI graph image backend.

use std::sync::Arc;
use struct_view_core::parser::JsonNode;
pub use struct_view_operations::app::headless::{
    GraphImageRenderer, GraphProgressReporter, GraphProgressSnapshot, add, copy, delete, graph,
    node_at_path, paste, rename, schema, set, table,
};
pub use struct_view_operations::clipboard::{ClipboardEntry, decode_structures, encode_structures};

pub fn graph_image(root: &JsonNode, png: bool, dark: bool) -> Result<Vec<u8>, String> {
    super::views::graph_image_for_headless(root, png, dark)
}

pub fn graph_image_with_progress(
    root: &JsonNode,
    png: bool,
    dark: bool,
    report_progress: impl Fn(GraphProgressSnapshot) + Send + Sync + 'static,
) -> Result<Vec<u8>, String> {
    super::views::graph_image_with_progress_for_headless(
        root,
        png,
        dark,
        Some(Arc::new(report_progress)),
    )
}

pub struct GuiGraphImageRenderer;

impl GraphImageRenderer for GuiGraphImageRenderer {
    fn render_graph_image(
        &self,
        root: &JsonNode,
        png: bool,
        dark: bool,
        report_progress: Option<GraphProgressReporter>,
    ) -> Result<Vec<u8>, String> {
        match report_progress {
            Some(report_progress) => graph_image_with_progress(root, png, dark, move |snapshot| {
                report_progress(snapshot);
            }),
            None => graph_image(root, png, dark),
        }
    }
}
