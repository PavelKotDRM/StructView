use crate::{
    Path, RoutingError, RoutingGraph, RoutingResult,
    search::{collect_graph_nodes, valid_weight},
};
use std::{collections::HashMap, hash::Hash};

/// Shortest paths from one source to every reachable node.
#[derive(Clone, Debug, PartialEq)]
pub struct SingleSourcePaths<N: Eq + Hash, E> {
    source: N,
    source_index: usize,
    nodes: Vec<N>,
    node_indices: HashMap<N, usize>,
    distances: Vec<Option<f64>>,
    predecessors: Vec<Option<(usize, E, f64)>>,
}

impl<N, E> SingleSourcePaths<N, E>
where
    N: Clone + Eq + std::hash::Hash,
    E: Clone,
{
    /// The source node used to calculate these paths.
    pub fn source(&self) -> &N {
        &self.source
    }

    /// All graph nodes in the graph's enumeration order.
    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    /// Return the shortest distance to a reachable node.
    ///
    /// Returns `None` when `target` is unknown or unreachable.
    pub fn distance_to(&self, target: &N) -> Option<f64> {
        let index = *self.node_indices.get(target)?;
        self.distances[index]
    }

    /// Reconstruct the shortest path to a reachable node.
    ///
    /// Returns `None` when `target` is unknown or unreachable.
    pub fn path_to(&self, target: &N) -> Option<Path<N, E>> {
        let target_index = *self.node_indices.get(target)?;
        let cost = self.distances[target_index]?;
        let mut current = target_index;
        let mut nodes = vec![self.nodes[current].clone()];
        let mut edges = Vec::new();
        let mut edge_costs = Vec::new();

        while current != self.source_index {
            if edges.len() == self.nodes.len() {
                return None;
            }
            let (previous, edge, edge_cost) = self.predecessors[current].as_ref()?.clone();
            current = previous;
            nodes.push(self.nodes[current].clone());
            edges.push(edge);
            edge_costs.push(edge_cost);
        }

        nodes.reverse();
        edges.reverse();
        edge_costs.reverse();
        Some(Path {
            nodes,
            edges,
            edge_costs,
            cost,
        })
    }

    /// Iterate over nodes reachable from the source, in graph enumeration order.
    pub fn reachable_nodes(&self) -> impl Iterator<Item = &N> {
        self.nodes
            .iter()
            .zip(&self.distances)
            .filter_map(|(node, distance)| distance.is_some().then_some(node))
    }
}

/// Shortest paths between every pair of nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct AllPairsPaths<N: Eq + Hash, E> {
    nodes: Vec<N>,
    node_indices: HashMap<N, usize>,
    distances: Vec<Vec<Option<f64>>>,
    first_edges: Vec<Vec<Option<(usize, E, f64)>>>,
}

impl<N, E> AllPairsPaths<N, E>
where
    N: Clone + Eq + std::hash::Hash,
    E: Clone,
{
    /// All graph nodes in the graph's enumeration order.
    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    /// Return the shortest distance between two reachable nodes.
    ///
    /// Returns `None` when either node is unknown or no path exists.
    pub fn distance(&self, source: &N, target: &N) -> Option<f64> {
        let source_index = *self.node_indices.get(source)?;
        let target_index = *self.node_indices.get(target)?;
        self.distances[source_index][target_index]
    }

    /// Reconstruct the shortest path between two reachable nodes.
    ///
    /// Returns `None` when either node is unknown or no path exists.
    pub fn path(&self, source: &N, target: &N) -> Option<Path<N, E>> {
        let source_index = *self.node_indices.get(source)?;
        let target_index = *self.node_indices.get(target)?;
        let cost = self.distances[source_index][target_index]?;
        if source_index == target_index {
            return Some(Path {
                nodes: vec![self.nodes[source_index].clone()],
                edges: Vec::new(),
                edge_costs: Vec::new(),
                cost,
            });
        }

        let mut current = source_index;
        let mut nodes = vec![self.nodes[current].clone()];
        let mut edges = Vec::new();
        let mut edge_costs = Vec::new();
        while current != target_index {
            if edges.len() == self.nodes.len() {
                return None;
            }
            let (next, edge, edge_cost) = self.first_edges[current][target_index].as_ref()?.clone();
            current = next;
            nodes.push(self.nodes[current].clone());
            edges.push(edge);
            edge_costs.push(edge_cost);
        }

        Some(Path {
            nodes,
            edges,
            edge_costs,
            cost,
        })
    }
}

