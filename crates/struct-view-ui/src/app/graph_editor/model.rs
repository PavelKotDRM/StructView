use super::*;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use struct_view_core::graph::EdgeDirection;

#[derive(Clone, Debug)]
pub(in crate::app) struct GraphDocument {
    pub metadata: Map<String, Value>,
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub partitions: Vec<String>,
}

impl GraphDocument {
    pub fn empty() -> Self {
        Self {
            metadata: json!({
                "editor_version": 1, "type": "mixed_multigraph",
                "direction": "directed", "name": "", "weight_unit": ""
            })
            .as_object()
            .expect("object literal")
            .clone(),
            nodes: Vec::new(),
            edges: Vec::new(),
            partitions: Vec::new(),
        }
    }

    pub fn read(root: &JsonNode) -> Result<Option<Self>, String> {
        if !root.children.iter().any(|child| {
            child.key.as_deref() == Some("graph")
                && child
                    .children
                    .iter()
                    .any(|field| field.key.as_deref() == Some("editor_version"))
        }) {
            return Ok(None);
        }
        let value = node_to_value(root)?;
        let Some(metadata) = value.get("graph").and_then(Value::as_object) else {
            return Ok(None);
        };
        let Some(version) = metadata.get("editor_version") else {
            return Ok(None);
        };
        if version.as_u64() != Some(1) {
            return Err("Unsupported graph editor_version".into());
        }
        let array = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| format!("{key} must be an array"))
        };
        let partitions = array("partitions")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "Partition names must be strings".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let document = Self {
            metadata: metadata.clone(),
            nodes: array("nodes")?,
            edges: array("edges")?,
            partitions,
        };
        document.validate()?;
        Ok(Some(document))
    }

    pub fn from_graph(root: &JsonNode, graph: &RelationshipGraph) -> Result<Self, String> {
        if let Some(document) = Self::read(root)? {
            return Ok(document);
        }
        let mut document = Self::empty();
        // Keep the typed source intact: inferred links and tooltip attributes can be lossy.
        document
            .metadata
            .insert("source_document".into(), node_to_value(root)?);
        let unit = document.metadata["source_document"]["graph"]["weight_unit"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        document.metadata.insert("weight_unit".into(), json!(unit));
        document.metadata.insert(
            "direction".into(),
            json!(EdgeDirection::from_directed(graph.directed).as_str()),
        );
        document.partitions = graph.partition_names.clone().unwrap_or_default();
        let mut used = HashSet::new();
        for node in &graph.nodes {
            let mut id = node.id.clone();
            if id.trim().is_empty() || !used.insert(id.clone()) {
                let base = if id.trim().is_empty() { "node" } else { &id };
                id = unique_id(base, &used);
                used.insert(id.clone());
            }
            let attributes = crate::app::edit::find_node(root, &node.path)
                .map(node_to_value)
                .transpose()?
                .filter(Value::is_object)
                .unwrap_or_else(|| attributes_value(&node.attributes));
            let mut record = json!({
                "id": id, "label": node.label,
                "attributes": attributes
            });
            if let Some(partition) = node.partition {
                record["partition"] = json!(
                    document
                        .partitions
                        .get(partition)
                        .ok_or("Invalid source node partition")?
                );
            }
            document.nodes.push(record);
        }
        for (index, edge) in graph.edges.iter().enumerate() {
            let source = document
                .nodes
                .get(edge.source)
                .ok_or("Invalid source endpoint")?;
            let target = document
                .nodes
                .get(edge.target)
                .ok_or("Invalid target endpoint")?;
            let mut record = json!({
                "id": format!("edge{}", index + 1),
                "source": source["id"], "target": target["id"],
                "label": edge.label, "direction": edge.direction.as_str(),
                "attributes": attributes_value(&edge.attributes)
            });
            if let Some(weight) = edge
                .attributes
                .iter()
                .find(|(key, _)| key == "weight" || key == "value")
                .and_then(|(_, value)| serde_json::from_str::<Value>(value).ok())
                .filter(Value::is_number)
            {
                let formatted = if unit.is_empty() {
                    weight.to_string()
                } else {
                    format!("{weight} {unit}")
                };
                let mut removed = false;
                let label = edge
                    .label
                    .split(" · ")
                    .filter(|part| {
                        if !removed && *part == formatted {
                            removed = true;
                            false
                        } else {
                            true
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                record["label"] = json!(label);
                record["weight"] = weight;
            }
            document.edges.push(record);
        }
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.metadata.get("type").and_then(Value::as_str) != Some("mixed_multigraph") {
            return Err("Editor graph.type must be mixed_multigraph".into());
        }
        direction(&Value::Object(self.metadata.clone()))?;
        for key in ["name", "weight_unit"] {
            if self
                .metadata
                .get(key)
                .is_some_and(|value| !value.is_string())
            {
                return Err(format!("graph.{key} must be a string"));
            }
        }
        let mut partitions = HashSet::new();
        for name in &self.partitions {
            if name.trim().is_empty() || !partitions.insert(name) {
                return Err("Partition names must be nonempty and unique".into());
            }
        }
        let mut ids = HashSet::new();
        for node in &self.nodes {
            validate_record(node)?;
            let id = required_string(node, "id")?;
            if !ids.insert(id) {
                return Err(format!("Duplicate node ID: {id}"));
            }
            if let Some(partition) = node.get("partition") {
                let name = partition.as_str().ok_or("partition must be a string")?;
                if !partitions.contains(&name.to_string()) {
                    return Err(format!("Unknown partition: {name}"));
                }
            }
        }
        let mut edge_ids = HashSet::new();
        for edge in &self.edges {
            validate_record(edge)?;
            let id = required_string(edge, "id")?;
            if !edge_ids.insert(id) {
                return Err(format!("Duplicate edge ID: {id}"));
            }
            for key in ["source", "target"] {
                let endpoint = required_string(edge, key)?;
                if !ids.contains(endpoint) {
                    return Err(format!("Unknown {key} node: {endpoint}"));
                }
            }
            direction(edge)?;
            if edge.get("weight").is_some_and(|value| !value.is_number()) {
                return Err("Edge weight must be a JSON number".into());
            }
        }
        Ok(())
    }

    pub fn to_root(&self) -> Result<JsonNode, String> {
        self.validate()?;
        let value = json!({
            "graph": self.metadata, "nodes": self.nodes,
            "edges": self.edges, "partitions": self.partitions
        });
        let content = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        parse_data(&content, Some(DataFormat::Json))
            .map(|(root, _)| root)
            .map_err(|error| error.to_string())
    }

    pub fn put_node(&mut self, index: Option<usize>, record: Value) -> Result<(), String> {
        let mut candidate = self.clone();
        if let Some(index) = index {
            let old = candidate.nodes.get(index).ok_or("Node no longer exists")?;
            let old_id = required_string(old, "id")?.to_string();
            let new_id = required_string(&record, "id")?;
            for edge in &mut candidate.edges {
                for key in ["source", "target"] {
                    if edge[key].as_str() == Some(&old_id) {
                        edge[key] = json!(new_id);
                    }
                }
            }
            candidate.nodes[index] = record;
        } else {
            candidate.nodes.push(record);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn put_edge(&mut self, index: Option<usize>, record: Value) -> Result<(), String> {
        let mut candidate = self.clone();
        if let Some(index) = index {
            *candidate
                .edges
                .get_mut(index)
                .ok_or("Edge no longer exists")? = record;
        } else {
            candidate.edges.push(record);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn delete_node(&mut self, index: usize) -> Result<(), String> {
        let id = required_string(self.nodes.get(index).ok_or("Node no longer exists")?, "id")?
            .to_string();
        self.edges.retain(|edge| {
            edge["source"].as_str() != Some(&id) && edge["target"].as_str() != Some(&id)
        });
        self.nodes.remove(index);
        Ok(())
    }

    pub fn next_id(&self, edge: bool) -> String {
        let records = if edge { &self.edges } else { &self.nodes };
        let used = records
            .iter()
            .filter_map(|record| record["id"].as_str())
            .map(str::to_string)
            .collect();
        unique_id(if edge { "edge" } else { "node" }, &used)
    }
}

fn unique_id(base: &str, used: &HashSet<String>) -> String {
    (1..)
        .map(|index| format!("{base}{index}"))
        .find(|id| !used.contains(id))
        .expect("finite ID set")
}

fn attributes_value(attributes: &[(String, String)]) -> Value {
    Value::Object(
        attributes
            .iter()
            .map(|(key, value)| (key.clone(), json!(value)))
            .collect(),
    )
}

pub(super) fn required_string<'a>(record: &'a Value, key: &str) -> Result<&'a str, String> {
    record
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{key} must be a nonempty string"))
}

fn validate_record(record: &Value) -> Result<(), String> {
    if !record.is_object() {
        return Err("Graph record must be an object".into());
    }
    if record.get("label").is_some_and(|value| !value.is_string()) {
        return Err("label must be a string".into());
    }
    if record
        .get("attributes")
        .is_some_and(|value| !value.is_object())
    {
        return Err("attributes must be a JSON object".into());
    }
    // Aliases would make the standard graph adapter ambiguous.
    for key in [
        "_id",
        "$id",
        "name",
        "title",
        "from",
        "to",
        "directed",
        "bidirectional",
    ] {
        if record.get(key).is_some() {
            return Err(format!(
                "Use canonical graph fields, put {key} in attributes"
            ));
        }
    }
    Ok(())
}

fn direction(record: &Value) -> Result<EdgeDirection, String> {
    EdgeDirection::parse(required_string(record, "direction")?)
}
