use super::*;

pub(super) fn explicit_weighted_matrix_parts(
    root: &JsonNode,
) -> Option<(&JsonNode, &JsonNode, String, EdgeDirection)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    if metadata.value_type != JsonValueType::Object {
        return None;
    }
    let (direction, _) = graph_format(&object_scalar(metadata, &["type"])?.to_ascii_lowercase())?;
    let node_order = object_child(metadata, "node_order")?;
    let matrix = object_child(metadata, "adjacency_matrix")?;
    if node_order.value_type != JsonValueType::Array || matrix.value_type != JsonValueType::Array {
        return None;
    }
    let weight_unit = object_scalar(metadata, &["weight_unit"]).unwrap_or_default();
    Some((node_order, matrix, weight_unit, direction))
}

pub(super) fn build_weighted_matrix_graph(
    node_order: &JsonNode,
    matrix: &JsonNode,
    weight_unit: &str,
    direction: EdgeDirection,
) -> RelationshipGraph {
    let mut graph = RelationshipGraph {
        directed: direction != EdgeDirection::Undirected,
        ..Default::default()
    };
    for (index, node) in node_order.children.iter().enumerate() {
        let id = scalar_value(node).unwrap_or_else(|| format!("node-{index}"));
        graph.nodes.push(GraphNode {
            id: id.clone(),
            label: id,
            path: node.path.clone(),
            search_paths: vec![node.path.clone()],
            partition: None,
            attributes: record_attributes(node),
        });
    }

    let mut edges = BTreeSet::new();
    for (source, row) in matrix.children.iter().enumerate().take(graph.nodes.len()) {
        if row.value_type != JsonValueType::Array {
            continue;
        }
        let first_target = if matches!(
            direction,
            EdgeDirection::Undirected | EdgeDirection::Bidirectional
        ) {
            source
        } else {
            0
        };
        for target in first_target..graph.nodes.len() {
            let Some(weight) = row.children.get(target) else {
                continue;
            };
            if weight.value_type != JsonValueType::Number
                && weight.value_type != JsonValueType::Float
            {
                continue;
            }
            let mantissa = weight
                .display_value
                .trim_start_matches(['+', '-'])
                .split(['e', 'E'])
                .next()
                .unwrap_or("");
            if source == target
                && mantissa.contains('0')
                && mantissa.chars().all(|ch| matches!(ch, '0' | '.'))
            {
                continue;
            }
            let label = if weight_unit.is_empty() {
                weight.display_value.clone()
            } else {
                format!("{} {weight_unit}", weight.display_value)
            };
            edges.insert((source, target, label));
        }
    }

    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
            direction,
            attributes: Vec::new(),
        })
        .collect();
    graph
}
