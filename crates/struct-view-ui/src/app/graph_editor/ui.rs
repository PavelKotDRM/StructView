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

impl StructViewApp {
    pub(in crate::app) fn can_edit_graph(&self) -> bool {
        self.visualization == VisualizationMode::Graph
            && self.mode == AppMode::Edit
            && self.comparison.is_none()
            && self.root.as_ref().is_some_and(|root| {
                GraphDocument::read(root).is_ok_and(|document| document.is_some())
            })
    }

    pub(in crate::app) fn show_graph_edit_commands(&mut self, ui: &mut egui::Ui) {
        if !self.can_edit_graph() {
            return;
        }
        let document = match self.root.as_ref().map(GraphDocument::read).transpose() {
            Ok(Some(Some(document))) => document,
            Ok(_) => return,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };
        self.graph_editor
            .show_commands(ui, &document, self.locale, true);
    }

    pub(in crate::app) fn show_graph_editor(&mut self, ctx: &egui::Context) {
        if !self.can_edit_graph() || !self.graph_editor.open {
            self.graph_editor.docking.reset_if_active();
            return;
        }
        let document = match self.root.as_ref().map(GraphDocument::read).transpose() {
            Ok(Some(Some(document))) => document,
            Ok(_) => return,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };
        let locale = self.locale;
        let mut docking = std::mem::take(&mut self.graph_editor.docking);
        docking.begin();
        let mut show_content = |ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                self.graph_editor
                    .show_commands(ui, &document, locale, false);
            });
            ui.separator();
            ui.add_enabled_ui(!self.graph_editor.is_busy(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (index, key) in [
                        TextKey::GraphNodes,
                        TextKey::GraphEdges,
                        TextKey::GraphGroups,
                        TextKey::GraphSettings,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.selectable_value(&mut self.graph_editor.tab, index, locale.text(key));
                    }
                });
                ui.separator();
                match self.graph_editor.tab {
                    0 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-nodes")
                            .show(ui, |ui| {
                                for (index, record) in document.nodes.iter().enumerate() {
                                    if ui
                                        .button(format!(
                                            "{}  {}",
                                            text(record, "id"),
                                            text(record, "label")
                                        ))
                                        .clicked()
                                    {
                                        self.graph_editor
                                            .open(&document, Target::Node(Some(index)));
                                    }
                                }
                            });
                    }
                    1 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-edges")
                            .show(ui, |ui| {
                                for (index, record) in document.edges.iter().enumerate() {
                                    if ui
                                        .button(format!(
                                            "{}: {} -> {}  {}",
                                            text(record, "id"),
                                            text(record, "source"),
                                            text(record, "target"),
                                            text(record, "label")
                                        ))
                                        .clicked()
                                    {
                                        self.graph_editor
                                            .open(&document, Target::Edge(Some(index)));
                                    }
                                }
                            });
                    }
                    2 => {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("graph-editor-groups")
                            .show(ui, |ui| {
                                for (index, name) in document.partitions.iter().enumerate() {
                                    if ui.button(name).clicked() {
                                        self.graph_editor
                                            .open(&document, Target::Partition(Some(index)));
                                    }
                                }
                            });
                    }
                    _ => {
                        ui.label(text(&Value::Object(document.metadata.clone()), "name"));
                        ui.label(locale.text(TextKey::GraphSettingsHint));
                    }
                }
                ui.separator();
                ui.weak(locale.text(TextKey::GraphEditorHint));
                ui.weak(locale.text(TextKey::GraphAutomaticLayout));
            });
        };
        let action = if docking.detached {
            show_detached_viewport(
                ctx,
                &mut docking,
                egui::ViewportId::from_hash_of("graph-editor-window"),
                locale.text(TextKey::GraphEditor),
                egui::vec2(420.0, 440.0),
                locale.text(TextKey::DockWindow),
                &mut show_content,
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(locale.text(TextKey::GraphEditor))
                .id(docking.area_id("graph-editor-window"))
                .open(&mut open)
                .default_size(egui::vec2(420.0, 440.0))
                .collapsible(true)
                .resizable(true);
            if let Some(size) = docking.embedded_size() {
                window = window.default_size(size);
            }
            if let Some(position) = docking.take_embedded_position() {
                window = window.current_pos(position);
            }
            let mut detach = false;
            let response = window.show(ctx, |ui| {
                detach = show_docking_controls(
                    ui,
                    false,
                    locale.text(TextKey::DetachWindow),
                    locale.text(TextKey::WindowOptions),
                );
                show_content(ui);
            });
            if !open {
                docking.reset_if_active();
                DetachedViewportAction::Closed
            } else {
                if let Some(response) = response {
                    docking.remember_embedded_size(response.response.rect.size());
                    if detach {
                        let origin =
                            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min));
                        docking.detach(ctx.viewport_rect(), response.response.rect, origin);
                        ctx.request_repaint();
                    }
                }
                DetachedViewportAction::KeepOpen
            }
        };
        drop(show_content);
        self.graph_editor.docking = docking;
        if action == DetachedViewportAction::Closed {
            self.graph_editor.open = false;
        }
    }

    pub(in crate::app) fn handle_graph_canvas_edit(&mut self, ctx: &egui::Context) {
        let request = ctx.data_mut(|data| {
            let id = egui::Id::new("graph-editor-canvas-request");
            let request = data.get_temp::<CanvasTarget>(id);
            data.remove::<CanvasTarget>(id);
            request
        });
        if self.mode != AppMode::Edit || self.graph_editor.is_busy() {
            return;
        }
        if let Some(request) = request {
            match self.root.as_ref().map(GraphDocument::read).transpose() {
                Ok(Some(Some(document))) => {
                    let target = match request {
                        CanvasTarget::Node(index) if index < document.nodes.len() => {
                            Target::Node(Some(index))
                        }
                        CanvasTarget::Edge(index) if index < document.edges.len() => {
                            Target::Edge(Some(index))
                        }
                        _ => return,
                    };
                    self.graph_editor.open(&document, target);
                }
                Err(error) => self.show_error(&error),
                _ => {}
            }
        }
    }

    pub(in crate::app) fn show_graph_editor_dialog(&mut self, ctx: &egui::Context) {
        if self.visualization != VisualizationMode::Graph
            || self.mode != AppMode::Edit
            || self.comparison.is_some()
        {
            return;
        }
        let Some(mut draft) = self.graph_editor.draft.take() else {
            return;
        };
        let document = match self.root.as_ref().map(GraphDocument::read).transpose() {
            Ok(Some(Some(document))) => document,
            result => {
                self.show_error(&match result {
                    Err(error) => error,
                    _ => self.locale.text(TextKey::NoDocument).to_string(),
                });
                return;
            }
        };
        let locale = self.locale;
        let mut close = false;
        let mut apply = false;
        let mut delete = false;
        let mut duplicate = false;
        let mut spawn = false;
        egui::Modal::new(egui::Id::new("graph-editor-dialog")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(locale.text(match draft.target {
                Target::Node(_) => TextKey::GraphNodes,
                Target::Edge(_) => TextKey::GraphEdges,
                Target::Partition(_) => TextKey::GraphGroups,
                Target::Settings => TextKey::GraphSettings,
            }));
            egui::ScrollArea::vertical()
                .max_height(480.0)
                .show(ui, |ui| {
                    match draft.target {
                        Target::Node(_) => {
                            field(ui, locale.text(TextKey::GraphId), &mut draft.id);
                            field(ui, locale.text(TextKey::GraphLabel), &mut draft.label);
                            let choices = std::iter::once(String::new())
                                .chain(document.partitions.iter().cloned())
                                .collect::<Vec<_>>();
                            choice(
                                ui,
                                "node-partition",
                                locale.text(TextKey::GraphGroups),
                                &mut draft.partition,
                                &choices,
                                locale,
                            );
                            if matches!(draft.target, Target::Node(None))
                                && !draft.source.is_empty()
                            {
                                ui.label(format!(
                                    "{}: {}",
                                    locale.text(TextKey::GraphSource),
                                    draft.source
                                ));
                                direction_choice(ui, &mut draft.direction, locale);
                            }
                        }
                        Target::Edge(_) => {
                            field(ui, locale.text(TextKey::GraphId), &mut draft.id);
                            field(ui, locale.text(TextKey::GraphLabel), &mut draft.label);
                            let ids = document
                                .nodes
                                .iter()
                                .map(|node| text(node, "id").to_string())
                                .collect::<Vec<_>>();
                            choice(
                                ui,
                                "edge-source",
                                locale.text(TextKey::GraphSource),
                                &mut draft.source,
                                &ids,
                                locale,
                            );
                            choice(
                                ui,
                                "edge-target",
                                locale.text(TextKey::GraphTarget),
                                &mut draft.target_id,
                                &ids,
                                locale,
                            );
                            direction_choice(ui, &mut draft.direction, locale);
                            field(ui, locale.text(TextKey::GraphWeight), &mut draft.weight);
                            if ui
                                .button(locale.text(TextKey::GraphReverseEndpoints))
                                .clicked()
                            {
                                std::mem::swap(&mut draft.source, &mut draft.target_id);
                            }
                        }
                        Target::Partition(_) => {
                            field(ui, locale.text(TextKey::GraphLabel), &mut draft.label);
                        }
                        Target::Settings => {
                            field(ui, locale.text(TextKey::GraphLabel), &mut draft.label);
                            direction_choice(ui, &mut draft.direction, locale);
                            field(ui, locale.text(TextKey::GraphWeightUnit), &mut draft.weight);
                        }
                    }
                    if !matches!(draft.target, Target::Partition(_)) {
                        ui.label(locale.text(TextKey::GraphAttributes));
                        ui.weak(locale.text(TextKey::GraphAttributesHint));
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.attributes)
                                .code_editor()
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                        ui.collapsing(locale.text(TextKey::GraphElementJson), |ui| {
                            match draft.build_record().and_then(|record| {
                                serde_json::to_string_pretty(&record)
                                    .map_err(|error| error.to_string())
                            }) {
                                Ok(preview) => {
                                    let mut preview = preview.as_str();
                                    ui.add(
                                        egui::TextEdit::multiline(&mut preview)
                                            .code_editor()
                                            .desired_width(f32::INFINITY),
                                    );
                                }
                                Err(error) => {
                                    ui.colored_label(ui.visuals().error_fg_color, error);
                                }
                            }
                        });
                    }
                });
            if let Some(error) = &draft.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if draft.delete_confirmation {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    locale.text(TextKey::GraphDeleteWarning),
                );
                ui.horizontal(|ui| {
                    delete = ui.button(locale.text(TextKey::GraphDelete)).clicked();
                    if ui.button(locale.text(TextKey::Cancel)).clicked() {
                        draft.delete_confirmation = false;
                    }
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    apply = ui.button(locale.text(TextKey::Apply)).clicked();
                    close = ui.button(locale.text(TextKey::Cancel)).clicked();
                    if matches!(draft.target, Target::Node(Some(_)) | Target::Edge(Some(_))) {
                        duplicate = ui.button(locale.text(TextKey::GraphDuplicate)).clicked();
                    }
                    if matches!(draft.target, Target::Node(Some(_))) {
                        spawn = ui.button(locale.text(TextKey::GraphSpawnNode)).clicked();
                    }
                    if matches!(
                        draft.target,
                        Target::Node(Some(_)) | Target::Edge(Some(_)) | Target::Partition(Some(_))
                    ) && ui.button(locale.text(TextKey::GraphDelete)).clicked()
                    {
                        draft.delete_confirmation = true;
                    }
                });
            }
        });
        if duplicate {
            draft.target = match draft.target {
                Target::Node(_) => Target::Node(None),
                _ => Target::Edge(None),
            };
            draft.id = document.next_id(matches!(draft.target, Target::Edge(_)));
            draft.error = None;
        }
        if apply || delete || spawn {
            let mut changed = document;
            let result = if delete {
                draft.delete(&mut changed)
            } else {
                draft.apply(&mut changed)
            };
            let next = changed.clone();
            let result = result.and_then(|()| self.apply_graph_document(changed));
            match result {
                Ok(()) => {
                    close = true;
                    if spawn {
                        self.graph_editor.open(&next, Target::Node(None));
                        if let Some(child) = self.graph_editor.draft.as_mut() {
                            child.source = draft.id.clone();
                            child.direction =
                                text(&Value::Object(next.metadata), "direction").to_string();
                        }
                    }
                }
                Err(error) => draft.error = Some(error),
            }
        }
        if !close {
            self.graph_editor.draft = Some(draft);
        }
    }
}

