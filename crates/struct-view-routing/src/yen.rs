use crate::{
    Path, RoutingError, RoutingGraph, RoutingResult, WeightedEdge, dijkstra, search::validate_graph,
};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashSet},
};

struct Candidate<N, E> {
    path: Path<N, E>,
    sequence: u64,
}

impl<N, E> PartialEq for Candidate<N, E> {
    fn eq(&self, other: &Self) -> bool {
        self.path.cost.total_cmp(&other.path.cost) == Ordering::Equal
            && self.sequence == other.sequence
    }
}

impl<N, E> Eq for Candidate<N, E> {}

impl<N, E> PartialOrd for Candidate<N, E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N, E> Ord for Candidate<N, E> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .path
            .cost
            .total_cmp(&self.path.cost)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

struct FilteredGraph<'a, G: RoutingGraph> {
    graph: &'a G,
    banned_nodes: &'a HashSet<G::Node>,
    banned_edges: &'a HashSet<G::EdgeId>,
}

impl<G: RoutingGraph> RoutingGraph for FilteredGraph<'_, G> {
    type Node = G::Node;
    type EdgeId = G::EdgeId;

    fn nodes(&self) -> Vec<Self::Node> {
        self.graph
            .nodes()
            .into_iter()
            .filter(|node| !self.banned_nodes.contains(node))
            .collect()
    }

    fn outgoing_edges(&self, node: &Self::Node) -> Vec<WeightedEdge<Self::Node, Self::EdgeId>> {
        if self.banned_nodes.contains(node) {
            return Vec::new();
        }
        self.graph
            .outgoing_edges(node)
            .into_iter()
            .filter(|edge| {
                !self.banned_nodes.contains(&edge.to) && !self.banned_edges.contains(&edge.id)
            })
            .collect()
    }
}

/// Return up to `k` shortest loopless paths in nondecreasing cost order.
///
/// Yen's algorithm uses Dijkstra searches, so all graph weights must be finite
/// and non-negative. Distinct directed arcs must have distinct edge IDs.
pub fn yen_k_shortest_paths<G: RoutingGraph>(
    graph: &G,
    source: G::Node,
    target: G::Node,
    k: usize,
) -> RoutingResult<Vec<Path<G::Node, G::EdgeId>>> {
    let nodes = validate_graph(graph, true)?;
    if !nodes.contains(&source) || !nodes.contains(&target) {
        return Err(RoutingError::UnknownNode);
    }
    if k == 0 {
        return Ok(Vec::new());
    }

    let Some(first_path) = dijkstra(graph, source.clone(), target.clone())? else {
        return Ok(Vec::new());
    };
    if first_path.edges.is_empty() {
        return Ok(vec![first_path]);
    }

    let mut paths = vec![first_path];
    let mut candidates = BinaryHeap::new();
    let mut seen = paths
        .iter()
        .map(|path| (path.nodes.clone(), path.edges.clone()))
        .collect::<HashSet<_>>();
    let mut sequence = 0_u64;

    while paths.len() < k {
        let previous_path = paths.last().expect("the first shortest path is present");
        let mut root_cost = 0.0;

        for spur_index in 0..previous_path.edges.len() {
            let spur_node = &previous_path.nodes[spur_index];
            let mut banned_edges = HashSet::new();
            for path in &paths {
                if path.edges.len() > spur_index
                    && path.nodes[..=spur_index] == previous_path.nodes[..=spur_index]
                    && path.edges[..spur_index] == previous_path.edges[..spur_index]
                {
                    banned_edges.insert(path.edges[spur_index].clone());
                }
            }
            let banned_nodes = previous_path.nodes[..spur_index]
                .iter()
                .cloned()
                .collect::<HashSet<_>>();
            let filtered_graph = FilteredGraph {
                graph,
                banned_nodes: &banned_nodes,
                banned_edges: &banned_edges,
            };

            if let Some(spur_path) = dijkstra(&filtered_graph, spur_node.clone(), target.clone())? {
                let cost = root_cost + spur_path.cost;
                if !cost.is_finite() {
                    return Err(RoutingError::NonFiniteWeight);
                }

                let mut nodes = previous_path.nodes[..=spur_index].to_vec();
                nodes.extend(spur_path.nodes.into_iter().skip(1));
                let mut edges = previous_path.edges[..spur_index].to_vec();
                edges.extend(spur_path.edges);
                let mut edge_costs = previous_path.edge_costs[..spur_index].to_vec();
                edge_costs.extend(spur_path.edge_costs);
                if seen.insert((nodes.clone(), edges.clone())) {
                    candidates.push(Candidate {
                        path: Path {
                            nodes,
                            edges,
                            edge_costs,
                            cost,
                        },
                        sequence,
                    });
                    sequence = sequence.wrapping_add(1);
                }
            }

            root_cost += previous_path.edge_costs[spur_index];
        }

        let Some(candidate) = candidates.pop() else {
            break;
        };
        paths.push(candidate.path);
    }

    Ok(paths)
}
