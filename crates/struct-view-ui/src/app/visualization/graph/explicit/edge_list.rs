use super::*;

pub(super) fn explicit_toml_multigraph_parts(
    root: &JsonNode,
) -> Option<(&JsonNode, &JsonNode, EdgeListGraphFormat)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    if metadata.value_type != JsonValueType::Object {
        return None;
    }
    let graph_type = object_scalar(metadata, &["type"])?.to_ascii_lowercase();
    let (default_direction, multigraph) = graph_format(&graph_type)?;
    let directed = default_direction != EdgeDirection::Undirected;
    let nodes = object_child(root, "nodes")?;
    let edges = object_child(root, "edges")?;
    if nodes.value_type != JsonValueType::Object || edges.value_type != JsonValueType::Array {
        return None;
    }
    Some((
        nodes,
        edges,
        EdgeListGraphFormat {
            directed,
            default_direction,
            multigraph,
            weight_unit: object_scalar(metadata, &["weight_unit"]).unwrap_or_default(),
        },
    ))
}

pub(super) fn build_explicit_multigraph(
    node_records: &JsonNode,
    edge_records: &JsonNode,
    format: EdgeListGraphFormat,
) -> Result<RelationshipGraph, String> {
    let mut graph = RelationshipGraph {
        directed: format.directed,
        ..Default::default()
    };
    let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();
    for node in &node_records.children {
        if node.value_type == JsonValueType::Comment {
            continue;
        }
        let Some(id) = node.key.as_deref() else {
            continue;
        };
        let label = scalar_value(node).unwrap_or_else(|| id.to_string());
        let index = graph.nodes.len();
        graph.nodes.push(GraphNode {
            id: id.to_string(),
            label,
            path: node.path.clone(),
            search_paths: vec![node.path.clone()],
            partition: None,
            attributes: record_attributes(node),
        });
        add_graph_alias(&mut aliases, id.to_string(), index);
    }

    let mut unique_edges = BTreeSet::new();
    for edge in &edge_records.children {
        if edge.value_type != JsonValueType::Object {
            continue;
        }
        let Some(source_id) = object_scalar(edge, &["source", "from"]) else {
            continue;
        };
        let Some(target_id) = object_scalar(edge, &["target", "to"]) else {
            continue;
        };
        let Some(sources) = aliases.get(&source_id) else {
            continue;
        };
        let [source] = sources.as_slice() else {
            continue;
        };
        let Some(targets) = aliases.get(&target_id) else {
            continue;
        };
        let [target] = targets.as_slice() else {
            continue;
        };
        let direction = record_direction(edge, format.default_direction)?;
        let (source, target) =
            normalize_edge_endpoints(*source, *target, direction != EdgeDirection::Undirected);
        let label = explicit_edge_label(edge, &format.weight_unit);
        if format.multigraph || unique_edges.insert((source, target, label.clone(), direction)) {
            graph.edges.push(GraphEdge {
                source,
                target,
                label,
                direction,
                attributes: record_attributes(edge),
            });
        }
    }
    Ok(graph)
}

pub(super) fn explicit_json_graph_parts(
    root: &JsonNode,
) -> Option<(&JsonNode, &JsonNode, EdgeListGraphFormat)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    let nodes = object_child(root, "nodes")?;
    let edge_records = object_child(root, "edges")?;
    if metadata.value_type != JsonValueType::Object {
        return None;
    }
    let graph_type = object_scalar(metadata, &["type"])?.to_ascii_lowercase();
    let (default_direction, multigraph) = graph_format(&graph_type)?;
    let directed = default_direction != EdgeDirection::Undirected;

    (nodes.value_type == JsonValueType::Array && edge_records.value_type == JsonValueType::Array)
        .then_some((
            nodes,
            edge_records,
            EdgeListGraphFormat {
                directed,
                default_direction,
                multigraph,
                weight_unit: object_scalar(metadata, &["weight_unit"]).unwrap_or_default(),
            },
        ))
}

pub(super) fn build_explicit_json_graph(
    nodes: &JsonNode,
    edge_records: &JsonNode,
    format: EdgeListGraphFormat,
) -> Result<RelationshipGraph, String> {
    let mut graph = RelationshipGraph {
        directed: format.directed,
        ..Default::default()
    };
    let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();
    for node in &nodes.children {
        if node.value_type != JsonValueType::Object {
            continue;
        }
        let Some(id) = object_scalar(node, &["id", "_id", "$id"]) else {
            continue;
        };
        let label = object_scalar(node, &["name", "title", "label"]).unwrap_or_else(|| id.clone());
        let index = graph.nodes.len();
        graph.nodes.push(GraphNode {
            id: id.clone(),
            label,
            path: node.path.clone(),
            search_paths: std::iter::once(node.path.clone())
                .chain(node.children.iter().map(|child| child.path.clone()))
                .collect(),
            partition: None,
            attributes: record_attributes(node),
        });
        add_graph_alias(&mut aliases, id, index);
    }

    let mut unique_edges = BTreeSet::new();
    for edge in &edge_records.children {
        if edge.value_type != JsonValueType::Object {
            continue;
        }
        let Some(source_id) = object_scalar(edge, &["source", "from"]) else {
            continue;
        };
        let Some(target_id) = object_scalar(edge, &["target", "to"]) else {
            continue;
        };
        let Some(sources) = aliases.get(&source_id) else {
            continue;
        };
        let [source] = sources.as_slice() else {
            continue;
        };
        let Some(targets) = aliases.get(&target_id) else {
            continue;
        };
        let [target] = targets.as_slice() else {
            continue;
        };
        let direction = record_direction(edge, format.default_direction)?;
        let (source, target) =
            normalize_edge_endpoints(*source, *target, direction != EdgeDirection::Undirected);
        let label = explicit_edge_label(edge, &format.weight_unit);
        if format.multigraph {
            graph.edges.push(GraphEdge {
                source,
                target,
                label,
                direction,
                attributes: record_attributes(edge),
            });
        } else {
            unique_edges.insert((source, target, label, direction));
        }
    }

    graph.edges.extend(
        unique_edges
            .into_iter()
            .map(|(source, target, label, direction)| GraphEdge {
                source,
                target,
                label,
                direction,
                attributes: Vec::new(),
            }),
    );
    Ok(graph)
}

pub(super) fn explicit_edge_label(edge: &JsonNode, weight_unit: &str) -> String {
    edge_label_with_id(edge, weight_unit, true)
}

pub(in crate::app::visualization::graph) fn edge_label_with_id(
    edge: &JsonNode,
    weight_unit: &str,
    include_id: bool,
) -> String {
    let mut parts = Vec::new();
    if let Some(relation) =
        object_scalar(edge, &["label", "name", "relation"]).filter(|relation| !relation.is_empty())
    {
        parts.push(relation);
    }
    if let Some(weight) = object_scalar(edge, &["weight", "value"]) {
        parts.push(if weight_unit.is_empty() {
            weight
        } else {
            format!("{weight} {weight_unit}")
        });
    }
    if let Some(cardinality) = object_scalar(edge, &["cardinality", "multiplicity"]) {
        parts.push(cardinality);
    }
    if let Some(contract) = object_scalar(edge, &["contract"]) {
        parts.push(contract);
    }
    let identity_fields: &[&str] = if include_id {
        &["edge_id", "id"]
    } else {
        &["edge_id"]
    };
    if let Some(id) = object_scalar(edge, identity_fields) {
        parts.push(id);
    }
    parts.join(" · ")
}
