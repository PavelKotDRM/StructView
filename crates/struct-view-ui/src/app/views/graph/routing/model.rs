use super::*;
use std::hash::{Hash, Hasher};

pub(in crate::app::views::graph) fn relationship_graph_fingerprint(
    graph: &RelationshipGraph,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for node in &graph.nodes {
        node.id.hash(&mut hasher);
        node.label.hash(&mut hasher);
        node.path.hash(&mut hasher);
        node.partition.hash(&mut hasher);
    }
    for edge in &graph.edges {
        edge.source.hash(&mut hasher);
        edge.target.hash(&mut hasher);
        edge.label.hash(&mut hasher);
        edge.direction.hash(&mut hasher);
    }
    graph.directed.hash(&mut hasher);
    graph.partition_names.hash(&mut hasher);
    hasher.finish()
}

pub(in crate::app::views) fn graph_edge_color(label: &str, colors: SyntaxColors) -> egui::Color32 {
    let hash = label.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let palette = [
        colors.number,
        colors.string,
        colors.boolean,
        colors.metadata,
        colors.comment,
        colors.null,
        colors.matched,
        colors.error,
    ];
    palette[(hash % palette.len() as u64) as usize]
}