impl Draft {
    fn build_record(&self) -> Result<Value, String> {
        let attributes: Value = serde_json::from_str(&self.attributes)
            .map_err(|error| format!("Invalid attributes JSON: {error}"))?;
        if !attributes.is_object() {
            return Err("Attributes must be a JSON object".into());
        }
        let mut record = self.record.clone();
        record["attributes"] = attributes;
        match self.target {
            Target::Node(_) => {
                record["id"] = json!(self.id);
                record["label"] = json!(self.label);
                if self.partition.is_empty() {
                    record
                        .as_object_mut()
                        .ok_or("Invalid node record")?
                        .remove("partition");
                } else {
                    record["partition"] = json!(self.partition);
                }
            }
            Target::Edge(_) => {
                record["id"] = json!(self.id);
                record["label"] = json!(self.label);
                record["source"] = json!(self.source);
                record["target"] = json!(self.target_id);
                record["direction"] = json!(self.direction);
                if self.weight.trim().is_empty() {
                    record
                        .as_object_mut()
                        .ok_or("Invalid edge record")?
                        .remove("weight");
                } else {
                    record["weight"] = serde_json::from_str(&self.weight)
                        .map_err(|error| format!("Invalid weight: {error}"))?;
                    if !record["weight"].is_number() {
                        return Err("Edge weight must be a JSON number".into());
                    }
                }
            }
            Target::Settings => {
                record["name"] = json!(self.label);
                record["direction"] = json!(self.direction);
                record["weight_unit"] = json!(self.weight);
            }
            Target::Partition(_) => return Err("Groups have no element record".into()),
        }
        Ok(record)
    }

