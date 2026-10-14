use super::*;
use crate::app::docking::{
    DetachedViewportAction, DockingState, show_detached_viewport, show_docking_controls,
};
use serde_json::{Value, json};
use struct_view_core::graph::EdgeDirection;

#[derive(Clone, Copy, Debug)]
pub(in crate::app) enum CanvasTarget {
    Node(usize),
    Edge(usize),
}

#[derive(Default)]
pub(in crate::app) struct GraphEditorState {
    draft: Option<Draft>,
    tab: usize,
    pub(in crate::app) open: bool,
    docking: DockingState,
}

struct Draft {
    target: Target,
    record: Value,
    id: String,
    label: String,
    source: String,
    target_id: String,
    direction: String,
    weight: String,
    partition: String,
    attributes: String,
    error: Option<String>,
    delete_confirmation: bool,
}

#[derive(Clone, Copy)]
enum Target {
    Node(Option<usize>),
    Edge(Option<usize>),
    Partition(Option<usize>),
    Settings,
}

impl GraphEditorState {
    fn show_commands(
        &mut self,
        ui: &mut egui::Ui,
        document: &GraphDocument,
        locale: Locale,
        close_menu: bool,
    ) {
        ui.add_enabled_ui(!self.is_busy(), |ui| {
            for (key, target, enabled) in [
                (TextKey::GraphAddNode, Target::Node(None), true),
                (
                    TextKey::GraphAddEdge,
                    Target::Edge(None),
                    !document.nodes.is_empty(),
                ),
                (TextKey::GraphAddGroup, Target::Partition(None), true),
                (TextKey::GraphSettings, Target::Settings, true),
            ] {
                if ui
                    .add_enabled(enabled, egui::Button::new(locale.text(key)))
                    .clicked()
                {
                    self.open(document, target);
                    if close_menu {
                        ui.close();
                    }
                }
            }
        });
    }

    pub(in crate::app) fn is_busy(&self) -> bool {
        self.draft.is_some()
    }

    fn open(&mut self, document: &GraphDocument, target: Target) {
        let record = match target {
            Target::Node(Some(index)) => document.nodes[index].clone(),
            Target::Edge(Some(index)) => document.edges[index].clone(),
            Target::Partition(index) => json!({"label": index.map(|i| &document.partitions[i])}),
            Target::Settings => Value::Object(document.metadata.clone()),
            Target::Node(None) => json!({"id": document.next_id(false), "attributes": {}}),
            Target::Edge(None) => json!({
                "id": document.next_id(true),
                "source": document.nodes.first().map(|node| &node["id"]),
                "target": document.nodes.get(1).or(document.nodes.first()).map(|node| &node["id"]),
                "direction": document.metadata["direction"], "attributes": {}
            }),
        };
        let string = |key: &str| record[key].as_str().unwrap_or_default().to_string();
        self.draft = Some(Draft {
            id: string("id"),
            label: string(if matches!(target, Target::Settings) {
                "name"
            } else {
                "label"
            }),
            source: string("source"),
            target_id: string("target"),
            direction: string("direction"),
            weight: if matches!(target, Target::Settings) {
                string("weight_unit")
            } else {
                record
                    .get("weight")
                    .map(Value::to_string)
                    .unwrap_or_default()
            },
            partition: string("partition"),
            attributes: record.get("attributes").unwrap_or(&json!({})).to_string(),
            target,
            record,
            error: None,
            delete_confirmation: false,
        });
    }
}

mod dialog;
#[cfg(test)]
mod tests;
mod window;
