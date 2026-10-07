//! Reusable weighted-graph pathfinding algorithms.
//!
//! Use [`AdjacencyListGraph`] for a ready-made directed graph, or implement
//! [`RoutingGraph`] to adapt an existing model. The crate provides breadth-first
//! search, Dijkstra, A*, Bellman-Ford, Floyd-Warshall, and Yen's K-shortest
//! loopless paths. Algorithms return node and edge sequences in a [`Path`],
//! including the cost of every edge.
//!
//! [`orthogonal`] provides an obstacle-aware orthogonal router with
//! dependency-free 2D geometry types.
//!
//! ```rust
//! use struct_view_routing::{AdjacencyListGraph, dijkstra};
//!
//! let mut graph = AdjacencyListGraph::<&str, &str>::new();
//! graph.add_node("api");
//! graph.add_node("database");
//! graph.add_edge(&"api", &"database", "query", 2.0)?;
//!
//! let path = dijkstra(&graph, "api", "database")?.expect("path exists");
//! assert_eq!(path.nodes, ["api", "database"]);
//! assert_eq!(path.cost, 2.0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod all_pairs;
mod graph;
pub mod orthogonal;
mod search;
mod yen;

pub use all_pairs::{AllPairsPaths, SingleSourcePaths, bellman_ford, floyd_warshall};
pub use graph::{AdjacencyListGraph, GraphBuildError};
pub use search::{
    IndexedNeighbor, IndexedPath, a_star, a_star_indexed, breadth_first_search, dijkstra,
};
pub use yen::yen_k_shortest_paths;

use std::hash::Hash;

/// An outgoing weighted arc in a graph.
#[derive(Clone, Debug, PartialEq)]
pub struct WeightedEdge<N, E> {
    /// The node reached by traversing this edge.
    pub to: N,
    /// An identifier unique to this directed arc.
    pub id: E,
    /// The traversal cost. Shortest-path algorithms require finite costs;
    /// Dijkstra, A*, and Yen require non-negative costs.
    pub weight: f64,
}

impl<N, E> WeightedEdge<N, E> {
    /// Create an outgoing weighted arc.
    pub fn new(to: N, id: E, weight: f64) -> Self {
        Self { to, id, weight }
    }
}

/// A graph that can enumerate its unique nodes and the outgoing arcs of each node.
///
/// Edge identifiers must distinguish parallel directed arcs. For an
/// undirected edge, expose one outgoing arc from each endpoint with an
/// identifier for each direction.
pub trait RoutingGraph {
    /// Node identifier type.
    type Node: Clone + Eq + Hash;
    /// Directed arc identifier type.
    type EdgeId: Clone + Eq + Hash;

    /// Return all nodes, including isolated nodes.
    fn nodes(&self) -> Vec<Self::Node>;

    /// Return all outgoing arcs from `node`.
    fn outgoing_edges(&self, node: &Self::Node) -> Vec<WeightedEdge<Self::Node, Self::EdgeId>>;
}

/// A path through a graph.
///
/// For [`breadth_first_search`], `edge_costs` contains unit costs because the
/// search minimizes hops and intentionally ignores graph weights.
#[derive(Clone, Debug, PartialEq)]
pub struct Path<N, E> {
    /// Ordered node sequence, including both endpoints.
    pub nodes: Vec<N>,
    /// Ordered arc identifiers; one fewer than `nodes`.
    pub edges: Vec<E>,
    /// Cost of each traversed arc, aligned with `edges`.
    pub edge_costs: Vec<f64>,
    /// Sum of `edge_costs`.
    pub cost: f64,
}

/// An error detected while searching a graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingError {
    /// A graph contains a NaN or infinite edge cost.
    NonFiniteWeight,
    /// A non-negative-weight algorithm encountered a negative edge.
    NegativeWeight,
    /// An A* heuristic returned a NaN, infinite, or negative estimate.
    InvalidHeuristic,
    /// A reachable negative cycle prevents a finite shortest path.
    NegativeCycle,
    /// A graph edge references a node not returned by `nodes`.
    UnknownNode,
    /// An indexed graph search referenced a node outside its declared range.
    InvalidNodeIndex,
    /// Routing geometry, dimensions, or options are invalid.
    InvalidGeometry,
    /// No orthogonal route could be found between the requested endpoints.
    NoOrthogonalPath,
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NonFiniteWeight => "Graph edge weights must be finite",
            Self::NegativeWeight => "This algorithm requires non-negative edge weights",
            Self::InvalidHeuristic => "A* heuristic values must be finite and non-negative",
            Self::NegativeCycle => "A reachable negative cycle prevents a finite shortest path",
            Self::UnknownNode => "A graph edge references an unknown node",
            Self::InvalidNodeIndex => "An indexed graph search referenced an invalid node",
            Self::InvalidGeometry => "Routing geometry and dimensions must be finite and valid",
            Self::NoOrthogonalPath => "No orthogonal route could be found",
        })
    }
}

impl std::error::Error for RoutingError {}

/// A result returned by pathfinding and graph construction algorithms.
pub type RoutingResult<T> = Result<T, RoutingError>;

#[cfg(test)]
mod tests;
