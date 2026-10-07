use super::*;
pub(in crate::app::views) use struct_view_routing::orthogonal::EdgePorts as GraphEdgePorts;

pub(in crate::app::views) fn graph_edge_ports(
    node_positions: &[Pos2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let positions = node_positions
        .iter()
        .map(|point| struct_view_routing::orthogonal::Point::new(point.x, point.y))
        .collect::<Vec<_>>();
    struct_view_routing::orthogonal::assign_edge_ports(
        &positions,
        struct_view_routing::orthogonal::Size::new(GRAPH_NODE_SIZE.x, GRAPH_NODE_SIZE.y),
        edge_endpoints,
    )
    .expect("the UI graph layout supplies valid nodes and edge endpoints")
}
