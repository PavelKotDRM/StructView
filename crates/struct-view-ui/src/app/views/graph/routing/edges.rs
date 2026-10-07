use super::super::calculation::{
    GraphProgressTracker, GraphStage, advance_graph_progress, begin_graph_stage,
};
use super::*;
use std::thread;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) enum GraphRoutingWorkerSetting {
    #[default]
    Automatic,
    Manual(usize),
}

pub(in crate::app) fn available_graph_routing_workers() -> usize {
    match thread::available_parallelism() {
        Ok(count) => count.get(),
        Err(error) => {
            eprintln!("Cannot determine graph routing parallelism: {error}; using one worker");
            1
        }
    }
}

pub(in crate::app) fn graph_routing_worker_count(
    edge_count: usize,
    setting: GraphRoutingWorkerSetting,
) -> usize {
    if edge_count == 0 {
        return 1;
    }
    let available = available_graph_routing_workers();
    match setting {
        GraphRoutingWorkerSetting::Automatic => available,
        GraphRoutingWorkerSetting::Manual(count) => count.clamp(1, available),
    }
    .min(edge_count)
}

#[cfg(test)]
pub(super) fn route_graph_edges(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
) -> Vec<Vec<Pos2>> {
    route_graph_edges_with_progress(grid, endpoints, ports, worker_count, None)
}

pub(in crate::app::views::graph) fn route_graph_edges_with_progress(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    if endpoints.is_empty() {
        return Vec::new();
    }
    let workers = worker_count.max(1).min(endpoints.len());
    if workers == 1 {
        begin_graph_stage(progress, GraphStage::Sequential, endpoints.len(), 1);
        let mut routes = Vec::with_capacity(endpoints.len());
        let mut route_index = GraphRouteSegmentIndex::new(&[]);
        for (&(source, target), &ports) in endpoints.iter().zip(ports) {
            let route = grid.route_edge_with_index(source, target, ports, &route_index);
            route_index.insert_route(&route);
            routes.push(route);
            advance_graph_progress(progress);
        }
        return routes;
    }

    let chunk_size = endpoints.len().div_ceil(workers);
    begin_graph_stage(progress, GraphStage::Preliminary, endpoints.len(), workers);
    let mut candidates = vec![Vec::new(); endpoints.len()];
    thread::scope(|scope| {
        for (chunk_index, chunk) in candidates.chunks_mut(chunk_size).enumerate() {
            let start = chunk_index * chunk_size;
            scope.spawn(move || {
                for (offset, candidate) in chunk.iter_mut().enumerate() {
                    let index = start + offset;
                    let (source, target) = endpoints[index];
                    *candidate = grid.route_edge_with_ports(source, target, ports[index], &[]);
                    advance_graph_progress(progress);
                }
            });
        }
    });

    resolve_graph_route_conflicts(grid, endpoints, ports, candidates, progress)
}

fn resolve_graph_route_conflicts(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    candidates: Vec<Vec<Pos2>>,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    let mut routes = Vec::with_capacity(endpoints.len());
    let mut route_index = GraphRouteSegmentIndex::new(&[]);
    begin_graph_stage(progress, GraphStage::Conflicts, endpoints.len(), 1);
    for (index, candidate) in candidates.iter().enumerate() {
        let route = if route_index.conflicts_route(candidate) {
            let (source, target) = endpoints[index];
            grid.route_edge_with_index(source, target, ports[index], &route_index)
        } else {
            candidate.clone()
        };
        route_index.insert_route(&route);
        routes.push(route);
        advance_graph_progress(progress);
    }
    routes
}

#[cfg(test)]
pub(super) fn graph_route_conflicts(candidate: &[Pos2], routes: &[Vec<Pos2>]) -> bool {
    candidate.windows(2).any(|segment| {
        routes
            .iter()
            .flat_map(|route| route.windows(2))
            .any(|other| {
                segments_within_clearance(
                    segment[0],
                    segment[1],
                    other[0],
                    other[1],
                    GRAPH_EDGE_CLEARANCE,
                )
            })
    })
}
