use super::i18n::{Locale, TextKey};
use super::state::{AppMode, DocumentReplacement, FileState, StructViewApp};
use super::visualization::{VisualizationMode, try_build_relationship_graph};
use struct_view_core::parser::{DataFormat, JsonNode};
#[cfg(test)]
use struct_view_core::parser::{node_to_value, parse_data};

#[cfg(test)]
mod tests;
mod ui;

pub(super) use struct_view_operations::app::graph_editor::GraphDocument;
pub(super) use ui::CanvasTarget;
pub(super) use ui::GraphEditorState;

impl StructViewApp {
    pub(in crate::app) fn request_new_graph(&mut self) {
        self.request_graph_document(GraphDocument::empty().to_root());
    }

    pub(in crate::app) fn request_graph_copy(&mut self) {
        let result = self
            .root
            .as_ref()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())
            .and_then(|root| {
                let graph = try_build_relationship_graph(root)?;
                GraphDocument::from_graph(root, &graph)?.to_root()
            });
        self.request_graph_document(result);
    }

    fn request_graph_document(&mut self, result: Result<JsonNode, String>) {
        match result {
            Ok(root) => {
                self.request_document_replacement(DocumentReplacement::Graph(Box::new(root)))
            }
            Err(error) => self.show_error(&error),
        }
    }

    pub(in crate::app) fn open_graph_document(&mut self, root: JsonNode) {
        self.clear_document_state();
        self.structure_view = super::views::structure::StructureView::default();
        self.root = Some(root);
        self.file_state = FileState {
            format: Some(DataFormat::Json),
            ..Default::default()
        };
        self.mode = AppMode::Edit;
        self.visualization = VisualizationMode::Graph;
        self.graph_editor.open = true;
    }

    pub(in crate::app) fn apply_graph_document(
        &mut self,
        document: GraphDocument,
    ) -> Result<(), String> {
        if self.mode != AppMode::Edit || self.comparison.is_some() {
            return Err(self.locale.text(TextKey::GraphEditOnly).to_string());
        }
        let replacement = document.to_root()?;
        self.apply_document_change(|root, _| {
            *root = replacement;
            Ok(())
        })
    }
}
