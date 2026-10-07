use crate::{Path, RoutingError, RoutingGraph, RoutingResult};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    hash::Hash,
};

/// Return a minimum-hop path, ignoring edge weights.
pub fn breadth_first_search<G: RoutingGraph>(
    graph: &G,
    source: G::Node,
    target: G::Node,
) -> RoutingResult<Option<Path<G::Node, G::EdgeId>>> {
    let nodes = validate_graph(graph, false)?;
    if !nodes.contains(&source) || !nodes.contains(&target) {
        return Err(RoutingError::UnknownNode);
    }
    if source == target {
        return Ok(Some(Path {
            nodes: vec![source],
            edges: Vec::new(),
            edge_costs: Vec::new(),
            cost: 0.0,
        }));
    }

    let mut queue = VecDeque::from([source.clone()]);
    let mut visited = HashSet::from([source.clone()]);
    let mut hops = HashMap::from([(source.clone(), 0_usize)]);
    let mut previous = HashMap::new();
    while let Some(node) = queue.pop_front() {
        for edge in graph.outgoing_edges(&node) {
            if visited.insert(edge.to.clone()) {
                let next_hops = hops[&node] + 1;
                let next = edge.to;
                previous.insert(next.clone(), (node.clone(), edge.id, 1.0));
                hops.insert(next.clone(), next_hops);
                if next == target {
                    return Ok(reconstruct_path(
                        &source,
                        &target,
                        next_hops as f64,
                        &previous,
                    ));
                }
                queue.push_back(next);
            }
        }
    }
    Ok(None)
}

/// Return a minimum-cost path for a graph with non-negative edge weights.
pub fn dijkstra<G: RoutingGraph>(
    graph: &G,
    source: G::Node,
    target: G::Node,
) -> RoutingResult<Option<Path<G::Node, G::EdgeId>>> {
    validate_graph(graph, true)?;
    best_first_search(graph, source, target, |_, _| 0.0)
}

/// Return a minimum-cost path using A* and a caller-supplied heuristic.
///
/// The heuristic should be admissible for optimality. Every heuristic value
/// must be finite and non-negative.
pub fn a_star<G, H>(
    graph: &G,
    source: G::Node,
    target: G::Node,
    heuristic: H,
) -> RoutingResult<Option<Path<G::Node, G::EdgeId>>>
where
    G: RoutingGraph,
    H: FnMut(&G::Node, &G::Node) -> f64,
{
    validate_graph(graph, true)?;
    best_first_search(graph, source, target, heuristic)
}

/// An outgoing edge in a dense, zero-based search graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexedNeighbor {
    /// Destination state index.
    pub node: usize,
    /// Non-negative traversal cost.
    pub cost: f64,
}

/// Result returned by [`a_star_indexed`].
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedPath {
    /// Ordered state indexes, including both endpoints.
    pub nodes: Vec<usize>,
    /// Total path cost.
    pub cost: f64,
}

