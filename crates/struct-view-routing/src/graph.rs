use crate::{RoutingGraph, WeightedEdge};
use std::collections::{HashMap, HashSet};

/// Errors raised while adding nodes and directed arcs to an adjacency-list graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphBuildError {
    /// The directed arc's source was not added to the graph.
    UnknownSource,
    /// The directed arc's destination was not added to the graph.
    UnknownTarget,
    /// An edge identifier was already used by another directed arc.
    DuplicateEdgeId,
}

impl std::fmt::Display for GraphBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownSource => "The graph edge source is not a known node",
            Self::UnknownTarget => "The graph edge target is not a known node",
            Self::DuplicateEdgeId => "The graph edge identifier is already in use",
        })
    }
}

impl std::error::Error for GraphBuildError {}

/// A reusable directed graph stored as an adjacency list.
///
/// Nodes are kept in insertion order. Edge identifiers must be unique across
/// all directed arcs. Weights are stored as supplied and validated by the
/// selected pathfinding algorithm.
#[derive(Clone, Debug)]
pub struct AdjacencyListGraph<N, E> {
    nodes: Vec<N>,
    node_indices: HashMap<N, usize>,
    outgoing: Vec<Vec<WeightedEdge<N, E>>>,
    edge_ids: HashSet<E>,
}

impl<N, E> Default for AdjacencyListGraph<N, E> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            node_indices: HashMap::new(),
            outgoing: Vec::new(),
            edge_ids: HashSet::new(),
        }
    }
}

impl<N, E> AdjacencyListGraph<N, E> {
    /// Create an empty directed graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of directed arcs in the graph.
    pub fn edge_count(&self) -> usize {
        self.outgoing.iter().map(Vec::len).sum()
    }

    /// Node identifiers in insertion order.
    pub fn node_ids(&self) -> &[N] {
        &self.nodes
    }
}

impl<N, E> AdjacencyListGraph<N, E>
where
    N: Clone + Eq + std::hash::Hash,
{
    /// Add a node, returning its stable zero-based index.
    ///
    /// Adding an existing node is idempotent and returns its existing index.
    pub fn add_node(&mut self, node: N) -> usize {
        if let Some(&index) = self.node_indices.get(&node) {
            return index;
        }

        let index = self.nodes.len();
        self.node_indices.insert(node.clone(), index);
        self.nodes.push(node);
        self.outgoing.push(Vec::new());
        index
    }

    /// Look up a node's stable zero-based index.
    pub fn node_index(&self, node: &N) -> Option<usize> {
        self.node_indices.get(node).copied()
    }

    /// Return the outgoing arcs from a known node.
    pub fn edges_from(&self, node: &N) -> Option<&[WeightedEdge<N, E>]> {
        self.node_indices
            .get(node)
            .map(|&index| self.outgoing[index].as_slice())
    }
}

impl<N, E> AdjacencyListGraph<N, E>
where
    N: Clone + Eq + std::hash::Hash,
    E: Clone + Eq + std::hash::Hash,
{
    /// Add a directed arc between existing nodes.
    pub fn add_edge(
        &mut self,
        source: &N,
        target: &N,
        id: E,
        weight: f64,
    ) -> Result<(), GraphBuildError> {
        let source_index = self
            .node_indices
            .get(source)
            .copied()
            .ok_or(GraphBuildError::UnknownSource)?;
        let target_index = self
            .node_indices
            .get(target)
            .copied()
            .ok_or(GraphBuildError::UnknownTarget)?;
        if self.edge_ids.contains(&id) {
            return Err(GraphBuildError::DuplicateEdgeId);
        }

        self.edge_ids.insert(id.clone());
        self.outgoing[source_index].push(WeightedEdge::new(
            self.nodes[target_index].clone(),
            id,
            weight,
        ));
        Ok(())
    }

    /// Add an undirected edge as two directed arcs with distinct identifiers.
    pub fn add_undirected_edge(
        &mut self,
        first: &N,
        second: &N,
        first_to_second_id: E,
        second_to_first_id: E,
        weight: f64,
    ) -> Result<(), GraphBuildError> {
        let first_index = self
            .node_indices
            .get(first)
            .copied()
            .ok_or(GraphBuildError::UnknownSource)?;
        let second_index = self
            .node_indices
            .get(second)
            .copied()
            .ok_or(GraphBuildError::UnknownTarget)?;
        if first_to_second_id == second_to_first_id
            || self.edge_ids.contains(&first_to_second_id)
            || self.edge_ids.contains(&second_to_first_id)
        {
            return Err(GraphBuildError::DuplicateEdgeId);
        }

        self.edge_ids.insert(first_to_second_id.clone());
        self.outgoing[first_index].push(WeightedEdge::new(
            self.nodes[second_index].clone(),
            first_to_second_id,
            weight,
        ));
        self.edge_ids.insert(second_to_first_id.clone());
        self.outgoing[second_index].push(WeightedEdge::new(
            self.nodes[first_index].clone(),
            second_to_first_id,
            weight,
        ));
        Ok(())
    }
}

impl<N, E> RoutingGraph for AdjacencyListGraph<N, E>
where
    N: Clone + Eq + std::hash::Hash,
    E: Clone + Eq + std::hash::Hash,
{
    type Node = N;
    type EdgeId = E;

    fn nodes(&self) -> Vec<Self::Node> {
        self.nodes.clone()
    }

    fn outgoing_edges(&self, node: &Self::Node) -> Vec<WeightedEdge<Self::Node, Self::EdgeId>> {
        self.edges_from(node).unwrap_or_default().to_vec()
    }
}
