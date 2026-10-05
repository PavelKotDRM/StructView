use super::*;

impl Builder {
    pub(super) fn edges(&mut self, edges: &JsonNode, cytoscape: bool) -> Result<(), String> {
        if !matches!(
            edges.value_type,
            JsonValueType::Array | JsonValueType::Object
        ) {
            return Err(format!(
                "edges must be an array or object at {}",
                edges.path
            ));
        }
        for edge in items(edges) {
            if edge.yaml_key.is_some() {
                return Err(format!(
                    "Edge dictionary keys must be strings at {}",
                    edge.path
                ));
            }
            let key = if edges.value_type == JsonValueType::Object {
                edge.key.as_deref()
            } else {
                None
            };
            self.edge(
                if cytoscape { data(edge)? } else { edge },
                None,
                None,
                key,
                cytoscape.then_some(edge),
            )?;
        }
        Ok(())
    }

    fn endpoint(&self, node: &JsonNode) -> Result<String, String> {
        if node.value_type == JsonValueType::Object {
            return identity(
                field(node, &["id", "_id", "$id"])?.ok_or("Endpoint object requires ID")?,
            );
        }
        identity(node)
    }

    pub(super) fn edge(
        &mut self,
        edge: &JsonNode,
        source_override: Option<&str>,
        target_override: Option<&str>,
        dictionary_key: Option<&str>,
        original: Option<&JsonNode>,
    ) -> Result<(), String> {
        let (source, target, mut label, direction) = match edge.value_type {
            JsonValueType::Object => {
                let endpoint = |names: &[&str], provided: Option<&str>| -> Result<String, String> {
                    let declared = field(edge, names)?
                        .map(|value| self.endpoint(value))
                        .transpose()?;
                    if let Some(value) = provided {
                        if declared
                            .as_deref()
                            .is_some_and(|declared| declared != value)
                        {
                            return Err(format!("Conflicting adjacency endpoint at {}", edge.path));
                        }
                        return Ok(value.to_string());
                    }
                    declared.ok_or_else(|| format!("Edge requires {} at {}", names[0], edge.path))
                };
                (
                    endpoint(&["source", "from"], source_override)?,
                    endpoint(&["target", "to"], target_override)?,
                    self.edge_label(edge)?,
                    record_direction(edge, self.direction)?,
                )
            }
            JsonValueType::Array => {
                let values = items(edge).collect::<Vec<_>>();
                if !(2..=4).contains(&values.len()) {
                    return Err(format!("Edge tuple requires 2-4 items at {}", edge.path));
                }
                let attributes = values
                    .get(2)
                    .filter(|value| value.value_type == JsonValueType::Object);
                let label = if let Some(attributes) = attributes {
                    self.edge_label(attributes)?
                } else {
                    values
                        .get(2)
                        .map(|value| {
                            let label = scalar_value(value)
                                .ok_or("Edge tuple label/weight must be scalar")?;
                            Ok::<_, String>(
                                if !self.unit.is_empty()
                                    && matches!(
                                        value.value_type,
                                        JsonValueType::Number | JsonValueType::Float
                                    )
                                {
                                    format!("{label} {}", self.unit)
                                } else {
                                    label
                                },
                            )
                        })
                        .transpose()?
                        .unwrap_or_default()
                };
                let attribute_direction = match attributes {
                    Some(attributes) => record_direction(attributes, self.direction)?,
                    None => self.direction,
                };
                let tuple_direction = values
                    .get(3)
                    .map(|value| {
                        let text = scalar_value(value).ok_or("Tuple direction must be a string")?;
                        EdgeDirection::parse(&text)
                    })
                    .transpose()?;
                if let Some(direction) = tuple_direction
                    && attributes.is_some_and(|attributes| {
                        ["direction", "directed", "bidirectional"]
                            .iter()
                            .any(|name| object_child(attributes, name).is_some())
                            || object_child(attributes, "attributes")
                                .is_some_and(|attributes| object_child(attributes, "dir").is_some())
                    })
                    && direction != attribute_direction
                {
                    return Err(format!("Conflicting tuple directions at {}", edge.path));
                }
                let direction = tuple_direction.unwrap_or(attribute_direction);
                (
                    self.endpoint(values[0])?,
                    self.endpoint(values[1])?,
                    label,
                    direction,
                )
            }
            _ => return Err(format!("Edge must be an object or tuple at {}", edge.path)),
        };
        if let Some(key) = dictionary_key
            && let Some(declared) = object_scalar(edge, &["id", "edge_id"])
            && declared != key
        {
            return Err(format!(
                "Edge dictionary key {key} conflicts with ID {declared}"
            ));
        }
        if let Some(key) = dictionary_key
            && object_child(edge, "id").is_none()
            && object_child(edge, "edge_id").is_none()
        {
            if !label.is_empty() {
                label.push_str(" · ");
            }
            label.push_str(key);
        }
        let source = *self
            .ids
            .get(&source)
            .ok_or_else(|| format!("Unknown source node: {source} at {}", edge.path))?;
        let target = *self
            .ids
            .get(&target)
            .ok_or_else(|| format!("Unknown target node: {target} at {}", edge.path))?;
        if self.multigraph && self.deduplicate_mirrors && direction == EdgeDirection::Undirected {
            let key = identity(
                object_child(edge, "key").ok_or("Multigraph adjacency requires an edge key")?,
            )?;
            let pair = (source.min(target), source.max(target), key);
            let value = (label.clone(), direction);
            if let Some(previous) = self.adjacency_keys.get(&pair)
                && previous != &value
            {
                return Err(format!(
                    "Conflicting mirrored adjacency edge at {}",
                    edge.path
                ));
            }
            self.adjacency_keys.insert(pair, value);
        }
        let attributes = if let Some(original) = original {
            record_attributes(original)
        } else if edge.value_type == JsonValueType::Array {
            items(edge)
                .nth(2)
                .map(record_attributes)
                .unwrap_or_default()
        } else {
            record_attributes(edge)
        };
        self.push_edge(source, target, label, direction, attributes);
        Ok(())
    }

