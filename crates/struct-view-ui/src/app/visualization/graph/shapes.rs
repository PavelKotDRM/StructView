//! Strict adapters for common node-link, adjacency, and Cytoscape documents.

use super::explicit::{edge_label_with_id, graph_format, record_direction};
use super::*;

mod edges;
mod nodes;
#[cfg(test)]
mod tests;

fn items(node: &JsonNode) -> impl Iterator<Item = &JsonNode> {
    node.children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment)
}

fn field<'a>(node: &'a JsonNode, names: &[&str]) -> Result<Option<&'a JsonNode>, String> {
    let mut found = names.iter().filter_map(|name| object_child(node, name));
    let first = found.next();
    if found.next().is_some() {
        return Err(format!(
            "Ambiguous {} fields at {}",
            names.join("/"),
            node.path
        ));
    }
    Ok(first)
}

fn boolean(node: &JsonNode, name: &str) -> Result<Option<bool>, String> {
    object_child(node, name)
        .map(|value| {
            if value.value_type != JsonValueType::Bool {
                return Err(format!("{name} must be a boolean at {}", value.path));
            }
            match value.display_value.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("Invalid {name} at {}", value.path)),
            }
        })
        .transpose()
}

fn identity(node: &JsonNode) -> Result<String, String> {
    if !matches!(
        node.value_type,
        JsonValueType::String | JsonValueType::Number
    ) {
        return Err(format!(
            "Node ID must be a string or integer at {}",
            node.path
        ));
    }
    scalar_value(node).ok_or_else(|| format!("Invalid node ID at {}", node.path))
}

fn data(node: &JsonNode) -> Result<&JsonNode, String> {
    match object_child(node, "data") {
        Some(value) if value.value_type == JsonValueType::Object => Ok(value),
        Some(value) => Err(format!("Element data must be an object at {}", value.path)),
        None => Err(format!("Cytoscape element requires data at {}", node.path)),
    }
}

