use super::*;

pub(super) fn build_partitioned_graph(
    root: &JsonNode,
) -> Result<Option<RelationshipGraph>, String> {
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
