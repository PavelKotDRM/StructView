use super::*;

mod adjacency;
mod edge_list;
mod legacy;
mod matrices;
mod partitioned;

use adjacency::{build_adjacency_graph, explicit_adjacency_parts};
pub(super) use edge_list::edge_label_with_id;
use edge_list::{
    build_explicit_json_graph, build_explicit_multigraph, explicit_json_graph_parts,
    explicit_toml_multigraph_parts,
};
use legacy::{build_explicit_graph, explicit_graph_parts};
use matrices::{build_weighted_matrix_graph, explicit_weighted_matrix_parts};
use partitioned::build_partitioned_graph;

pub(super) fn build_supported_graph(root: &JsonNode) -> Result<Option<RelationshipGraph>, String> {
    if let Some(metadata) = object_child(root, "graph")
        && (object_child(metadata, "nodes").is_some() || object_child(metadata, "edges").is_some())
    {
        if object_child(root, "nodes").is_some() || object_child(root, "edges").is_some() {
            return Err(
                "Ambiguous graph: nodes/edges exist both inside graph and at the document root"
                    .to_string(),
            );
        }
        let nodes = object_child(metadata, "nodes")
            .ok_or("graph.nodes is required for a nested edge-list graph")?;
        let edges = object_child(metadata, "edges")
            .ok_or("graph.edges is required for a nested edge-list graph")?;
        let kind = object_scalar(metadata, &["type"])
            .ok_or("graph.type is required for an edge-list graph")?;
        let (default_direction, multigraph) = graph_format(&kind.to_ascii_lowercase())
            .ok_or_else(|| format!("Unsupported graph type: {kind}"))?;
        let default_direction = record_direction(metadata, default_direction)?;
        let format = EdgeListGraphFormat {
            directed: default_direction != EdgeDirection::Undirected,
            default_direction,
            multigraph,
            weight_unit: object_scalar(metadata, &["weight_unit"]).unwrap_or_default(),
        };
        if edges.value_type != JsonValueType::Array {
            return Err("graph.edges must be an array".to_string());
        }
        return match nodes.value_type {
            JsonValueType::Array => build_explicit_json_graph(nodes, edges, format).map(Some),
            JsonValueType::Object => build_explicit_multigraph(nodes, edges, format).map(Some),
            _ => Err("graph.nodes must be an array or a keyed object".to_string()),
        };
    }
    if let Some(graph) = build_partitioned_graph(root)? {
        return Ok(Some(graph));
    }
    if let Some((nodes, edges, mut format)) = explicit_toml_multigraph_parts(root) {
        apply_default_direction(root, &mut format)?;
        return build_explicit_multigraph(nodes, edges, format).map(Some);
    }
    if let Some((entities, edge_records, directed)) = explicit_graph_parts(root) {
        let direction = object_child(root, "graph")
            .map(|metadata| record_direction(metadata, EdgeDirection::from_directed(directed)))
            .transpose()?
            .unwrap_or(EdgeDirection::from_directed(directed));
        return build_explicit_graph(entities, edge_records, direction).map(Some);
    }
    if let Some((nodes, edge_records, mut format)) = explicit_json_graph_parts(root) {
        apply_default_direction(root, &mut format)?;
        return build_explicit_json_graph(nodes, edge_records, format).map(Some);
    }
    if let Some((node_order, matrix, weight_unit, direction)) = explicit_weighted_matrix_parts(root)
    {
        let direction = record_direction(
            object_child(root, "graph").ok_or("Missing graph metadata")?,
            direction,
        )?;
        return Ok(Some(build_weighted_matrix_graph(
            node_order,
            matrix,
            &weight_unit,
            direction,
        )));
    }
    if let Some((adjacency, direction)) = explicit_adjacency_parts(root) {
        let direction = record_direction(
            object_child(root, "graph").ok_or("Missing graph metadata")?,
            direction,
        )?;
        return Ok(Some(build_adjacency_graph(adjacency, direction)));
    }
    Ok(None)
}

fn apply_default_direction(
    root: &JsonNode,
    format: &mut EdgeListGraphFormat,
) -> Result<(), String> {
    if let Some(metadata) = object_child(root, "graph") {
        format.default_direction = record_direction(metadata, format.default_direction)?;
        format.directed = format.default_direction != EdgeDirection::Undirected;
    }
    Ok(())
}

pub(super) fn graph_format(kind: &str) -> Option<(EdgeDirection, bool)> {
    match kind {
        "directed" | "weighted_directed" | "mixed" => Some((EdgeDirection::Directed, false)),
        "undirected" | "weighted_undirected" => Some((EdgeDirection::Undirected, false)),
        "bidirectional" | "weighted_bidirectional" => Some((EdgeDirection::Bidirectional, false)),
        "directed_multigraph" | "mixed_multigraph" => Some((EdgeDirection::Directed, true)),
        "undirected_multigraph" => Some((EdgeDirection::Undirected, true)),
        "bidirectional_multigraph" => Some((EdgeDirection::Bidirectional, true)),
        _ => None,
    }
}

fn direction_value(node: &JsonNode) -> Result<EdgeDirection, String> {
    let value = scalar_value(node)
        .ok_or_else(|| format!("Edge direction must be a string in {}", node.path))?;
    EdgeDirection::parse(&value).map_err(|error| format!("{error} in {}", node.path))
}

pub(super) fn record_direction(
    record: &JsonNode,
    default: EdgeDirection,
) -> Result<EdgeDirection, String> {
    let bidirectional = object_child(record, "bidirectional")
        .map(|node| {
            if node.value_type != JsonValueType::Bool {
                return Err(format!("bidirectional must be a boolean in {}", node.path));
            }
            match node.display_value.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("Invalid bidirectional boolean in {}", node.path)),
            }
        })
        .transpose()?;
    if let Some(node) = object_child(record, "direction") {
        let direction = direction_value(node)?;
        if bidirectional == Some(true) && direction != EdgeDirection::Bidirectional {
            return Err(format!(
                "Conflicting direction and bidirectional in {}",
                record.path
            ));
        }
        return Ok(direction);
    }
    if bidirectional == Some(true) {
        return Ok(EdgeDirection::Bidirectional);
    }
    if let Some(node) = object_child(record, "directed") {
        return match scalar_value(node).as_deref() {
            Some("true") => Ok(EdgeDirection::Directed),
            Some("false") => Ok(EdgeDirection::Undirected),
            _ => Err(format!("directed must be true or false in {}", node.path)),
        };
    }
    // Also recognize legacy converted DOT documents that only retain the raw attribute.
    if let Some(node) =
        object_child(record, "attributes").and_then(|attributes| object_child(attributes, "dir"))
    {
        return direction_value(node);
    }
    Ok(default)
}

fn tuple_direction(
    record: &JsonNode,
    index: usize,
    default: EdgeDirection,
) -> Result<EdgeDirection, String> {
    record
        .children
        .get(index)
        .map(direction_value)
        .transpose()
        .map(|direction| direction.unwrap_or(default))
}

#[derive(Clone)]
struct EdgeListGraphFormat {
    directed: bool,
    default_direction: EdgeDirection,
    multigraph: bool,
    weight_unit: String,
}

fn normalize_edge_endpoints(source: usize, target: usize, directed: bool) -> (usize, usize) {
    if !directed && source > target {
        (target, source)
    } else {
        (source, target)
    }
}
