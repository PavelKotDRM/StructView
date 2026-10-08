use super::{Point, Size, Vector};
use crate::{RoutingError, RoutingResult};
use std::collections::HashMap;

/// A side of a node used as an edge attachment point.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum NodeSide {
    /// Left border.
    Left,
    /// Right border.
    Right,
    /// Top border.
    Top,
    /// Bottom border.
    Bottom,
}

/// Preferred side orientation for one end of an edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortAxis {
    /// Use the side that faces the other node along the geometric direction between them.
    #[default]
    Geometric,
    /// Use the left or right side facing the other node when the nodes differ horizontally;
    /// otherwise behave like [`Self::Geometric`].
    Horizontal,
}

/// Preferred port orientation for both ends of an edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgePortAxes {
    /// Orientation preferred at the source node.
    pub source: PortAxis,
    /// Orientation preferred at the target node.
    pub target: PortAxis,
}

impl NodeSide {
    fn for_direction(direction: Vector, node_size: Size) -> Self {
        let half_width = node_size.width / 2.0;
        let half_height = node_size.height / 2.0;
        let horizontal_distance = if direction.x.abs() > 0.0 {
            half_width / direction.x.abs()
        } else {
            f32::INFINITY
        };
        let vertical_distance = if direction.y.abs() > 0.0 {
            half_height / direction.y.abs()
        } else {
            f32::INFINITY
        };
        if horizontal_distance <= vertical_distance {
            if direction.x > 0.0 {
                Self::Right
            } else {
                Self::Left
            }
        } else if direction.y > 0.0 {
            Self::Bottom
        } else {
            Self::Top
        }
    }

    fn max_lane_offset(self, node_size: Size) -> f32 {
        // Small nodes cannot fit the corner margin; a negative offset would
        // reverse lane order and place ports beyond the node side.
        match self {
            Self::Left | Self::Right => node_size.height / 2.0 - 10.0,
            Self::Top | Self::Bottom => node_size.width / 2.0 - 16.0,
        }
        .max(0.0)
    }
}

/// Port sides and lane offsets for a directed edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePorts {
    /// Source node attachment side.
    pub source_side: NodeSide,
    /// Target node attachment side.
    pub target_side: NodeSide,
    /// Offset along the source side.
    pub source_offset: f32,
    /// Offset along the target side.
    pub target_offset: f32,
}

struct PortEndpoint {
    edge_index: usize,
    is_source: bool,
    opposite_coordinate: f32,
}

/// The side of a node that faces `direction`, honoring a horizontal preference when it applies.
fn side_facing(direction: Vector, node_size: Size, axis: PortAxis) -> NodeSide {
    match axis {
        PortAxis::Horizontal if direction.x > 0.0 => NodeSide::Right,
        PortAxis::Horizontal if direction.x < 0.0 => NodeSide::Left,
        _ => NodeSide::for_direction(direction, node_size),
    }
}

/// Assign deterministic, neighbor-ordered ports to directed edges.
pub fn assign_edge_ports(
    node_positions: &[Point],
    node_size: Size,
    edge_endpoints: &[(usize, usize)],
) -> RoutingResult<Vec<EdgePorts>> {
    let axes = vec![EdgePortAxes::default(); edge_endpoints.len()];
    assign_edge_ports_on_axes(node_positions, node_size, edge_endpoints, &axes)
}

/// Assign ports like [`assign_edge_ports`], choosing each end's side from its [`EdgePortAxes`].
pub fn assign_edge_ports_on_axes(
    node_positions: &[Point],
    node_size: Size,
    edge_endpoints: &[(usize, usize)],
    axes: &[EdgePortAxes],
) -> RoutingResult<Vec<EdgePorts>> {
    let node_sizes = vec![node_size; node_positions.len()];
    assign_edge_ports_for_sizes(node_positions, &node_sizes, edge_endpoints, axes)
}

/// Assign ports like [`assign_edge_ports_on_axes`] for nodes with individual sizes.
pub fn assign_edge_ports_for_sizes(
    node_positions: &[Point],
    node_sizes: &[Size],
    edge_endpoints: &[(usize, usize)],
    axes: &[EdgePortAxes],
) -> RoutingResult<Vec<EdgePorts>> {
    if node_sizes.len() != node_positions.len()
        || node_sizes.iter().any(|size| !size.is_valid())
        || node_positions.iter().any(|point| !point.is_finite())
        || axes.len() != edge_endpoints.len()
    {
        return Err(RoutingError::InvalidGeometry);
    }

    let mut groups: HashMap<(usize, NodeSide), Vec<PortEndpoint>> = HashMap::new();
    let mut ports = Vec::with_capacity(edge_endpoints.len());
    for (edge_index, &(source, target)) in edge_endpoints.iter().enumerate() {
        let (Some(&source_position), Some(&target_position)) =
            (node_positions.get(source), node_positions.get(target))
        else {
            return Err(RoutingError::InvalidNodeIndex);
        };
        let (source_side, target_side) = if source == target {
            (NodeSide::Right, NodeSide::Bottom)
        } else {
            let direction = (target_position - source_position).normalized();
            (
                side_facing(direction, node_sizes[source], axes[edge_index].source),
                side_facing(-direction, node_sizes[target], axes[edge_index].target),
            )
        };
        ports.push(EdgePorts {
            source_side,
            target_side,
            source_offset: 0.0,
            target_offset: 0.0,
        });
        let opposite_coordinate = |node: usize, side: NodeSide| match side {
            NodeSide::Left | NodeSide::Right => node_positions[node].y,
            NodeSide::Top | NodeSide::Bottom => node_positions[node].x,
        };
        groups
            .entry((source, source_side))
            .or_default()
            .push(PortEndpoint {
                edge_index,
                is_source: true,
                opposite_coordinate: opposite_coordinate(target, source_side),
            });
        groups
            .entry((target, target_side))
            .or_default()
            .push(PortEndpoint {
                edge_index,
                is_source: false,
                opposite_coordinate: opposite_coordinate(source, target_side),
            });
    }

    for ((node, side), mut endpoints) in groups {
        endpoints.sort_by(|left, right| {
            left.opposite_coordinate
                .total_cmp(&right.opposite_coordinate)
                .then_with(|| left.edge_index.cmp(&right.edge_index))
        });
        let lane_spacing = (2.0 * side.max_lane_offset(node_sizes[node])
            / endpoints.len().saturating_sub(1).max(1) as f32)
            .min(16.0);
        for (lane, endpoint) in endpoints.iter().enumerate() {
            let offset = (lane as f32 - (endpoints.len() - 1) as f32 / 2.0) * lane_spacing;
            if endpoint.is_source {
                ports[endpoint.edge_index].source_offset = offset;
            } else {
                ports[endpoint.edge_index].target_offset = offset;
            }
        }
    }

    Ok(ports)
}
