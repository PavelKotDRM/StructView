use std::collections::{BTreeSet, HashMap};

use struct_view_core::parser::{JsonNode, JsonValueType};

/// Узел графа сущностей, найденный по идентификатору или определению схемы.
#[derive(Debug, Clone)]
pub(in crate::app) struct GraphNode {
    pub(in crate::app) id: String,
    pub(in crate::app) label: String,
    pub(in crate::app) path: String,
    pub(in crate::app) search_paths: Vec<String>,
    pub(in crate::app) partition: Option<usize>,
}

/// Направленное ребро от сущности, содержащей ссылку, к её целевой сущности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct GraphEdge {
    pub(in crate::app) source: usize,
    pub(in crate::app) target: usize,
    pub(in crate::app) label: String,
}

/// Граф идентифицированных сущностей и распознанных ссылок между ними.
#[derive(Debug)]
pub(in crate::app) struct RelationshipGraph {
    pub(in crate::app) nodes: Vec<GraphNode>,
    pub(in crate::app) edges: Vec<GraphEdge>,
    pub(in crate::app) directed: bool,
    pub(in crate::app) partition_names: Option<Vec<String>>,
}

impl Default for RelationshipGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            directed: true,
            partition_names: None,
        }
    }
}

/// Построить граф по полям `id`, `_id`, `$id`, `$ref` и именам зависимостей.
pub(in crate::app) fn build_relationship_graph(root: &JsonNode) -> RelationshipGraph {
    if let Some(graph) = build_partitioned_graph(root) {
        return graph;
    }
    if let Some((nodes, edges, directed)) = explicit_toml_multigraph_parts(root) {
        return build_explicit_multigraph(nodes, edges, directed);
    }
    if let Some((entities, edge_records, directed)) = explicit_graph_parts(root) {
        return build_explicit_graph(entities, edge_records, directed);
    }
    if let Some((nodes, edge_records, format)) = explicit_json_graph_parts(root) {
        return build_explicit_json_graph(nodes, edge_records, format);
    }
    if let Some((node_order, matrix, weight_unit)) = explicit_weighted_matrix_parts(root) {
        return build_weighted_matrix_graph(node_order, matrix, &weight_unit);
    }
    if let Some(adjacency) = explicit_undirected_adjacency(root) {
        return build_undirected_adjacency_graph(adjacency);
    }

    let mut graph = RelationshipGraph::default();
    let mut nodes_by_path = HashMap::new();
    let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();
    collect_graph_nodes(
        root,
        "#",
        false,
        &mut graph.nodes,
        &mut nodes_by_path,
        &mut aliases,
    );

    let mut edges = BTreeSet::new();
    collect_graph_edges(root, None, &nodes_by_path, &aliases, &mut edges);
    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
        })
        .collect();
    graph
}

fn build_partitioned_graph(root: &JsonNode) -> Option<RelationshipGraph> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    if metadata.value_type != JsonValueType::Object {
        return None;
    }
    let graph_type = object_scalar(metadata, &["type"])?.to_ascii_lowercase();
    let is_bipartite = graph_type == "bipartite";
    if !is_bipartite && graph_type != "multipartite" {
        return None;
    }

    let partitions = object_child(root, "partitions")?;
    let relations = object_child(root, "relations")?;
    let pairs = object_child(relations, "pairs")?;
    let labels =
        object_child(root, "labels").filter(|node| node.value_type == JsonValueType::Object);
    if partitions.value_type != JsonValueType::Object
        || relations.value_type != JsonValueType::Object
        || pairs.value_type != JsonValueType::Array
    {
        return None;
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
        return None;
    }
    let partition_names = partition_entries
        .iter()
        .map(|partition| partition.key.as_deref().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    let directed = object_child(metadata, "directed")
        .and_then(scalar_value)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));

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
        if graph.nodes[*source].partition == graph.nodes[*target].partition {
            continue;
        }

        let label = pair
            .children
            .get(2)
            .and_then(scalar_value)
            .unwrap_or_default();
        let (source, target) = normalize_edge_endpoints(*source, *target, directed);
        edges.insert((source, target, label));
    }
    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
        })
        .collect();
    Some(graph)
}

