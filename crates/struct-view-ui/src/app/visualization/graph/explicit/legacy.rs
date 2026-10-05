use super::*;

pub(super) fn explicit_graph_parts(root: &JsonNode) -> Option<(&JsonNode, &JsonNode, bool)> {
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

pub(super) fn build_explicit_graph(
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