pub(super) fn build(root: &JsonNode) -> Result<Option<RelationshipGraph>, String> {
    if root.value_type != JsonValueType::Object {
        return Ok(None);
    }
    let metadata =
        object_child(root, "graph").filter(|node| node.value_type == JsonValueType::Object);
    let networkx = object_child(root, "directed").is_some()
        && object_child(root, "multigraph").is_some()
        && object_child(root, "nodes").is_some();
    let nested = metadata.filter(|node| {
        !networkx && {
            ["nodes", "edges", "links", "elements", "adjacency"]
                .iter()
                .any(|name| object_child(node, name).is_some())
        }
    });
    let container = nested.unwrap_or(root);
    if nested.is_some()
        && ["nodes", "edges", "links", "elements", "adjacency"]
            .iter()
            .any(|name| object_child(root, name).is_some())
    {
        return Err("Ambiguous graph containers at the root and inside graph".to_string());
    }
    let elements = object_child(container, "elements");
    let nodes = object_child(container, "nodes");
    let edge_fields = ["edges", "links"]
        .iter()
        .filter_map(|name| object_child(container, name))
        .collect::<Vec<_>>();
    let edges = edge_fields.first().copied();
    let adjacency = object_child(container, "adjacency");
    let adjacency_array = adjacency.filter(|value| value.value_type == JsonValueType::Array);
    let marked = object_scalar(metadata.unwrap_or(container), &["type"])
        .is_some_and(|kind| graph_format(&kind.to_ascii_lowercase()).is_some())
        || object_child(root, "directed").is_some()
        || object_child(container, "directed").is_some();
    let keyed_adjacency =
        adjacency.filter(|value| value.value_type == JsonValueType::Object && marked);
    if elements.is_none()
        && !(nodes.is_some() && (edges.is_some() || adjacency_array.is_some()))
        && keyed_adjacency.is_none()
    {
        if marked
            && (nodes.is_some() || edges.is_some() || adjacency.is_some())
            && object_child(container, "adjacency_matrix").is_none()
        {
            return Err(
                "Graph requires nodes and edges/links, or a valid adjacency container".to_string(),
            );
        }
        return Ok(None);
    }
    if !marked && elements.is_none() {
        let plausible_nodes = nodes.is_some_and(|nodes| {
            items(nodes).any(|node| {
                matches!(
                    node.value_type,
                    JsonValueType::String | JsonValueType::Number
                ) || object_scalar(node, &["id", "_id", "$id"]).is_some()
                    || nodes.value_type == JsonValueType::Object
            })
        });
        let plausible_edges = edges.is_some_and(|edges| {
            items(edges).any(|edge| {
                edge.value_type == JsonValueType::Array && edge.data_child_count() >= 2
                    || object_child(edge, "source").is_some()
                    || object_child(edge, "from").is_some()
            })
        });
        if !plausible_nodes || !plausible_edges {
            return Ok(None);
        }
    }
    if let Some(elements) = elements
        && !marked
    {
        let records = match elements.value_type {
            JsonValueType::Object => object_child(elements, "nodes")
                .map(|nodes| items(nodes).collect::<Vec<_>>())
                .unwrap_or_default(),
            JsonValueType::Array => items(elements).collect(),
            _ => return Ok(None),
        };
        if !records.iter().any(|node| {
            object_child(node, "data").is_some_and(|data| object_child(data, "id").is_some())
        }) {
            return Ok(None);
        }
    }
    if edge_fields.len() > 1 {
        return Err("Ambiguous edges/links fields".to_string());
    }
    if edges.is_some() && adjacency.is_some() {
        return Err("Ambiguous graph: both edge list and adjacency supplied".to_string());
    }
    if elements.is_some() && (nodes.is_some() || edges.is_some() || adjacency.is_some()) {
        return Err("Ambiguous Cytoscape elements and node-link fields".to_string());
    }
    let settings = if networkx {
        root
    } else {
        metadata.unwrap_or(container)
    };
    let kind = object_scalar(settings, &["type"]);
    let (mut direction, mut multigraph) = match kind.as_deref() {
        Some(kind) => graph_format(&kind.to_ascii_lowercase())
            .ok_or_else(|| format!("Unsupported graph type: {kind}"))?,
        None => (EdgeDirection::Directed, false),
    };
    for config in [root, settings] {
        if let Some(directed) = boolean(config, "directed")? {
            direction = EdgeDirection::from_directed(directed);
        }
        if let Some(multi) = boolean(config, "multigraph")? {
            multigraph = multi;
        }
        direction = record_direction(config, direction)?;
    }
    let unit = object_scalar(settings, &["weight_unit"]).unwrap_or_default();
    let mut builder = Builder {
        graph: RelationshipGraph {
            directed: direction != EdgeDirection::Undirected,
            ..Default::default()
        },
        ids: HashMap::new(),
        direction,
        multigraph,
        deduplicate_mirrors: adjacency.is_some(),
        unit,
        unique: HashMap::new(),
        adjacency_keys: HashMap::new(),
    };
    if let Some(elements) = elements {
        match elements.value_type {
            JsonValueType::Object => {
                let nodes = object_child(elements, "nodes").ok_or("elements.nodes is required")?;
                let edges = object_child(elements, "edges").ok_or("elements.edges is required")?;
                builder.nodes(nodes, true)?;
                builder.edges(edges, true)?;
            }
            JsonValueType::Array => {
                for element in items(elements) {
                    let record = data(element)?;
                    if object_child(record, "source").is_none()
                        && object_child(record, "target").is_none()
                    {
                        builder.node(record, None, element)?;
                    }
                }
                for element in items(elements) {
                    let record = data(element)?;
                    if object_child(record, "source").is_some()
                        || object_child(record, "target").is_some()
                    {
                        builder.edge(record, None, None, None, Some(element))?;
                    }
                }
            }
            _ => return Err("elements must be an object or array".to_string()),
        }
    } else if let Some(adjacency) = keyed_adjacency {
        builder.keyed_adjacency(adjacency, nodes)?;
    } else {
        builder.nodes(nodes.ok_or("nodes are required")?, false)?;
        if let Some(adjacency) = adjacency_array {
            if adjacency.data_child_count() != builder.graph.nodes.len() {
                return Err("NetworkX adjacency rows must match node count".to_string());
            }
            for (index, row) in items(adjacency).enumerate() {
                if row.value_type != JsonValueType::Array {
                    return Err(format!("Adjacency row must be an array at {}", row.path));
                }
                let source = builder.graph.nodes[index].id.clone();
                for neighbor in items(row) {
                    let target = field(neighbor, &["id", "target"])?.ok_or_else(|| {
                        format!("Adjacency entry requires id at {}", neighbor.path)
                    })?;
                    let target = identity(target)?;
                    builder.edge(neighbor, Some(&source), Some(&target), None, None)?;
                }
            }
        } else {
            builder.edges(edges.ok_or("edges or links are required")?, false)?;
        }
    }
    if !builder.multigraph
        && nodes.is_some_and(|nodes| nodes.value_type == JsonValueType::Array)
        && elements.is_none()
        && adjacency.is_none()
    {
        builder.graph.edges.sort_by(|left, right| {
            (left.source, left.target, &left.label, left.direction).cmp(&(
                right.source,
                right.target,
                &right.label,
                right.direction,
            ))
        });
    }
    Ok(Some(builder.graph))
}

struct Builder {
    graph: RelationshipGraph,
    ids: HashMap<String, usize>,
    direction: EdgeDirection,
    multigraph: bool,
    deduplicate_mirrors: bool,
    unit: String,
    unique: HashMap<(usize, usize, String, EdgeDirection), usize>,
    adjacency_keys: HashMap<(usize, usize, String), (String, EdgeDirection)>,
}
