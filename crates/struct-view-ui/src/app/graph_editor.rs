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
        match result {
            Ok(root) => {
                self.request_document_replacement(DocumentReplacement::GraphCopy(Box::new(root)))
            }
            Err(error) => self.show_error(&error),
        }
    }

    pub(in crate::app) fn has_editable_graph_document(&self) -> bool {
        self.root
            .as_ref()
            .is_some_and(|root| GraphDocument::read(root).is_ok_and(|document| document.is_some()))
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
        self.open_graph_document_with_structure(root, true);
    }

    pub(in crate::app) fn open_graph_copy_document(
        &mut self,
        root: JsonNode,
    ) -> Result<(), String> {
        let initial_fingerprint =
            super::state::document_content_fingerprint(&root, DataFormat::Json).ok_or_else(
                || {
                    self.locale
                        .text(TextKey::GraphCopyFingerprintError)
                        .to_string()
                },
            )?;
        let source = self
            .take_previous_document()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
        let graph_calculation = std::mem::take(&mut self.graph_calculation);
        self.open_graph_document_with_structure(root, false);
        self.graph_calculation = graph_calculation;
        self.file_state.saved_content_fingerprint = Some(initial_fingerprint);
        self.generated_graph_copy = Some(super::state::GeneratedGraphCopy {
            source,
            initial_fingerprint,
        });
        Ok(())
    }

    pub(in crate::app) fn switch_graph_to_view_mode(&mut self) {
        let unchanged = self.generated_graph_copy.as_ref().is_some_and(|copy| {
            self.root.as_ref().is_some_and(|root| {
                super::state::document_content_fingerprint(root, DataFormat::Json)
                    == Some(copy.initial_fingerprint)
            })
        });
        if unchanged && let Some(copy) = self.generated_graph_copy.take() {
            let graph_calculation = std::mem::take(&mut self.graph_calculation);
            self.restore_previous_document(copy.source);
            self.graph_calculation = graph_calculation;
        } else {
            self.mode = AppMode::View;
        }
    }

    fn open_graph_document_with_structure(&mut self, root: JsonNode, reset_structure: bool) {
        self.clear_document_state();
        if reset_structure {
            self.structure_view = super::views::structure::StructureView::default();
        }
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