fn explicit_weighted_matrix_parts(root: &JsonNode) -> Option<(&JsonNode, &JsonNode, String)> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    if metadata.value_type != JsonValueType::Object
        || !object_scalar(metadata, &["type"])?.eq_ignore_ascii_case("weighted_undirected")
    {
        return None;
    }
    let node_order = object_child(metadata, "node_order")?;
    let matrix = object_child(metadata, "adjacency_matrix")?;
    if node_order.value_type != JsonValueType::Array || matrix.value_type != JsonValueType::Array {
        return None;
    }
    let weight_unit = object_scalar(metadata, &["weight_unit"]).unwrap_or_default();
    Some((node_order, matrix, weight_unit))
}

fn build_weighted_matrix_graph(
    node_order: &JsonNode,
    matrix: &JsonNode,
    weight_unit: &str,
) -> RelationshipGraph {
    let mut graph = RelationshipGraph {
        directed: false,
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
        });
    }

    let mut edges = BTreeSet::new();
    for (source, row) in matrix.children.iter().enumerate().take(graph.nodes.len()) {
        if row.value_type != JsonValueType::Array {
            continue;
        }
        for target in source + 1..graph.nodes.len() {
            let Some(weight) = row.children.get(target) else {
                continue;
            };
            if weight.value_type != JsonValueType::Number
                && weight.value_type != JsonValueType::Float
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
        })
        .collect();
    graph
}

#[derive(Clone)]
struct EdgeListGraphFormat {
    directed: bool,
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
    let directed = match graph_type.as_str() {
        "directed_multigraph" => true,
        "undirected_multigraph" => false,
        _ => return None,
    };
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
            multigraph: true,
            weight_unit: object_scalar(metadata, &["weight_unit"]).unwrap_or_default(),
        },
    ))
}

fn build_explicit_multigraph(
    node_records: &JsonNode,
    edge_records: &JsonNode,
    format: EdgeListGraphFormat,
) -> RelationshipGraph {
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
        });
        add_graph_alias(&mut aliases, id.to_string(), index);
    }

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
        if source == target {
            continue;
        }
        let (source, target) = normalize_edge_endpoints(*source, *target, format.directed);
        graph.edges.push(GraphEdge {
            source,
            target,
            label: explicit_edge_label(edge, &format.weight_unit),
        });
    }
    graph
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
    let (directed, multigraph) = match graph_type.as_str() {
        "directed" | "weighted_directed" => (true, false),
        "undirected" | "weighted_undirected" => (false, false),
        "directed_multigraph" => (true, true),
        "undirected_multigraph" => (false, true),
        _ => return None,
    };

    (nodes.value_type == JsonValueType::Array && edge_records.value_type == JsonValueType::Array)
        .then_some((
            nodes,
            edge_records,
            EdgeListGraphFormat {
                directed,
                multigraph,
                weight_unit: object_scalar(metadata, &["weight_unit"]).unwrap_or_default(),
            },
        ))
}

fn build_explicit_json_graph(
    nodes: &JsonNode,
    edge_records: &JsonNode,
    format: EdgeListGraphFormat,
) -> RelationshipGraph {
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
        if source == target {
            continue;
        }

        let (source, target) = normalize_edge_endpoints(*source, *target, format.directed);
        let label = explicit_edge_label(edge, &format.weight_unit);
        if format.multigraph {
            graph.edges.push(GraphEdge {
                source,
                target,
                label,
            });
        } else {
            unique_edges.insert((source, target, label));
        }
    }

    graph.edges.extend(
        unique_edges
            .into_iter()
            .map(|(source, target, label)| GraphEdge {
                source,
                target,
                label,
            }),
    );
    graph
}

