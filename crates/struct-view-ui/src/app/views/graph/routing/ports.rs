use super::*;
pub(in crate::app::views) use struct_view_routing::orthogonal::EdgePorts as GraphEdgePorts;
use struct_view_routing::orthogonal::{EdgePortAxes, Point as RoutePoint, PortAxis};

/// Nodes with more incident edges than this use geometric ports for every edge: a left or right
/// side has room for only about seven lanes.
const GRAPH_HORIZONTAL_PORT_DEGREE_LIMIT: usize = 7;

#[cfg(test)]
pub(in crate::app::views) fn graph_edge_ports(
    node_positions: &[Pos2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let node_sizes = vec![GRAPH_NODE_SIZE; node_positions.len()];
    graph_edge_ports_sized(node_positions, &node_sizes, edge_endpoints)
}

pub(in crate::app::views) fn graph_edge_ports_sized(
    node_positions: &[Pos2],
    node_sizes: &[Vec2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let positions = node_positions
        .iter()
        .map(|point| RoutePoint::new(point.x, point.y))
        .collect::<Vec<_>>();
    let sizes = node_sizes
        .iter()
        .map(|size| struct_view_routing::orthogonal::Size::new(size.x, size.y))
        .collect::<Vec<_>>();
    let mut degrees = vec![0_usize; node_positions.len()];
    for &(source, target) in edge_endpoints {
        if let Some(degree) = degrees.get_mut(source) {
            *degree += 1;
        }
        if let Some(degree) = degrees.get_mut(target) {
            *degree += 1;
        }
    }
    // Layered layouts place nodes in columns, so an edge between columns leaves and enters
    // through the left and right sides; edges within a column keep the top and bottom sides.
    let axes = edge_endpoints
        .iter()
        .map(|&(source, target)| {
            let crosses_columns = match (node_positions.get(source), node_positions.get(target)) {
                (Some(from), Some(to)) => from.x != to.x,
                _ => false,
            };
            let axis = |node: usize| {
                if crosses_columns
                    && degrees
                        .get(node)
                        .is_some_and(|&degree| degree <= GRAPH_HORIZONTAL_PORT_DEGREE_LIMIT)
                {
                    PortAxis::Horizontal
                } else {
                    PortAxis::Geometric
                }
            };
            EdgePortAxes {
                source: axis(source),
                target: axis(target),
            }
        })
        .collect::<Vec<_>>();
    struct_view_routing::orthogonal::assign_edge_ports_for_sizes(
        &positions,
        &sizes,
        edge_endpoints,
        &axes,
    )
    .expect("the UI graph layout supplies valid nodes and edge endpoints")
}
