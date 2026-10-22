use crate::app::visualization::RelationshipGraph;
use serde_json::{Value, json};

pub fn graph_descriptor(graph_id: &str, graph: &RelationshipGraph) -> Value {
    let is_flowchart = graph
        .nodes
        .iter()
        .any(|node| node.flowchart_shape().is_some());
    json!({
        "graph_id":graph_id,
        "type":if is_flowchart {"flowchart"} else {"graph"},
        "nodes":graph.nodes.iter().map(|node| {
            let mut value = json!({"id":node.id,"label":node.label});
            if let Some(shape) = node.flowchart_shape() {
                value["shape"] = json!(shape.as_str());
            }
            value
        }).collect::<Vec<_>>(),
        "edges":graph.edges.iter().enumerate().map(|(index, edge)| json!({
            "index":index, "source":graph.nodes[edge.source].id,
            "target":graph.nodes[edge.target].id, "label":edge.label,
            "direction":edge.direction.as_str()
        })).collect::<Vec<_>>()
    })
}