fn explicit_edge_label(edge: &JsonNode, weight_unit: &str) -> String {
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
    if let Some(id) = object_scalar(edge, &["edge_id", "id"]) {
        parts.push(id);
    }
    parts.join(" · ")
}

fn explicit_undirected_adjacency(root: &JsonNode) -> Option<&JsonNode> {
    if root.value_type != JsonValueType::Object {
        return None;
    }
    let metadata = object_child(root, "graph")?;
    let adjacency = object_child(root, "adjacency")?;
    let graph_type = object_scalar(metadata, &["type"])?;
    (metadata.value_type == JsonValueType::Object
        && graph_type.eq_ignore_ascii_case("undirected")
        && adjacency.value_type == JsonValueType::Object)
        .then_some(adjacency)
}

fn build_undirected_adjacency_graph(adjacency: &JsonNode) -> RelationshipGraph {
    let mut graph = RelationshipGraph {
        directed: false,
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
            if source != target {
                let (source, target) = normalize_edge_endpoints(source, target, false);
                edges.insert((source, target, String::new()));
            }
        }
    }

    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
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
    directed: bool,
) -> RelationshipGraph {
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
        if source == target {
            continue;
        }

        let label = record
            .children
            .get(3)
            .and_then(scalar_value)
            .filter(|cardinality| !cardinality.is_empty())
            .map_or(label.clone(), |cardinality| {
                format!("{label} ({cardinality})")
            });
        let (source, target) = normalize_edge_endpoints(source, target, directed);
        edges.insert((source, target, label));
    }

    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
        })
        .collect();
    graph
}

fn collect_graph_nodes(
    node: &JsonNode,
    pointer: &str,
    is_definition: bool,
    nodes: &mut Vec<GraphNode>,
    nodes_by_path: &mut HashMap<String, usize>,
    aliases: &mut HashMap<String, Vec<usize>>,
) {
    if node.value_type == JsonValueType::Comment {
        return;
    }
    if node.value_type == JsonValueType::Metadata {
        for child in &node.children {
            if child.value_type != JsonValueType::Comment {
                collect_graph_nodes(child, pointer, is_definition, nodes, nodes_by_path, aliases);
            }
        }
        return;
    }

    if node.value_type == JsonValueType::Object {
        let identity = object_scalar(node, &["id", "_id", "$id"]);
        let name = object_scalar(node, &["name", "title", "label"]);
        let contains_reference = node
            .children
            .iter()
            .any(|child| child.key.as_deref().is_some_and(is_reference_key));
        let inferred_identity = if identity.is_none() && contains_reference {
            name.clone()
        } else {
            None
        };

        if identity.is_some() || inferred_identity.is_some() || is_definition {
            let id = identity
                .or(inferred_identity)
                .unwrap_or_else(|| pointer.to_string());
            let label = name
                .or_else(|| node.key.clone())
                .unwrap_or_else(|| id.clone());
            let index = nodes.len();
            nodes.push(GraphNode {
                id: id.clone(),
                label,
                path: node.path.clone(),
                search_paths: std::iter::once(node.path.clone())
                    .chain(node.children.iter().map(|child| child.path.clone()))
                    .collect(),
                partition: None,
            });
            nodes_by_path.insert(node.path.clone(), index);
            add_graph_alias(aliases, id, index);
            add_graph_alias(aliases, pointer.to_string(), index);
        }
    }

    let is_definition_container = node
        .key
        .as_deref()
        .is_some_and(|key| matches!(key, "definitions" | "$defs" | "schemas"));
    for child in &node.children {
        let child_pointer = append_json_pointer(pointer, child.key.as_deref().unwrap_or_default());
        let child_is_definition =
            is_definition_container && child.value_type == JsonValueType::Object;
        collect_graph_nodes(
            child,
            &child_pointer,
            child_is_definition,
            nodes,
            nodes_by_path,
            aliases,
        );
    }
}

