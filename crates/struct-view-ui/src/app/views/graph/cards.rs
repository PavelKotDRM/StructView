use super::*;

/// Links a card can carry on one side at the routing edge clearance before its lanes crowd.
const GRAPH_CARD_LANE_LIMIT: usize = 22;
/// Spacing between neighbouring lanes on a card side; equals the routing edge clearance.
const GRAPH_CARD_LANE_SPACING: f32 = 8.0;
/// Margin kept free at both ends of a top or bottom side.
const GRAPH_CARD_SIDE_MARGIN: f32 = 16.0;
/// Extra height of a big card, holding its link counts and document path below the label.
const GRAPH_CARD_INFO_HEIGHT: f32 = 40.0;
/// Offsets from a card's top edge to its label row, identifier row, and the two rows of a big
/// card's information block.
pub(in crate::app::views) const GRAPH_CARD_LABEL_OFFSET: f32 = 26.0;
pub(in crate::app::views) const GRAPH_CARD_ID_OFFSET: f32 = 48.0;
pub(in crate::app::views) const GRAPH_CARD_COUNTS_OFFSET: f32 = 84.0;
pub(in crate::app::views) const GRAPH_CARD_PATH_OFFSET: f32 = 100.0;

/// Whether a node with `links` incident edges is drawn as a big card with an information block.
pub(in crate::app::views) fn is_big_card(links: usize) -> bool {
    links > GRAPH_CARD_LANE_LIMIT
}

/// Card size for a node with `links` incident edges. A big card grows wide enough for one lane per
/// link on its top side and gains a block below the label for its counts and document path.
pub(in crate::app::views) fn graph_node_size(links: usize) -> Vec2 {
    if !is_big_card(links) {
        return GRAPH_NODE_SIZE;
    }
    let lanes = GRAPH_CARD_LANE_SPACING * links.saturating_sub(1) as f32;
    let width = (lanes + 2.0 * GRAPH_CARD_SIDE_MARGIN).max(GRAPH_NODE_SIZE.x);
    Vec2::new(width, GRAPH_NODE_SIZE.y + GRAPH_CARD_INFO_HEIGHT)
}

/// Card size of every graph node, in node order.
pub(in crate::app::views) fn graph_node_sizes(graph: &RelationshipGraph) -> Vec<Vec2> {
    let mut links = vec![0_usize; graph.nodes.len()];
    for edge in &graph.edges {
        links[edge.source] += 1;
        links[edge.target] += 1;
    }
    links.into_iter().map(graph_node_size).collect()
}

/// Incoming and outgoing link counts of every node as `(incoming, outgoing)`, counted the way the
/// layout orders them: a reversed link points into its source, and undirected or bidirectional
/// links count both ways.
pub(in crate::app::views) fn graph_link_counts(graph: &RelationshipGraph) -> Vec<(usize, usize)> {
    let mut counts = vec![(0_usize, 0_usize); graph.nodes.len()];
    for edge in &graph.edges {
        if edge.direction != EdgeDirection::Reverse {
            counts[edge.source].1 += 1;
            counts[edge.target].0 += 1;
        }
        if edge.direction != EdgeDirection::Directed {
            counts[edge.target].1 += 1;
            counts[edge.source].0 += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_grow_only_past_the_lane_limit() {
        assert_eq!(graph_node_size(GRAPH_CARD_LANE_LIMIT), GRAPH_NODE_SIZE);
        assert_eq!(
            graph_node_size(GRAPH_CARD_LANE_LIMIT + 1),
            Vec2::new(
                GRAPH_NODE_SIZE.x,
                GRAPH_NODE_SIZE.y + GRAPH_CARD_INFO_HEIGHT
            ),
        );
        assert_eq!(
            graph_node_size(57),
            Vec2::new(480.0, GRAPH_NODE_SIZE.y + GRAPH_CARD_INFO_HEIGHT),
        );
    }

    #[test]
    fn link_counts_are_incoming_and_outgoing() {
        let root = struct_view_core::parser::parse_json(
            r#"{"graph":{"type":"directed_multigraph"},"nodes":[{"id":"a"},{"id":"b"},{"id":"c"}],"edges":[{"source":"a","target":"b"},{"source":"c","target":"b"}]}"#,
        )
        .unwrap();
        let graph = build_relationship_graph(&root);
        let counts = graph_link_counts(&graph);
        let index = |id: &str| graph.nodes.iter().position(|node| node.id == id).unwrap();
        assert_eq!(counts[index("a")], (0, 1));
        assert_eq!(counts[index("b")], (2, 0));
        assert_eq!(counts[index("c")], (0, 1));
    }

    #[test]
    fn big_cards_do_not_overlap_their_neighbours() {
        let nodes = std::iter::once(serde_json::json!({"id": "hub"}))
            .chain((0..30).map(|id| serde_json::json!({"id": format!("s{id}")})))
            .collect::<Vec<_>>();
        let edges = (0..30)
            .map(|id| serde_json::json!({"source": format!("s{id}"), "target": "hub"}))
            .collect::<Vec<_>>();
        let root = struct_view_core::parser::parse_json(
            &serde_json::json!({"nodes": nodes, "edges": edges}).to_string(),
        )
        .unwrap();
        let graph = build_relationship_graph(&root);
        let positions = graph_node_positions(&graph);
        let sizes = graph_node_sizes(&graph);
        let rects = positions
            .iter()
            .zip(&sizes)
            .map(|(center, size)| egui::Rect::from_center_size(*center, *size))
            .collect::<Vec<_>>();
        for (first, a) in rects.iter().enumerate() {
            for (second, b) in rects.iter().enumerate().skip(first + 1) {
                assert!(
                    !a.intersects(*b),
                    "cards {first} and {second} overlap: {a:?} {b:?}"
                );
            }
        }
    }
}
