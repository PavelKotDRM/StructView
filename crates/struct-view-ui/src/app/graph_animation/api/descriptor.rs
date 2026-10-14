use crate::app::visualization::RelationshipGraph;
use serde_json::{Value, json};

pub fn graph_descriptor(graph_id: &str, graph: &RelationshipGraph) -> Value {
    json!({
        "graph_id":graph_id,
        "nodes":graph.nodes.iter().map(|node| json!({"id":node.id,"label":node.label})).collect::<Vec<_>>(),
        "edges":graph.edges.iter().enumerate().map(|(index, edge)| json!({
            "index":index, "source":graph.nodes[edge.source].id,
            "target":graph.nodes[edge.target].id, "label":edge.label,
            "direction":edge.direction.as_str()
        })).collect::<Vec<_>>()
    })
}