/// A* for implicit dense graphs whose states are `0..node_count`.
///
/// `neighbors` fills the supplied scratch vector for the current state.
/// `is_goal` may match multiple states, which is useful when the goal has
/// directional variants.
pub fn a_star_indexed<G, N, H>(
    node_count: usize,
    start: usize,
    mut is_goal: G,
    mut neighbors: N,
    mut heuristic: H,
) -> RoutingResult<Option<IndexedPath>>
where
    G: FnMut(usize) -> bool,
    N: FnMut(usize, &mut Vec<IndexedNeighbor>),
    H: FnMut(usize) -> f64,
{
    if start >= node_count {
        return Err(RoutingError::InvalidNodeIndex);
    }
    let start_heuristic = valid_heuristic(heuristic(start))?;
    let mut distances = vec![f64::INFINITY; node_count];
    let mut previous = vec![usize::MAX; node_count];
    let mut queue = BinaryHeap::new();
    let mut sequence = 0;
    distances[start] = 0.0;
    queue.push(IndexedQueueEntry {
        priority: start_heuristic,
        cost: 0.0,
        sequence,
        node: start,
    });
    let mut outgoing = Vec::with_capacity(8);
    let mut goal = None;

    while let Some(entry) = queue.pop() {
        if entry.cost > distances[entry.node] {
            continue;
        }
        if is_goal(entry.node) {
            goal = Some(entry.node);
            break;
        }

        outgoing.clear();
        neighbors(entry.node, &mut outgoing);
        for edge in outgoing.iter().copied() {
            if edge.node >= node_count {
                return Err(RoutingError::InvalidNodeIndex);
            }
            let weight = valid_nonnegative_weight(edge.cost)?;
            let next_cost = entry.cost + weight;
            if !next_cost.is_finite() {
                return Err(RoutingError::NonFiniteWeight);
            }
            if next_cost >= distances[edge.node] {
                continue;
            }
            let estimated_total = next_cost + valid_heuristic(heuristic(edge.node))?;
            if !estimated_total.is_finite() {
                return Err(RoutingError::InvalidHeuristic);
            }
            distances[edge.node] = next_cost;
            previous[edge.node] = entry.node;
            sequence = sequence.wrapping_add(1);
            queue.push(IndexedQueueEntry {
                priority: estimated_total,
                cost: next_cost,
                sequence,
                node: edge.node,
            });
        }
    }

    let Some(mut node) = goal else {
        return Ok(None);
    };
    let cost = distances[node];
    let mut path = vec![node];
    while node != start {
        node = previous[node];
        if node == usize::MAX {
            return Ok(None);
        }
        path.push(node);
    }
    path.reverse();
    Ok(Some(IndexedPath { nodes: path, cost }))
}

pub(crate) fn validate_graph<G: RoutingGraph>(
    graph: &G,
    require_nonnegative: bool,
) -> RoutingResult<HashSet<G::Node>> {
    let nodes = graph.nodes().into_iter().collect::<HashSet<_>>();
    validate_graph_edges(graph, &nodes, require_nonnegative)?;
    Ok(nodes)
}

pub(crate) struct GraphNodeIndices<N> {
    pub(crate) nodes: Vec<N>,
    pub(crate) indices: HashMap<N, usize>,
}

pub(crate) fn collect_graph_nodes<G: RoutingGraph>(
    graph: &G,
    require_nonnegative: bool,
) -> RoutingResult<GraphNodeIndices<G::Node>> {
    let mut nodes = Vec::new();
    let mut indices = HashMap::new();
    for node in graph.nodes() {
        if let std::collections::hash_map::Entry::Vacant(entry) = indices.entry(node.clone()) {
            entry.insert(nodes.len());
            nodes.push(node);
        }
    }
    let known_nodes = nodes.iter().cloned().collect::<HashSet<_>>();
    validate_graph_edges(graph, &known_nodes, require_nonnegative)?;
    Ok(GraphNodeIndices { nodes, indices })
}

fn validate_graph_edges<G: RoutingGraph>(
    graph: &G,
    nodes: &HashSet<G::Node>,
    require_nonnegative: bool,
) -> RoutingResult<()> {
    for node in nodes {
        for edge in graph.outgoing_edges(node) {
            if !nodes.contains(&edge.to) {
                return Err(RoutingError::UnknownNode);
            }
            valid_weight(edge.weight)?;
            if require_nonnegative && edge.weight < 0.0 {
                return Err(RoutingError::NegativeWeight);
            }
        }
    }
    Ok(())
}

pub(crate) fn valid_weight(weight: f64) -> RoutingResult<f64> {
    if weight.is_finite() {
        Ok(weight)
    } else {
        Err(RoutingError::NonFiniteWeight)
    }
}

pub(crate) fn valid_nonnegative_weight(weight: f64) -> RoutingResult<f64> {
    let weight = valid_weight(weight)?;
    if weight < 0.0 {
        Err(RoutingError::NegativeWeight)
    } else {
        Ok(weight)
    }
}

pub(crate) fn valid_heuristic(heuristic: f64) -> RoutingResult<f64> {
    if heuristic.is_finite() && heuristic >= 0.0 {
        Ok(heuristic)
    } else {
        Err(RoutingError::InvalidHeuristic)
    }
}

