use super::super::calculation::{
    GraphProgressTracker, GraphStage, advance_graph_progress, begin_graph_stage,
};
use super::*;
use std::thread;

#[derive(Debug, Clone)]
pub(in crate::app::views::graph) enum GraphRoutingError {
    Edge {
        source: usize,
        target: usize,
        cause: struct_view_routing::RoutingError,
    },
    /// Edges that found no overlap-free path in the current node placement.
    Crowded {
        edges: Vec<(usize, usize)>,
    },
    InvalidPortCount,
    MissingRoute,
}

impl GraphRoutingError {
    fn from_edge(source: usize, target: usize, cause: struct_view_routing::RoutingError) -> Self {
        Self::Edge {
            source,
            target,
            cause,
        }
    }

    pub(in crate::app::views::graph) fn needs_more_space(&self) -> bool {
        matches!(self, Self::Crowded { .. })
    }

    pub(in crate::app::views::graph) fn crowded_edges(&self) -> &[(usize, usize)] {
        match self {
            Self::Crowded { edges } => edges,
            _ => &[],
        }
    }
}

/// Collects edges without a free path so the layout can widen every crowded place at once.
#[derive(Default)]
struct CrowdedEdges(Vec<(usize, usize)>);

impl CrowdedEdges {
    fn route(
        &mut self,
        source: usize,
        target: usize,
        result: struct_view_routing::RoutingResult<Vec<Pos2>>,
    ) -> Result<Option<Vec<Pos2>>, GraphRoutingError> {
        match result {
            Ok(route) => Ok(Some(route)),
            Err(struct_view_routing::RoutingError::NoOrthogonalPath) => {
                self.0.push((source, target));
                Ok(None)
            }
            Err(cause) => Err(GraphRoutingError::from_edge(source, target, cause)),
        }
    }

    fn finish(self, routes: Vec<Vec<Pos2>>) -> Result<Vec<Vec<Pos2>>, GraphRoutingError> {
        if self.0.is_empty() {
            Ok(routes)
        } else {
            Err(GraphRoutingError::Crowded { edges: self.0 })
        }
    }
}

impl std::fmt::Display for GraphRoutingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Edge {
                source,
                target,
                cause,
            } => {
                write!(
                    formatter,
                    "Cannot route graph edge {source} -> {target}: {cause}"
                )
            }
            Self::Crowded { edges } => {
                let (source, target) = edges.first().copied().unwrap_or_default();
                write!(
                    formatter,
                    "Cannot route graph edge {source} -> {target}: {}",
                    struct_view_routing::RoutingError::NoOrthogonalPath
                )?;
                if edges.len() > 1 {
                    write!(formatter, " (and {} more crowded edge(s))", edges.len() - 1)?;
                }
                Ok(())
            }
            Self::InvalidPortCount => {
                formatter.write_str("Graph edge and port counts do not match")
            }
            Self::MissingRoute => {
                formatter.write_str("Graph routing worker did not produce a route")
            }
        }
    }
}

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
        .expect("test graph must have valid routes")
}

