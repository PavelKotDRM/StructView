use super::*;

pub(super) fn explicit_adjacency_parts(root: &JsonNode) -> Option<(&JsonNode, EdgeDirection)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    let adjacency = object_child(root, "adjacency")?;
    let (direction, _) = graph_format(&object_scalar(metadata, &["type"])?.to_ascii_lowercase())?;
    (metadata.value_type == JsonValueType::Object && adjacency.value_type == JsonValueType::Object)
        .then_some((adjacency, direction))
}

pub(super) fn build_adjacency_graph(
    adjacency: &JsonNode,
    direction: EdgeDirection,
) -> RelationshipGraph {
    let mut graph = RelationshipGraph {
        directed: direction != EdgeDirection::Undirected,
        ..Default::default()
    };
    let mut nodes_by_id = HashMap::with_capacity(adjacency.children.len());

    for entry in &adjacency.children {
        if entry.value_type != JsonValueType::Comment
            && let Some(id) = entry.key.as_deref()
        {
            ensure_adjacency_node(&mut graph, &mut nodes_by_id, id, &entry.path);
        }
    }
    for entry in &adjacency.children {
        if entry.value_type != JsonValueType::Array {
            continue;
        }
        for neighbor in &entry.children {
            if let Some(id) = scalar_value(neighbor) {
                ensure_adjacency_node(&mut graph, &mut nodes_by_id, &id, &neighbor.path);
            }
        }
    }

    let mut edges = BTreeSet::new();
    for entry in &adjacency.children {
        let Some(source_id) = entry.key.as_deref() else {
            continue;
        };
        let Some(&source) = nodes_by_id.get(source_id) else {
            continue;
        };
        if entry.value_type != JsonValueType::Array {
            continue;
        }
        for neighbor in &entry.children {
            let Some(target_id) = scalar_value(neighbor) else {
                continue;
            };
            let Some(&target) = nodes_by_id.get(&target_id) else {
                continue;
            };
            let (source, target) =
                normalize_edge_endpoints(source, target, direction != EdgeDirection::Undirected);
            edges.insert((source, target, String::new()));
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

fn ensure_adjacency_node(
    graph: &mut RelationshipGraph,
    nodes_by_id: &mut HashMap<String, usize>,
    id: &str,
    path: &str,
) -> usize {
    if let Some(&index) = nodes_by_id.get(id) {
        if !graph.nodes[index]
            .search_paths
            .iter()
            .any(|search_path| search_path == path)
        {
            graph.nodes[index].search_paths.push(path.to_string());
        }
        return index;
    }

    let index = graph.nodes.len();
    graph.nodes.push(GraphNode {
        id: id.to_string(),
        label: id.to_string(),
        path: path.to_string(),
        search_paths: vec![path.to_string()],
        partition: None,
        attributes: Vec::new(),
    });
    nodes_by_id.insert(id.to_string(), index);
    index
}
