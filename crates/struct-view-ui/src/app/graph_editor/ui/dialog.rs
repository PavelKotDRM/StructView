use super::*;

impl StructViewApp {
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
        egui::Modal::new(egui::Id::unique("graph-editor-dialog")).show(ctx, |ui| {
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
                            field(ui, locale.text(TextKey::GraphLabel), &mut draft.label);
                            if draft.flowchart {
                                flowchart_shape_choice(ui, &mut draft.shape, locale);
                            }
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
                    if matches!(draft.target, Target::Node(_) | Target::Edge(_)) {
                        if let Err(error) =
                            color_attribute_editor(ui, &mut draft.attributes, "color", locale)
                        {
                            draft.error = Some(error);
                        }
                        if matches!(draft.target, Target::Node(_))
                            && let Err(error) = color_attribute_editor(
                                ui,
                                &mut draft.attributes,
                                "fillcolor",
                                locale,
                            )
                        {
                            draft.error = Some(error);
                        }
                        ui.collapsing(locale.text(TextKey::GraphAdvancedAttributes), |ui| {
                            field(ui, locale.text(TextKey::GraphId), &mut draft.id);
                            ui.label(locale.text(TextKey::GraphAttributes));
                            ui.weak(locale.text(TextKey::GraphAttributesHint));
                            ui.add(
                                egui::TextEdit::multiline(&mut draft.attributes)
                                    .code_editor()
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.collapsing(locale.text(TextKey::GraphElementJson), |ui| match draft
                                .build_record()
                                .and_then(|record| {
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
                            });
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
    pub(super) fn build_record(&self) -> Result<Value, String> {
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
                if self.flowchart {
                    record["attributes"]["shape"] = json!(self.shape);
                }
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

    pub(super) fn apply(&self, document: &mut GraphDocument) -> Result<(), String> {
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

    pub(super) fn delete(&self, document: &mut GraphDocument) -> Result<(), String> {
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

pub(super) fn text<'a>(record: &'a Value, key: &str) -> &'a str {
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

fn flowchart_shape_choice(ui: &mut egui::Ui, value: &mut String, locale: Locale) {
    let choices = [
        ("process", TextKey::GraphShapeProcess),
        ("terminator", TextKey::GraphShapeTerminator),
        ("decision", TextKey::GraphShapeDecision),
        ("input_output", TextKey::GraphShapeInputOutput),
        ("preparation", TextKey::GraphShapePreparation),
        ("subprocess", TextKey::GraphShapeSubprocess),
        ("database", TextKey::GraphShapeDatabase),
        ("connector", TextKey::GraphShapeConnector),
    ];
    let selected = choices
        .iter()
        .find(|(shape, _)| *shape == value)
        .map_or(value.as_str(), |(_, key)| locale.text(*key));
    ui.label(locale.text(TextKey::GraphFlowchartShape));
    egui::ComboBox::from_id_salt("graph-flowchart-shape")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (shape, key) in choices {
                ui.selectable_value(value, shape.to_string(), locale.text(key));
            }
        });
}

fn color_attribute_editor(
    ui: &mut egui::Ui,
    attributes: &mut String,
    key: &str,
    locale: Locale,
) -> Result<(), String> {
    let Ok(Value::Object(mut values)) = serde_json::from_str::<Value>(attributes) else {
        return Ok(());
    };
    let original = values
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let original_color = crate::app::colors::parse_color(&original);
    let mut color = original_color.unwrap_or(egui::Color32::WHITE);
    let color_format = crate::app::colors::show_color_format_picker(ui, locale, key);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(key);
        if crate::app::colors::color_edit_button_srgba(ui, &mut color, locale) {
            changed = true;
        }
        if color_format.changed && original_color.is_some() {
            changed = true;
        }
        if changed {
            values.insert(
                key.to_string(),
                json!(crate::app::colors::format_color_as(
                    &original,
                    color,
                    color_format.format
                )),
            );
        }
    });
    if changed {
        *attributes = serde_json::to_string_pretty(&Value::Object(values))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
