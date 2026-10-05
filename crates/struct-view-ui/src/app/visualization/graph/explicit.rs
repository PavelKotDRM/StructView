use super::*;

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

fn build_partitioned_graph(root: &JsonNode) -> Result<Option<RelationshipGraph>, String> {
    if root.value_type != JsonValueType::Object {
        return Ok(None);
    }
    let Some(metadata) = object_child(root, "graph") else {
        return Ok(None);
    };
    if metadata.value_type != JsonValueType::Object {
        return Ok(None);
    }
    let Some(graph_type) = object_scalar(metadata, &["type"]) else {
        return Ok(None);
    };
    let graph_type = graph_type.to_ascii_lowercase();
    let is_bipartite = graph_type == "bipartite";
    if !is_bipartite && graph_type != "multipartite" {
        return Ok(None);
    }

    let (Some(partitions), Some(relations)) = (
        object_child(root, "partitions"),
        object_child(root, "relations"),
    ) else {
        return Ok(None);
    };
    let Some(pairs) = object_child(relations, "pairs") else {
        return Ok(None);
    };
    let labels =
        object_child(root, "labels").filter(|node| node.value_type == JsonValueType::Object);
    if partitions.value_type != JsonValueType::Object
        || relations.value_type != JsonValueType::Object
        || pairs.value_type != JsonValueType::Array
    {
        return Ok(None);
    }

    let partition_entries = partitions
        .children
        .iter()
        .filter(|partition| partition.value_type != JsonValueType::Comment)
        .collect::<Vec<_>>();
    if partition_entries.len() < 2
        || (is_bipartite && partition_entries.len() != 2)
        || partition_entries
            .iter()
            .any(|partition| partition.value_type != JsonValueType::Array)
    {
        return Ok(None);
    }
    let partition_names = partition_entries
        .iter()
        .map(|partition| partition.key.as_deref().map(str::to_string))
        .collect::<Option<Vec<_>>>()
        .ok_or("Graph partitions must have names")?;
    let directed = object_child(metadata, "directed")
        .and_then(scalar_value)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let default_direction = record_direction(metadata, EdgeDirection::from_directed(directed))?;

    let mut graph = RelationshipGraph {
        directed,
        partition_names: Some(partition_names),
        ..Default::default()
    };
    let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();
    for (partition_index, partition) in partition_entries.into_iter().enumerate() {
        for member in &partition.children {
            let Some(id) = scalar_value(member) else {
                continue;
            };
            let label_node = labels.and_then(|labels| object_child(labels, &id));
            let label = label_node
                .and_then(scalar_value)
                .unwrap_or_else(|| id.clone());
            let mut search_paths = vec![member.path.clone()];
            if let Some(label_node) = label_node {
                search_paths.push(label_node.path.clone());
            }
            let index = graph.nodes.len();
            graph.nodes.push(GraphNode {
                id: id.clone(),
                label,
                path: member.path.clone(),
                search_paths,
                partition: Some(partition_index),
                attributes: record_attributes(member),
            });
            add_graph_alias(&mut aliases, id, index);
        }
    }

    let mut edges = BTreeSet::new();
    for pair in &pairs.children {
        if pair.value_type != JsonValueType::Array || pair.children.len() < 2 {
            continue;
        }
        let Some(source_id) = scalar_value(&pair.children[0]) else {
            continue;
        };
        let Some(target_id) = scalar_value(&pair.children[1]) else {
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
        if source != target && graph.nodes[*source].partition == graph.nodes[*target].partition {
            continue;
        }

        let label = pair
            .children
            .get(2)
            .and_then(scalar_value)
            .unwrap_or_default();
        let direction = tuple_direction(pair, 3, default_direction)?;
        let (source, target) =
            normalize_edge_endpoints(*source, *target, direction != EdgeDirection::Undirected);
        edges.insert((source, target, label, direction));
    }
    graph.edges = edges
        .into_iter()
        .map(|(source, target, label, direction)| GraphEdge {
            source,
            target,
            label,
            direction,
            attributes: Vec::new(),
        })
        .collect();
    Ok(Some(graph))
}

fn explicit_weighted_matrix_parts(
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

fn build_weighted_matrix_graph(
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

#[derive(Clone)]
struct EdgeListGraphFormat {
    directed: bool,
    default_direction: EdgeDirection,
    multigraph: bool,
    weight_unit: String,
}

fn explicit_toml_multigraph_parts(
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

fn build_explicit_multigraph(
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

fn explicit_json_graph_parts(
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

fn build_explicit_json_graph(
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

pub(super) fn edge_label_with_id(edge: &JsonNode, weight_unit: &str, include_id: bool) -> String {
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

fn explicit_adjacency_parts(root: &JsonNode) -> Option<(&JsonNode, EdgeDirection)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    let adjacency = object_child(root, "adjacency")?;
    let (direction, _) = graph_format(&object_scalar(metadata, &["type"])?.to_ascii_lowercase())?;
    (metadata.value_type == JsonValueType::Object && adjacency.value_type == JsonValueType::Object)
        .then_some((adjacency, direction))
}

fn build_adjacency_graph(adjacency: &JsonNode, direction: EdgeDirection) -> RelationshipGraph {
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

fn normalize_edge_endpoints(source: usize, target: usize, directed: bool) -> (usize, usize) {
    if !directed && source > target {
        (target, source)
    } else {
        (source, target)
    }
}

fn explicit_graph_parts(root: &JsonNode) -> Option<(&JsonNode, &JsonNode, bool)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    let entities = object_child(root, "entities")?;
    let relations = object_child(root, "relations")?;
    let edge_records = object_child(relations, "edges")?;
    if metadata.value_type != JsonValueType::Object
        || entities.value_type != JsonValueType::Object
        || relations.value_type != JsonValueType::Object
        || edge_records.value_type != JsonValueType::Array
        || object_scalar(metadata, &["name"]).is_none()
        || object_child(metadata, "entity_count")
            .is_none_or(|node| node.value_type != JsonValueType::Number)
        || object_child(metadata, "relation_count")
            .is_none_or(|node| node.value_type != JsonValueType::Number)
    {
        return None;
    }
    let directed = match object_child(metadata, "directed").and_then(scalar_value)? {
        value if value.eq_ignore_ascii_case("true") => true,
        value if value.eq_ignore_ascii_case("false") => false,
        _ => return None,
    };
    Some((entities, edge_records, directed))
}

fn build_explicit_graph(
    entities: &JsonNode,
    edge_records: &JsonNode,
    default_direction: EdgeDirection,
) -> Result<RelationshipGraph, String> {
    let directed = default_direction != EdgeDirection::Undirected;
    let mut graph = RelationshipGraph {
        directed,
        ..Default::default()
    };
    let mut entities_by_id = HashMap::with_capacity(entities.children.len());
    for entity in &entities.children {
        if entity.value_type == JsonValueType::Comment {
            continue;
        }
        let Some(id) = entity.key.as_deref() else {
            continue;
        };
        let index = graph.nodes.len();
        let label = scalar_value(entity).unwrap_or_else(|| id.to_string());
        graph.nodes.push(GraphNode {
            id: id.to_string(),
            label,
            path: entity.path.clone(),
            search_paths: vec![entity.path.clone()],
            partition: None,
            attributes: record_attributes(entity),
        });
        entities_by_id.insert(id.to_string(), index);
    }

    let mut edges = BTreeSet::new();
    for record in &edge_records.children {
        if record.value_type != JsonValueType::Array || record.children.len() < 3 {
            continue;
        }
        let Some(source_id) = scalar_value(&record.children[0]) else {
            continue;
        };
        let Some(target_id) = scalar_value(&record.children[1]) else {
            continue;
        };
        let Some(label) = scalar_value(&record.children[2]).filter(|label| !label.is_empty())
        else {
            continue;
        };
        let Some(&source) = entities_by_id.get(&source_id) else {
            continue;
        };
        let Some(&target) = entities_by_id.get(&target_id) else {
            continue;
        };
        let label = record
            .children
            .get(3)
            .and_then(scalar_value)
            .filter(|cardinality| !cardinality.is_empty())
            .map_or(label.clone(), |cardinality| {
                format!("{label} ({cardinality})")
            });
        let direction = tuple_direction(record, 4, default_direction)?;
        let (source, target) =
            normalize_edge_endpoints(source, target, direction != EdgeDirection::Undirected);
        edges.insert((source, target, label, direction));
    }

    graph.edges = edges
        .into_iter()
        .map(|(source, target, label, direction)| GraphEdge {
            source,
            target,
            label,
            direction,
            attributes: Vec::new(),
        })
        .collect();
    Ok(graph)
}
