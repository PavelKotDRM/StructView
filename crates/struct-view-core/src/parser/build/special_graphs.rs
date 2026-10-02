mod dot;
mod xml;

use serde_json::{Map, Value};

use super::format::DataFormat;

pub(super) type JsonObject = Map<String, Value>;

pub(super) fn parse_special_graph(input: &str, format: DataFormat) -> Result<Value, String> {
    match format {
        DataFormat::Dot => dot::parse(input),
        DataFormat::GraphMl => xml::parse_graphml(input),
        DataFormat::Gexf => xml::parse_gexf(input),
        _ => Err(format!("{format} is not a special graph format")),
    }
}

pub(super) fn graph_document(
    name: Option<String>,
    graph_type: &str,
    source_format: &str,
    graph_attributes: JsonObject,
    graph_metadata: JsonObject,
    nodes: Vec<Value>,
    edges: Vec<Value>,
) -> Value {
    let mut graph = Map::new();
    graph.insert("type".to_string(), Value::String(graph_type.to_string()));
    graph.insert(
        "source_format".to_string(),
        Value::String(source_format.to_string()),
    );
    if let Some(name) = name.filter(|name| !name.is_empty()) {
        graph.insert("name".to_string(), Value::String(name));
    }
    if !graph_attributes.is_empty() {
        graph.insert("attributes".to_string(), Value::Object(graph_attributes));
    }
    graph.extend(graph_metadata);

    let mut document = Map::new();
    document.insert("graph".to_string(), Value::Object(graph));
    document.insert("nodes".to_string(), Value::Array(nodes));
    document.insert("edges".to_string(), Value::Array(edges));
    Value::Object(document)
}

pub(super) fn node_record(id: String, attributes: JsonObject) -> Value {
    let label = ["label", "name", "title"]
        .iter()
        .find_map(|key| attributes.get(*key).and_then(value_as_string))
        .unwrap_or_else(|| id.clone());
    let mut node = Map::new();
    node.insert("id".to_string(), Value::String(id));
    node.insert("label".to_string(), Value::String(label));
    node.insert("attributes".to_string(), Value::Object(attributes));
    Value::Object(node)
}

pub(super) fn edge_record(
    source: String,
    target: String,
    edge_id: Option<String>,
    attributes: JsonObject,
) -> Value {
    let mut edge = Map::new();
    edge.insert("source".to_string(), Value::String(source));
    edge.insert("target".to_string(), Value::String(target));
    if let Some(edge_id) = edge_id.filter(|id| !id.is_empty()) {
        edge.insert("edge_id".to_string(), Value::String(edge_id));
    }
    for key in [
        "label",
        "name",
        "relation",
        "weight",
        "value",
        "cardinality",
        "multiplicity",
        "contract",
    ] {
        if let Some(value) = attributes.get(key) {
            edge.insert(key.to_string(), value.clone());
        }
    }
    edge.insert("attributes".to_string(), Value::Object(attributes));
    Value::Object(edge)
}

pub(super) fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}