    fn apply(&self, document: &mut GraphDocument) -> Result<(), String> {
        if let Target::Partition(index) = self.target {
            if let Some(index) = index {
                let old = &document.partitions[index];
                for node in &mut document.nodes {
                    if node["partition"].as_str() == Some(old) {
                        node["partition"] = json!(self.label);
                    }
                }
                document.partitions[index] = self.label.clone();
            } else {
                document.partitions.push(self.label.clone());
            }
            return document.validate();
        }
        let record = self.build_record()?;
        match self.target {
            Target::Node(index) => {
                document.put_node(index, record)?;
                if index.is_none() && !self.source.is_empty() {
                    document.put_edge(
                        None,
                        json!({
                            "id": document.next_id(true),
                            "source": self.source, "target": self.id,
                            "direction": self.direction, "attributes": {}
                        }),
                    )?;
                }
                Ok(())
            }
            Target::Edge(index) => document.put_edge(index, record),
            Target::Settings => {
                document.metadata = record.as_object().ok_or("Invalid graph metadata")?.clone();
                document.validate()
            }
            Target::Partition(_) => unreachable!(),
        }
    }

    fn delete(&self, document: &mut GraphDocument) -> Result<(), String> {
        match self.target {
            Target::Node(Some(index)) => document.delete_node(index)?,
            Target::Edge(Some(index)) => {
                document.edges.remove(index);
            }
            Target::Partition(Some(index)) => {
                let name = document.partitions.remove(index);
                for node in &mut document.nodes {
                    if node["partition"].as_str() == Some(&name) {
                        node.as_object_mut()
                            .ok_or("Invalid node record")?
                            .remove("partition");
                    }
                }
            }
            _ => return Err("No graph element selected for deletion".into()),
        }
        document.validate()
    }
}