fn add_graph_alias(aliases: &mut HashMap<String, Vec<usize>>, alias: String, index: usize) {
    let indices = aliases.entry(alias).or_default();
    if !indices.contains(&index) {
        indices.push(index);
    }
}

fn collect_graph_edges(
    node: &JsonNode,
    source: Option<usize>,
    nodes_by_path: &HashMap<String, usize>,
    aliases: &HashMap<String, Vec<usize>>,
    edges: &mut BTreeSet<(usize, usize, String)>,
) {
    let source = nodes_by_path.get(&node.path).copied().or(source);
    if node.value_type == JsonValueType::Object
        && let Some(source) = source
    {
        for child in &node.children {
            if let Some(label) = child.key.as_deref().filter(|key| is_reference_key(key)) {
                let mut references = Vec::new();
                collect_reference_values(child, &mut references);
                for reference in references {
                    if let Some(targets) = aliases.get(&reference)
                        && let [target] = targets.as_slice()
                        && *target != source
                    {
                        edges.insert((source, *target, label.to_string()));
                    }
                }
            }
        }
    }

    for child in &node.children {
        collect_graph_edges(child, source, nodes_by_path, aliases, edges);
    }
}

fn collect_reference_values(node: &JsonNode, references: &mut Vec<String>) {
    match node.value_type {
        JsonValueType::Comment => {}
        JsonValueType::Metadata => {
            for child in &node.children {
                if child.value_type != JsonValueType::Comment {
                    collect_reference_values(child, references);
                }
            }
        }
        JsonValueType::String => {
            if let Ok(value) = serde_json::from_str::<String>(&node.display_value) {
                references.push(value);
            }
        }
        JsonValueType::Number | JsonValueType::Float | JsonValueType::Bool => {
            references.push(node.display_value.clone());
        }
        JsonValueType::Array => {
            for child in &node.children {
                collect_reference_values(child, references);
            }
        }
        JsonValueType::Object => {
            for child in &node.children {
                if child
                    .key
                    .as_deref()
                    .is_some_and(|key| matches!(key, "id" | "_id" | "$id" | "$ref"))
                {
                    collect_reference_values(child, references);
                }
            }
        }
        JsonValueType::DateTime | JsonValueType::Null => {}
    }
}

fn object_scalar(node: &JsonNode, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|candidate| {
        node.children.iter().find_map(|child| {
            child
                .key
                .as_deref()
                .filter(|key| key.eq_ignore_ascii_case(candidate))
                .and_then(|_| scalar_value(child))
        })
    })
}

fn object_child<'a>(node: &'a JsonNode, key: &str) -> Option<&'a JsonNode> {
    node.children.iter().find(|child| {
        child
            .key
            .as_deref()
            .is_some_and(|field| field.eq_ignore_ascii_case(key))
    })
}

fn scalar_value(node: &JsonNode) -> Option<String> {
    match node.value_type {
        JsonValueType::String => serde_json::from_str::<String>(&node.display_value).ok(),
        JsonValueType::Number | JsonValueType::Float | JsonValueType::Bool => {
            Some(node.display_value.clone())
        }
        JsonValueType::Metadata => node
            .children
            .iter()
            .find(|child| child.value_type != JsonValueType::Comment)
            .and_then(scalar_value),
        JsonValueType::DateTime
        | JsonValueType::Object
        | JsonValueType::Array
        | JsonValueType::Null
        | JsonValueType::Comment => None,
    }
}

fn is_reference_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if matches!(normalized.as_str(), "id" | "$id") || normalized.ends_with("count") {
        return false;
    }

    normalized.ends_with("id")
        || [
            "ref", "depend", "require", "parent", "child", "target", "source", "owner", "link",
            "relation", "use",
        ]
        .iter()
        .any(|part| normalized.contains(part))
}

fn append_json_pointer(parent: &str, segment: &str) -> String {
    let escaped = segment.replace('~', "~0").replace('/', "~1");
    format!("{parent}/{escaped}")
}