    pub(super) fn push_edge(
        &mut self,
        source: usize,
        target: usize,
        label: String,
        direction: EdgeDirection,
        attributes: Vec<(String, String)>,
    ) {
        let (source, target) = if direction == EdgeDirection::Undirected && source > target {
            (target, source)
        } else {
            (source, target)
        };
        let key = (source, target, label.clone(), direction);
        let keep_parallel = self.multigraph
            && !(self.deduplicate_mirrors && direction == EdgeDirection::Undirected);
        if !keep_parallel && let Some(&index) = self.unique.get(&key) {
            let edge = &mut self.graph.edges[index];
            for attribute in attributes {
                if !edge.attributes.contains(&attribute) {
                    edge.attributes.push(attribute);
                }
            }
            return;
        }
        self.unique.insert(key, self.graph.edges.len());
        self.graph.edges.push(GraphEdge {
            source,
            target,
            label,
            direction,
            attributes,
        });
    }

    fn edge_label(&self, edge: &JsonNode) -> Result<String, String> {
        boolean(edge, "directed")?;
        for name in ["weight", "value"] {
            if let Some(value) = object_child(edge, name)
                && scalar_value(value).is_none()
            {
                return Err(format!("Edge {name} must be scalar at {}", value.path));
            }
        }
        let mut label = edge_label_with_id(edge, &self.unit, !self.deduplicate_mirrors);
        if let Some(key) = object_child(edge, "key") {
            let key = identity(key)?;
            if !label.is_empty() {
                label.push_str(" · ");
            }
            label.push_str(&key);
        } else if self.multigraph
            && self.deduplicate_mirrors
            && self.direction == EdgeDirection::Undirected
        {
            return Err(format!(
                "Undirected multigraph adjacency requires an edge key at {}",
                edge.path
            ));
        }
        Ok(label)
    }
}
