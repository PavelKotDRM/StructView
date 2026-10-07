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
        match self {
            Self::Left | Self::Right => node_size.height / 2.0 - 10.0,
            Self::Top | Self::Bottom => node_size.width / 2.0 - 16.0,
        }
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

/// Assign deterministic, neighbor-ordered ports to directed edges.
pub fn assign_edge_ports(
    node_positions: &[Point],
    node_size: Size,
    edge_endpoints: &[(usize, usize)],
) -> RoutingResult<Vec<EdgePorts>> {
    if !node_size.is_valid() || node_positions.iter().any(|point| !point.is_finite()) {
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
                NodeSide::for_direction(direction, node_size),
                NodeSide::for_direction(-direction, node_size),
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

    for ((_, side), mut endpoints) in groups {
        endpoints.sort_by(|left, right| {
            left.opposite_coordinate
                .total_cmp(&right.opposite_coordinate)
                .then_with(|| left.edge_index.cmp(&right.edge_index))
        });
        let lane_spacing = (2.0 * side.max_lane_offset(node_size)
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