/// Find shortest paths from `source`, allowing negative edges but not reachable
/// negative cycles.
pub fn bellman_ford<G: RoutingGraph>(
    graph: &G,
    source: G::Node,
) -> RoutingResult<SingleSourcePaths<G::Node, G::EdgeId>> {
    let graph_nodes = collect_graph_nodes(graph, false)?;
    let nodes = graph_nodes.nodes;
    let node_indices = graph_nodes.indices;
    let source_index = *node_indices.get(&source).ok_or(RoutingError::UnknownNode)?;
    let mut edges = Vec::new();
    for (from, node) in nodes.iter().enumerate() {
        for edge in graph.outgoing_edges(node) {
            let to = *node_indices
                .get(&edge.to)
                .ok_or(RoutingError::UnknownNode)?;
            edges.push((from, to, edge.id, valid_weight(edge.weight)?));
        }
    }

    let mut distances = vec![None; nodes.len()];
    let mut predecessors = vec![None; nodes.len()];
    distances[source_index] = Some(0.0);

    for _ in 1..nodes.len() {
        let mut changed = false;
        for (from, to, edge, weight) in &edges {
            let Some(source_cost) = distances[*from] else {
                continue;
            };
            let next_cost = source_cost + weight;
            if !next_cost.is_finite() {
                return Err(RoutingError::NonFiniteWeight);
            }
            if distances[*to].is_none_or(|known_cost| next_cost < known_cost) {
                distances[*to] = Some(next_cost);
                predecessors[*to] = Some((*from, edge.clone(), *weight));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (from, to, _, weight) in &edges {
        let Some(source_cost) = distances[*from] else {
            continue;
        };
        let next_cost = source_cost + weight;
        if !next_cost.is_finite() {
            return Err(RoutingError::NonFiniteWeight);
        }
        if distances[*to].is_none_or(|known_cost| next_cost < known_cost) {
            return Err(RoutingError::NegativeCycle);
        }
    }

    Ok(SingleSourcePaths {
        source,
        source_index,
        nodes,
        node_indices,
        distances,
        predecessors,
    })
}

/// Find shortest paths between all node pairs, allowing negative edges but not
/// negative cycles.
pub fn floyd_warshall<G: RoutingGraph>(
    graph: &G,
) -> RoutingResult<AllPairsPaths<G::Node, G::EdgeId>> {
    let graph_nodes = collect_graph_nodes(graph, false)?;
    let nodes = graph_nodes.nodes;
    let node_indices = graph_nodes.indices;
    let node_count = nodes.len();
    let mut distances = vec![vec![None; node_count]; node_count];
    let mut first_edges = vec![vec![None; node_count]; node_count];

    for (index, row) in distances.iter_mut().enumerate() {
        row[index] = Some(0.0);
    }
    for (from, node) in nodes.iter().enumerate() {
        for edge in graph.outgoing_edges(node) {
            let to = *node_indices
                .get(&edge.to)
                .ok_or(RoutingError::UnknownNode)?;
            let weight = valid_weight(edge.weight)?;
            if distances[from][to].is_none_or(|known_cost| weight < known_cost) {
                distances[from][to] = Some(weight);
                first_edges[from][to] = Some((to, edge.id, weight));
            }
        }
    }

    for intermediate in 0..node_count {
        for from in 0..node_count {
            let Some(to_intermediate) = distances[from][intermediate] else {
                continue;
            };
            let first_edge = first_edges[from][intermediate].clone();
            for to in 0..node_count {
                let Some(from_intermediate) = distances[intermediate][to] else {
                    continue;
                };
                let candidate = to_intermediate + from_intermediate;
                if !candidate.is_finite() {
                    return Err(RoutingError::NonFiniteWeight);
                }
                if distances[from][to].is_none_or(|known_cost| candidate < known_cost) {
                    distances[from][to] = Some(candidate);
                    first_edges[from][to] = first_edge.clone();
                }
            }
        }
        if (0..node_count).any(|index| distances[index][index].is_some_and(|cost| cost < 0.0)) {
            return Err(RoutingError::NegativeCycle);
        }
    }

    Ok(AllPairsPaths {
        nodes,
        node_indices,
        distances,
        first_edges,
    })
}
