//! Obstacle-aware orthogonal routing with selectable A* backends.
//!
//! The router accepts node positions and edge endpoints in its own `Point`,
//! `Size`, and `Rect` types. UI crates can adapt native geometry without
//! introducing a GUI dependency into this library.
//! [`OrthogonalRouterOptions::search_backend`] selects the built-in,
//! `pathfinding`, or `petgraph` implementation.
//!
//! ```rust
//! use struct_view_routing::orthogonal::{
//!     OrthogonalRouter, OrthogonalRouterOptions, Point, RouteIndex, Size, assign_edge_ports,
//! };
//!
//! let nodes = [
//!     Point::new(40.0, 80.0),
//!     Point::new(260.0, 80.0),
//!     Point::new(480.0, 80.0),
//! ];
//! let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
//! let router = OrthogonalRouter::new(&nodes, &[0, 1, 2], options)?;
//! let ports = assign_edge_ports(&nodes, options.node_size, &[(0, 2)])?;
//! let already_routed = RouteIndex::new(&[], options.edge_clearance)?;
//! let route = router.route_edge(0, 2, ports[0], &already_routed)?;
//!
//! assert_eq!(route.first(), Some(&Point::new(70.0, 80.0)));
//! # Ok::<(), struct_view_routing::RoutingError>(())
//! ```

mod geometry;
mod index;
mod ports;
mod router;

pub use crate::RoutingSearchBackend;

#[cfg(test)]
mod tests;

pub use geometry::{
    Point, Rect, Size, Vector, closest_point_on_segment, cross_product, point_to_segment_distance,
    segment_intersects_rect, segments_intersect, segments_within_clearance,
};
pub use index::{
    GRAPH_EDGE_CROSSING_PENALTY, GRAPH_EDGE_ROUTE_PENALTY, GRAPH_EDGE_SHARED_SEGMENT_PENALTY_LIMIT,
    GRAPH_EDGE_SHARED_SEGMENT_PENALTY_MINIMUM, GRAPH_EDGE_SHARED_SEGMENT_PENALTY_SCALE,
    GRAPH_ROUTE_SHARED_SEGMENT_DETOUR_LIMIT, GRAPH_ROUTE_SHARED_SEGMENT_SEARCH_THRESHOLD,
    GRAPH_ROUTE_SHARED_SEGMENT_VISIBLE_THRESHOLD, RouteIndex, segment_pair_penalty,
};
pub use ports::{EdgePorts, NodeSide, assign_edge_ports};
pub use router::{
    DEFAULT_EDGE_CLEARANCE, DEFAULT_ROUTE_CLEARANCE, DEFAULT_ROUTE_TRACK_LIMIT,
    GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY, GRAPH_ROUTE_TURN_PENALTY, OrthogonalRouter,
    OrthogonalRouterOptions, detour_side_preference_penalty, simplify_route,
};