pub(crate) fn best_first_search<G, H>(
    graph: &G,
    source: G::Node,
    target: G::Node,
    mut heuristic: H,
) -> RoutingResult<Option<Path<G::Node, G::EdgeId>>>
where
    G: RoutingGraph,
    H: FnMut(&G::Node, &G::Node) -> f64,
{
    let nodes = validate_graph(graph, true)?;
    if !nodes.contains(&source) || !nodes.contains(&target) {
        return Err(RoutingError::UnknownNode);
    }
    let start_heuristic = valid_heuristic(heuristic(&source, &target))?;
    let mut distances = HashMap::from([(source.clone(), 0.0)]);
    let mut previous = HashMap::new();
    let mut queue = BinaryHeap::new();
    let mut sequence = 0;
    queue.push(QueueEntry {
        priority: start_heuristic,
        cost: 0.0,
        sequence,
        node: source.clone(),
    });

    while let Some(entry) = queue.pop() {
        if entry.cost > distances.get(&entry.node).copied().unwrap_or(f64::INFINITY) {
            continue;
        }
        if entry.node == target {
            return Ok(reconstruct_path(&source, &target, entry.cost, &previous));
        }
        for edge in graph.outgoing_edges(&entry.node) {
            let weight = valid_nonnegative_weight(edge.weight)?;
            let next_cost = entry.cost + weight;
            if !next_cost.is_finite() {
                return Err(RoutingError::NonFiniteWeight);
            }
            if next_cost >= distances.get(&edge.to).copied().unwrap_or(f64::INFINITY) {
                continue;
            }
            let estimated_total = next_cost + valid_heuristic(heuristic(&edge.to, &target))?;
            if !estimated_total.is_finite() {
                return Err(RoutingError::InvalidHeuristic);
            }
            distances.insert(edge.to.clone(), next_cost);
            previous.insert(edge.to.clone(), (entry.node.clone(), edge.id, weight));
            sequence = sequence.wrapping_add(1);
            queue.push(QueueEntry {
                priority: estimated_total,
                cost: next_cost,
                sequence,
                node: edge.to,
            });
        }
    }
    Ok(None)
}

pub(crate) fn reconstruct_path<N, E>(
    source: &N,
    target: &N,
    cost: f64,
    previous: &HashMap<N, (N, E, f64)>,
) -> Option<Path<N, E>>
where
    N: Clone + Eq + Hash,
    E: Clone,
{
    let mut nodes = vec![target.clone()];
    let mut edges = Vec::new();
    let mut edge_costs = Vec::new();
    let mut current = target.clone();
    let mut remaining = previous.len() + 1;
    while &current != source {
        if remaining == 0 {
            return None;
        }
        remaining -= 1;
        let (parent, edge, edge_cost) = previous.get(&current)?.clone();
        edges.push(edge);
        edge_costs.push(edge_cost);
        nodes.push(parent.clone());
        current = parent;
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

struct QueueEntry<N> {
    priority: f64,
    cost: f64,
    sequence: u64,
    node: N,
}

struct IndexedQueueEntry {
    priority: f64,
    cost: f64,
    sequence: u64,
    node: usize,
}

impl PartialEq for IndexedQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority.total_cmp(&other.priority) == Ordering::Equal
            && self.cost.total_cmp(&other.cost) == Ordering::Equal
            && self.node == other.node
            && self.sequence == other.sequence
    }
}

impl Eq for IndexedQueueEntry {}

impl PartialOrd for IndexedQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexedQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.cost.total_cmp(&self.cost))
            .then_with(|| other.node.cmp(&self.node))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl<N> PartialEq for QueueEntry<N> {
    fn eq(&self, other: &Self) -> bool {
        self.priority.total_cmp(&other.priority) == Ordering::Equal
            && self.cost.total_cmp(&other.cost) == Ordering::Equal
            && self.sequence == other.sequence
    }
}

impl<N> Eq for QueueEntry<N> {}

impl<N> PartialOrd for QueueEntry<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N> Ord for QueueEntry<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.cost.total_cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}