fn text<'a>(record: &'a Value, key: &str) -> &'a str {
    record[key].as_str().unwrap_or_default()
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
}

fn choice(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    choices: &[String],
    locale: Locale,
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(if value.is_empty() {
            locale.text(TextKey::GraphNoGroup)
        } else {
            value.as_str()
        })
        .show_ui(ui, |ui| {
            for item in choices {
                ui.selectable_value(
                    value,
                    item.clone(),
                    if item.is_empty() {
                        locale.text(TextKey::GraphNoGroup)
                    } else {
                        item
                    },
                );
            }
        });
}

fn direction_choice(ui: &mut egui::Ui, value: &mut String, locale: Locale) {
    ui.label(locale.text(TextKey::GraphDirection));
    egui::ComboBox::from_id_salt("graph-direction")
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for (direction, key) in [
                (EdgeDirection::Directed, TextKey::GraphDirected),
                (EdgeDirection::Undirected, TextKey::GraphUndirected),
                (EdgeDirection::Bidirectional, TextKey::GraphBidirectional),
                (EdgeDirection::Reverse, TextKey::GraphReverse),
            ] {
                ui.selectable_value(value, direction.as_str().to_string(), locale.text(key));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GraphDocument {
        let mut document = GraphDocument::empty();
        document.nodes = vec![
            json!({"id":"a","label":"A","partition":"first","attributes":{}}),
            json!({"id":"b","label":"B","attributes":{}}),
        ];
        document.partitions = vec!["first".into()];
        document
    }

    #[test]
    fn graph_editor_forms_create_connected_entities_and_preserve_attributes() {
        let mut document = sample();
        let mut editor = GraphEditorState::default();
        editor.open(&document, Target::Node(None));
        let draft = editor.draft.as_mut().unwrap();
        draft.source = "a".into();
        draft.direction = "bidirectional".into();
        draft.attributes = r#"{"active":true,"tags":["one"],"nested":{"x":2}}"#.into();
        draft.apply(&mut document).unwrap();
        assert_eq!(document.nodes.len(), 3);
        assert_eq!(document.edges.len(), 1);
        assert_eq!(document.edges[0]["source"], "a");
        assert_eq!(document.edges[0]["target"], draft.id);
        assert_eq!(document.nodes[2]["attributes"]["active"], true);
        assert_eq!(document.nodes[2]["attributes"]["nested"]["x"], 2);
    }

    #[test]
    fn graph_editor_live_json_matches_applied_records_without_duplicating_attributes() {
        for target in [Target::Node(None), Target::Edge(None), Target::Settings] {
            let mut document = sample();
            let mut editor = GraphEditorState::default();
            editor.open(&document, target);
            let draft = editor.draft.as_mut().unwrap();
            draft.id = "new-id".into();
            draft.label = "New label".into();
            draft.attributes = r#"{"count":2,"active":true,"nested":{"values":[1,null]}}"#.into();
            if matches!(target, Target::Edge(_)) {
                draft.source = "a".into();
                draft.target_id = "b".into();
                draft.weight = "2.5".into();
            }
            let preview = draft.build_record().unwrap();
            assert_eq!(preview["attributes"]["count"], 2);
            assert_eq!(preview["attributes"]["active"], true);
            assert!(preview["attributes"].get("id").is_none());
            draft.apply(&mut document).unwrap();
            let applied = match target {
                Target::Node(_) => document.nodes.last().unwrap().clone(),
                Target::Edge(_) => document.edges.last().unwrap().clone(),
                Target::Settings => Value::Object(document.metadata),
                _ => unreachable!(),
            };
            assert_eq!(preview, applied);
            if !matches!(target, Target::Settings) {
                assert_eq!(preview["id"], "new-id");
                assert_eq!(preview["label"], "New label");
            }
            draft.attributes = "[]".into();
            assert!(draft.build_record().is_err());
            draft.attributes = "{".into();
            assert!(draft.build_record().is_err());
        }
    }

    #[test]
    fn graph_editor_group_rename_and_delete_update_memberships() {
        let mut document = sample();
        let mut editor = GraphEditorState::default();
        editor.open(&document, Target::Partition(Some(0)));
        let draft = editor.draft.as_mut().unwrap();
        draft.label = "renamed".into();
        draft.apply(&mut document).unwrap();
        assert_eq!(document.nodes[0]["partition"], "renamed");
        draft.delete(&mut document).unwrap();
        assert!(document.partitions.is_empty());
        assert!(document.nodes[0].get("partition").is_none());
    }

    #[test]
    fn graph_editor_settings_and_edge_forms_use_numeric_weights() {
        let mut document = sample();
        let mut editor = GraphEditorState::default();
        editor.open(&document, Target::Settings);
        let draft = editor.draft.as_mut().unwrap();
        draft.direction = "undirected".into();
        draft.weight = "ms".into();
        draft.apply(&mut document).unwrap();
        editor.open(&document, Target::Edge(None));
        let draft = editor.draft.as_mut().unwrap();
        assert_eq!(draft.direction, "undirected");
        draft.weight = "2.5".into();
        draft.apply(&mut document).unwrap();
        assert_eq!(document.edges[0]["weight"], 2.5);
        draft.weight = "\"not a number\"".into();
        assert!(draft.apply(&mut document).is_err());
        draft.weight.clear();
        draft.target = Target::Edge(Some(0));
        draft.apply(&mut document).unwrap();
        assert!(document.edges[0].get("weight").is_none());
    }

    fn render(
        ctx: &egui::Context,
        app: &mut StructViewApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                app.show_top_panel(ui);
                app.show_central_panel(ui);
                app.show_graph_editor(ui.ctx());
                app.show_graph_editor_dialog(ui.ctx());
            },
        )
    }

    fn open_menu(ctx: &egui::Context, app: &mut StructViewApp, key: TextKey) -> egui::FullOutput {
        let output = render(ctx, app, vec![]);
        let events = click(&output, app.locale.text(key));
        output.drop_without_applying_deltas();
        render(ctx, app, events).drop_without_applying_deltas();
        render(ctx, app, vec![])
    }

    fn click(output: &egui::FullOutput, label: &str) -> Vec<egui::Event> {
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap()
            .1
            .bounds()
            .unwrap();
        let point = egui::pos2(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn graph_editor_native_ui_add_apply_cancel_and_history_work() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = StructViewApp::default();
        app.locale = Locale::English;
        app.open_graph_document(GraphDocument::empty().to_root().unwrap());
        let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
        let events = click(&output, "Add entity");
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        assert!(app.graph_editor.is_busy());
        assert!(!app.can_undo());
        let output = render(&ctx, &mut app, vec![]);
        let events = click(&output, "Apply");
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        assert!(!app.graph_editor.is_busy());
        assert_eq!(
            GraphDocument::read(app.root.as_ref().unwrap())
                .unwrap()
                .unwrap()
                .nodes
                .len(),
            1
        );
        assert!(app.can_undo());
        app.undo();
        assert!(
            GraphDocument::read(app.root.as_ref().unwrap())
                .unwrap()
                .unwrap()
                .nodes
                .is_empty()
        );
        app.redo();
        let original = node_to_value(app.root.as_ref().unwrap()).unwrap();
        let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
        let events = click(&output, "Add entity");
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        let output = render(&ctx, &mut app, vec![]);
        let events = click(&output, "Cancel");
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        assert!(!app.graph_editor.is_busy());
        assert_eq!(node_to_value(app.root.as_ref().unwrap()).unwrap(), original);
    }

    #[test]
    fn graph_editor_read_only_view_has_no_mutation_controls() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = StructViewApp::default();
        app.locale = Locale::English;
        app.open_graph_document(sample().to_root().unwrap());
        app.mode = AppMode::View;
        let output = render(&ctx, &mut app, vec![]);
        let nodes = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes;
        assert!(
            !nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Add entity"))
        );
        assert!(
            nodes
                .iter()
                .all(|(_, node)| node.label() != Some(app.locale.text(TextKey::GraphEditor)))
        );
        output.drop_without_applying_deltas();
        let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
        for key in [
            TextKey::GraphAddNode,
            TextKey::GraphAddEdge,
            TextKey::GraphAddGroup,
            TextKey::GraphSettings,
        ] {
            assert!(!has_label(&output, app.locale.text(key)));
        }
        output.drop_without_applying_deltas();
    }

    fn has_label(output: &egui::FullOutput, label: &str) -> bool {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some(label))
    }

    #[test]
    fn graph_editor_commands_are_in_menus_and_hidden_in_other_modes() {
        for mode in [
            VisualizationMode::Graph,
            VisualizationMode::Tree,
            VisualizationMode::Table,
            VisualizationMode::Schema,
        ] {
            for key in [TextKey::FileMenu, TextKey::EditMenu, TextKey::ViewMenu] {
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                let mut app = StructViewApp::default();
                app.locale = Locale::English;
                app.open_graph_document(sample().to_root().unwrap());
                app.visualization = mode;
                app.graph_editor.open = false;
                let output = open_menu(&ctx, &mut app, key);
                let graph = mode == VisualizationMode::Graph;
                match key {
                    TextKey::FileMenu => {
                        assert_eq!(has_label(&output, "New graph"), graph);
                        assert_eq!(has_label(&output, "Create editable graph copy"), graph);
                    }
                    TextKey::EditMenu => {
                        assert_eq!(has_label(&output, "Add entity"), graph);
                        assert_eq!(has_label(&output, "Add link"), graph);
                        assert_eq!(has_label(&output, "Add group"), graph);
                        assert_eq!(has_label(&output, "Graph properties"), graph);
                        assert_eq!(
                            has_label(&output, app.locale.text(TextKey::CopySelectedStructures)),
                            mode == VisualizationMode::Tree
                        );
                    }
                    TextKey::ViewMenu => {
                        assert_eq!(has_label(&output, "Graph editor"), graph);
                        assert_eq!(
                            has_label(
                                &output,
                                &format!("{} \u{23f5}", app.locale.text(TextKey::TreeActions))
                            ),
                            mode == VisualizationMode::Tree,
                            "Mode {mode:?}, labels: {:?}",
                            output
                                .platform_output
                                .accesskit_update
                                .as_ref()
                                .unwrap()
                                .nodes
                                .iter()
                                .filter_map(|(_, node)| node.label())
                                .collect::<Vec<_>>()
                        );
                    }
                    _ => unreachable!(),
                }
                output.drop_without_applying_deltas();
            }
        }
    }

    #[test]
    fn graph_editor_window_is_toggleable_and_supports_detach_and_dock() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = StructViewApp::default();
        app.locale = Locale::English;
        app.open_graph_document(sample().to_root().unwrap());
        let output = render(&ctx, &mut app, vec![]);
        assert!(has_label(&output, "Graph editor"));
        assert!(has_label(&output, "Add entity"));
        assert!(!app.graph_editor.docking.detached);
        output.drop_without_applying_deltas();

        app.graph_editor.open = false;
        let output = open_menu(&ctx, &mut app, TextKey::ViewMenu);
        let events = click(&output, "Graph editor");
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        assert!(app.graph_editor.open);
        // Exercise the same immediate viewport fallback used by docking tests.
        let main = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let window = egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(420.0, 440.0));
        app.graph_editor.docking.detach(main, window, None);
        render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
        let output = render(&ctx, &mut app, vec![]);
        assert!(has_label(&output, app.locale.text(TextKey::DockWindow)));
        let events = click(&output, app.locale.text(TextKey::DockWindow));
        output.drop_without_applying_deltas();
        render(&ctx, &mut app, events).drop_without_applying_deltas();
        assert!(!app.graph_editor.docking.detached);
        assert!(app.graph_editor.open);
        app.visualization = VisualizationMode::Tree;
        let output = render(&ctx, &mut app, vec![]);
        assert!(!has_label(&output, "Graph editor"));
        output.drop_without_applying_deltas();
        app.visualization = VisualizationMode::Graph;
        let output = render(&ctx, &mut app, vec![]);
        assert!(has_label(&output, "Graph editor"));
        assert!(!app.graph_editor.docking.detached);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn graph_editor_window_can_collapse_and_expand_without_losing_document() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.style_mut_of(ctx.theme(), |style| style.animation_time = 0.0);
        let mut app = StructViewApp::default();
        app.locale = Locale::English;
        app.open_graph_document(sample().to_root().unwrap());
        let before = node_to_value(app.root.as_ref().unwrap()).unwrap();
        let frame = |app: &mut StructViewApp, time, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 700.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| app.show_graph_editor(ui.ctx()),
            )
        };
        frame(&mut app, 0.0, vec![]).drop_without_applying_deltas();
        let events = |title| {
            vec![
                egui::Event::PointerMoved(title),
                egui::Event::PointerButton {
                    pos: title,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: title,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let id = app
            .graph_editor
            .docking
            .area_id("graph-editor-window")
            .with("collapsing");
        for (index, expected) in [false, true].into_iter().enumerate() {
            let time = index as f64 + 1.0;
            let output = frame(&mut app, time, vec![]);
            let title = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Graph editor" => {
                        Some(text.pos + egui::vec2(10.0, 5.0))
                    }
                    _ => None,
                })
                .expect("Editor title is visible");
            output.drop_without_applying_deltas();
            frame(&mut app, time + 0.1, events(title)).drop_without_applying_deltas();
            frame(&mut app, time + 0.2, events(title)).drop_without_applying_deltas();
            assert_eq!(
                egui::collapsing_header::CollapsingState::load(&ctx, id)
                    .unwrap()
                    .is_open(),
                expected,
            );
            assert!(app.graph_editor.open);
            assert_eq!(node_to_value(app.root.as_ref().unwrap()).unwrap(), before);
        }
    }

    #[test]
    fn graph_editor_window_commands_open_forms_attached_and_detached() {
        for detached in [false, true] {
            for key in [
                TextKey::GraphAddNode,
                TextKey::GraphAddEdge,
                TextKey::GraphAddGroup,
                TextKey::GraphSettings,
            ] {
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                let mut app = StructViewApp::default();
                app.locale = Locale::English;
                app.open_graph_document(sample().to_root().unwrap());
                if detached {
                    let rect =
                        egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(420.0, 440.0));
                    app.graph_editor
                        .docking
                        .detach(ctx.viewport_rect(), rect, None);
                }
                render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
                let output = render(&ctx, &mut app, vec![]);
                let bounds = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.label() == Some(app.locale.text(key)))
                    .filter_map(|(_, node)| node.bounds())
                    .min_by(|left, right| left.y0.total_cmp(&right.y0))
                    .unwrap();
                let events = vec![
                    egui::Event::PointerMoved(egui::pos2(
                        bounds.x0 as f32 + 5.0,
                        bounds.y0 as f32 + 5.0,
                    )),
                    egui::Event::PointerButton {
                        pos: egui::pos2(bounds.x0 as f32 + 5.0, bounds.y0 as f32 + 5.0),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: egui::pos2(bounds.x0 as f32 + 5.0, bounds.y0 as f32 + 5.0),
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
                output.drop_without_applying_deltas();
                render(&ctx, &mut app, events).drop_without_applying_deltas();
                let target = app
                    .graph_editor
                    .draft
                    .as_ref()
                    .expect("Command opens a form")
                    .target;
                assert!(match key {
                    TextKey::GraphAddNode => matches!(target, Target::Node(None)),
                    TextKey::GraphAddEdge => matches!(target, Target::Edge(None)),
                    TextKey::GraphAddGroup => matches!(target, Target::Partition(None)),
                    TextKey::GraphSettings => matches!(target, Target::Settings),
                    _ => unreachable!(),
                });
                assert!(
                    app.graph_editor.open,
                    "Command must not close the editor window"
                );
                assert_eq!(app.graph_editor.docking.detached, detached);
            }
        }
    }
}
