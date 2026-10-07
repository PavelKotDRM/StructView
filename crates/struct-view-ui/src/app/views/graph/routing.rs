use super::*;

#[cfg(test)]
use super::calculation::{GraphProgress, GraphStage, build_graph_calculation};
#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::Duration;

mod edges;
mod geometry;
mod grid;
mod model;
mod ports;

#[cfg(test)]
mod tests;

pub(super) use edges::route_graph_edges_with_progress;
pub(in crate::app) use edges::{
    GraphRoutingWorkerSetting, available_graph_routing_workers, graph_routing_worker_count,
};
#[cfg(test)]
use edges::{graph_route_conflicts, route_graph_edges};
pub(super) use geometry::{
    GRAPH_EDGE_OUTLINE_WIDTH, arrow_head_wings, closest_point_on_segment, draw_arrow_head,
    edge_arrowheads, point_to_segment_distance, segment_intersects_rect, segments_intersect,
};
pub(in crate::app::views) use geometry::{GraphRouteSegmentIndex, segments_within_clearance};
#[cfg(test)]
pub(in crate::app::views) use geometry::{box_border_offset, segment_crosses_rect_interior};
#[cfg(test)]
use geometry::{detour_side_preference_penalty, segment_pair_penalty};
pub(in crate::app::views) use grid::GraphRoutingGrid;
pub(in crate::app::views) use model::graph_edge_color;
pub(super) use model::relationship_graph_fingerprint;
pub(in crate::app::views) use ports::{GraphEdgePorts, graph_edge_ports};
pub(in crate::app) use struct_view_routing::orthogonal::RoutingSearchBackend;

pub(in crate::app::views) const GRAPH_ROUTE_CLEARANCE: f32 =
    struct_view_routing::orthogonal::DEFAULT_ROUTE_CLEARANCE;
pub(in crate::app::views) const GRAPH_EDGE_CLEARANCE: f32 =
    struct_view_routing::orthogonal::DEFAULT_EDGE_CLEARANCE;
#[cfg(test)]
const GRAPH_ROUTE_PORT_LEAD: f32 = GRAPH_EDGE_CLEARANCE + 2.0;
#[cfg(test)]
pub(super) use struct_view_routing::orthogonal::{
    GRAPH_EDGE_SHARED_SEGMENT_PENALTY_LIMIT, GRAPH_EDGE_SHARED_SEGMENT_PENALTY_MINIMUM,
    GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY,
};
