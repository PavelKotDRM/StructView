use super::*;

impl Builder {
    pub(super) fn keyed_adjacency(
        &mut self,
        adjacency: &JsonNode,
        nodes: Option<&JsonNode>,
    ) -> Result<(), String> {
        if let Some(nodes) = nodes {
            self.nodes(nodes, false)?;
        }
        for entry in items(adjacency) {
            if entry.yaml_key.is_some() {
                return Err("Adjacency keys must be strings".to_string());
            }
            let id = entry.key.as_deref().ok_or("Adjacency source needs a key")?;
            if nodes.is_none() && !self.ids.contains_key(id) {
                self.implicit_node(id, entry);
            }
        }
        for entry in items(adjacency) {
            let source = entry.key.as_deref().ok_or("Adjacency source needs a key")?;
            if !self.ids.contains_key(source) {
                return Err(format!("Unknown adjacency source: {source}"));
            }
            if !matches!(
                entry.value_type,
                JsonValueType::Array | JsonValueType::Object
            ) {
                return Err(format!(
                    "Adjacency neighbors must be an array or object at {}",
                    entry.path
                ));
            }
            for neighbor in items(entry) {
                let target = if entry.value_type == JsonValueType::Object {
                    if neighbor.yaml_key.is_some() {
                        return Err("Adjacency keys must be strings".to_string());
                    }
                    neighbor.key.clone().ok_or("Adjacency target needs a key")?
                } else if neighbor.value_type == JsonValueType::Object {
                    identity(
                        field(neighbor, &["id", "target", "to"])?
                            .ok_or("Adjacency neighbor requires id/target")?,
                    )?
                } else {
                    identity(neighbor)?
                };
                if nodes.is_none() && !self.ids.contains_key(&target) {
                    self.implicit_node(&target, neighbor);
                }
                if neighbor.value_type == JsonValueType::Object {
                    self.edge(neighbor, Some(source), Some(&target), None, None)?;
                } else {
                    let source_index = *self.ids.get(source).ok_or("Unknown adjacency source")?;
                    let target_index = *self
                        .ids
                        .get(&target)
                        .ok_or_else(|| format!("Unknown adjacency target: {target}"))?;
                    let label = if entry.value_type == JsonValueType::Object {
                        let weight =
                            scalar_value(neighbor).ok_or("Adjacency weight must be scalar")?;
                        if self.unit.is_empty() {
                            weight
                        } else {
                            format!("{weight} {}", self.unit)
                        }
                    } else {
                        String::new()
                    };
                    self.push_edge(
                        source_index,
                        target_index,
                        label,
                        self.direction,
                        Vec::new(),
                    );
                }
            }
        }
        Ok(())
    }

    fn implicit_node(&mut self, id: &str, original: &JsonNode) {
        self.ids.insert(id.to_string(), self.graph.nodes.len());
        self.graph.nodes.push(GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            path: original.path.clone(),
            search_paths: vec![original.path.clone()],
            partition: None,
            attributes: Vec::new(),
        });
    }

    pub(super) fn nodes(&mut self, nodes: &JsonNode, cytoscape: bool) -> Result<(), String> {
        if !matches!(
            nodes.value_type,
            JsonValueType::Array | JsonValueType::Object
        ) {
            return Err(format!(
                "nodes must be an array or object at {}",
                nodes.path
            ));
        }
        for node in items(nodes) {
            let record = if cytoscape { data(node)? } else { node };
            let key = if nodes.value_type == JsonValueType::Object {
                Some(node.key.as_deref().ok_or("Keyed node has no key")?)
            } else {
                None
            };
            self.node(record, key, node)?;
        }
        Ok(())
    }

    pub(super) fn node(
        &mut self,
        record: &JsonNode,
        key: Option<&str>,
        original: &JsonNode,
    ) -> Result<(), String> {
        if record.yaml_key.is_some() {
            return Err(format!(
                "Graph dictionary keys must be strings at {}",
                record.path
            ));
        }
        if key.is_some()
            && record.value_type != JsonValueType::Object
            && scalar_value(record).is_none()
        {
            return Err(format!(
                "Node dictionary value must be a record or scalar at {}",
                record.path
            ));
        }
        let id = if record.value_type == JsonValueType::Object {
            let declared = field(record, &["id", "_id", "$id"])?
                .map(identity)
                .transpose()?;
            if let (Some(key), Some(declared)) = (key, declared.as_deref())
                && key != declared
            {
                return Err(format!(
                    "Node dictionary key {key} conflicts with ID {declared}"
                ));
            }
            declared
                .or_else(|| key.map(str::to_string))
                .ok_or_else(|| format!("Node requires ID at {}", record.path))?
        } else {
            match key {
                Some(key) => key.to_string(),
                None => identity(record)?,
            }
        };
        if self.ids.contains_key(&id) {
            return Err(format!("Duplicate node ID: {id}"));
        }
        let label = if record.value_type == JsonValueType::Object {
            object_scalar(record, &["label", "name", "title"]).unwrap_or_else(|| id.clone())
        } else {
            scalar_value(record).unwrap_or_else(|| id.clone())
        };
        self.ids.insert(id.clone(), self.graph.nodes.len());
        self.graph.nodes.push(GraphNode {
            id,
            label,
            path: original.path.clone(),
            partition: None,
            attributes: record_attributes(original),
            search_paths: std::iter::once(original.path.clone())
                .chain(std::iter::once(record.path.clone()))
                .chain(items(record).map(|child| child.path.clone()))
                .collect(),
        });
        Ok(())
    }
}