pub(in crate::app::views::graph) fn route_graph_edges_with_progress(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
    progress: Option<&GraphProgressTracker>,
) -> Result<Vec<Vec<Pos2>>, GraphRoutingError> {
    if endpoints.len() != ports.len() {
        return Err(GraphRoutingError::InvalidPortCount);
    }
    if endpoints.is_empty() {
        return Ok(Vec::new());
    }
    let workers = worker_count.max(1).min(endpoints.len());
    let mut candidates = vec![None; endpoints.len()];
    if workers == 1 {
        begin_graph_stage(progress, GraphStage::Sequential, endpoints.len(), 1);
        let empty_index = GraphRouteSegmentIndex::new(&[]);
        for (index, candidate) in candidates.iter_mut().enumerate() {
            let (source, target) = endpoints[index];
            *candidate =
                Some(grid.route_edge_with_index(source, target, ports[index], &empty_index));
        }
    } else {
        let chunk_size = endpoints.len().div_ceil(workers);
        begin_graph_stage(progress, GraphStage::Preliminary, endpoints.len(), workers);
        thread::scope(|scope| {
            for (chunk_index, chunk) in candidates.chunks_mut(chunk_size).enumerate() {
                let start = chunk_index * chunk_size;
                scope.spawn(move || {
                    let empty_index = GraphRouteSegmentIndex::new(&[]);
                    for (offset, candidate) in chunk.iter_mut().enumerate() {
                        let index = start + offset;
                        let (source, target) = endpoints[index];
                        *candidate = Some(grid.route_edge_with_index(
                            source,
                            target,
                            ports[index],
                            &empty_index,
                        ));
                        advance_graph_progress(progress);
                    }
                });
            }
        });
    }
    let mut crowded = CrowdedEdges::default();
    let candidates = candidates
        .into_iter()
        .zip(endpoints)
        .map(|(candidate, &(source, target))| {
            crowded.route(
                source,
                target,
                candidate.ok_or(GraphRoutingError::MissingRoute)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !crowded.0.is_empty() {
        return crowded.finish(Vec::new());
    }
    let candidates = candidates.into_iter().flatten().collect();

    if workers > 1 {
        begin_graph_stage(progress, GraphStage::Conflicts, endpoints.len(), 1);
    }
    let mut routes = resolve_graph_route_conflicts(grid, endpoints, ports, candidates, progress)?;
    upgrade_straight_routes(grid, endpoints, &mut routes);
    Ok(routes)
}

/// Longest straight link that may replace an orthogonal route: two layout columns.
const GRAPH_DIAGONAL_MAX_LENGTH: f32 = 2.0 * GRAPH_STEP.x;
/// Most crossings a straight link may have with the other links of the final layout.
const GRAPH_DIAGONAL_MAX_CROSSINGS: usize = 1;

/// A drawn segment and the index of the link it belongs to.
struct PlacedSegment {
    owner: usize,
    start: Pos2,
    end: Pos2,
}

/// Replaces orthogonal routes with the straight segment between their ports when that segment is
/// short, clears every card, keeps the edge clearance from other links and crosses few enough
/// links. It runs after the orthogonal layout is final, so every check sees the routes that are
/// drawn, including straight links accepted earlier in this pass.
pub(super) fn upgrade_straight_routes(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    routes: &mut [Vec<Pos2>],
) {
    let mut placed = Vec::new();
    for (owner, route) in routes.iter().enumerate() {
        placed.extend(route.windows(2).map(|pair| PlacedSegment {
            owner,
            start: pair[0],
            end: pair[1],
        }));
    }
    let mut straight_crossings: Vec<Option<usize>> = vec![None; routes.len()];
    for index in 0..routes.len() {
        let (source, target) = endpoints[index];
        let route = &routes[index];
        if source == target || route.len() <= 2 {
            continue;
        }
        let (start, end) = (route[0], route[route.len() - 1]);
        if start.distance(end) > GRAPH_DIAGONAL_MAX_LENGTH
            || !grid.card_clear_of_segment(source, target, start, end)
        {
            continue;
        }
        let Some((crossings, crossed_straight)) =
            straight_link_crossings(start, end, index, &placed, &straight_crossings)
        else {
            continue;
        };
        for owner in crossed_straight {
            if let Some(count) = &mut straight_crossings[owner] {
                *count += 1;
            }
        }
        placed.retain(|segment| segment.owner != index);
        placed.push(PlacedSegment {
            owner: index,
            start,
            end,
        });
        straight_crossings[index] = Some(crossings);
        routes[index] = vec![start, end];
    }
}

/// Crossings a straight link from `start` to `end` makes with the other links, and the straight
/// links among them. `None` when it runs within the edge clearance of another link, or when it or
/// a straight link it crosses would exceed the crossing limit.
fn straight_link_crossings(
    start: Pos2,
    end: Pos2,
    owner: usize,
    placed: &[PlacedSegment],
    straight_crossings: &[Option<usize>],
) -> Option<(usize, Vec<usize>)> {
    let margin = Vec2::splat(GRAPH_EDGE_CLEARANCE);
    let (low, high) = (start.min(end) - margin, start.max(end) + margin);
    let mut crossings = 0;
    let mut crossed_straight = Vec::new();
    for segment in placed.iter().filter(|segment| segment.owner != owner) {
        let segment_low = segment.start.min(segment.end);
        let segment_high = segment.start.max(segment.end);
        if segment_high.x < low.x
            || segment_low.x > high.x
            || segment_high.y < low.y
            || segment_low.y > high.y
        {
            continue;
        }
        if segment_endpoints_too_close(start, end, segment.start, segment.end) {
            return None;
        }
        if segments_intersect(start, end, segment.start, segment.end) {
            crossings += 1;
            if crossings > GRAPH_DIAGONAL_MAX_CROSSINGS {
                return None;
            }
            if let Some(count) = straight_crossings[segment.owner] {
                if count >= GRAPH_DIAGONAL_MAX_CROSSINGS {
                    return None;
                }
                crossed_straight.push(segment.owner);
            }
        }
    }
    Some((crossings, crossed_straight))
}
fn resolve_graph_route_conflicts(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    candidates: Vec<Vec<Pos2>>,
    progress: Option<&GraphProgressTracker>,
) -> Result<Vec<Vec<Pos2>>, GraphRoutingError> {
    let mut routes = Vec::with_capacity(endpoints.len());
    let mut route_index = GraphRouteSegmentIndex::new(&[]);
    let mut crowded = CrowdedEdges::default();
    for (index, candidate) in candidates.iter().enumerate() {
        let route = if route_index.parallel_conflicts_route(candidate) {
            let (source, target) = endpoints[index];
            crowded
                .route(
                    source,
                    target,
                    grid.route_edge_with_index(source, target, ports[index], &route_index),
                )?
                .unwrap_or_default()
        } else {
            candidate.clone()
        };
        route_index.insert_route(&route);
        routes.push(route);
        advance_graph_progress(progress);
    }
    crowded.finish(routes)
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
