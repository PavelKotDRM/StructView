use super::*;

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};

const GRAPH_PARALLEL_EDGE_THRESHOLD: usize = 64;
const GRAPH_MAX_ROUTING_WORKERS: usize = 8;

pub(super) fn graph_routing_worker_count(edge_count: usize) -> usize {
    if edge_count < GRAPH_PARALLEL_EDGE_THRESHOLD {
        return 1;
    }
    match thread::available_parallelism() {
        Ok(count) => count
            .get()
            .saturating_sub(1)
            .clamp(1, GRAPH_MAX_ROUTING_WORKERS),
        Err(error) => {
            eprintln!("Cannot determine graph routing parallelism: {error}; using one worker");
            1
        }
    }
}

#[cfg(test)]
fn route_graph_edges(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
) -> Vec<Vec<Pos2>> {
    route_graph_edges_with_progress(grid, endpoints, ports, worker_count, None)
}

pub(super) fn route_graph_edges_with_progress(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    if endpoints.is_empty() {
        return Vec::new();
    }
    let workers = worker_count
        .clamp(1, GRAPH_MAX_ROUTING_WORKERS)
        .min(endpoints.len());
    if workers == 1 {
        begin_graph_stage(progress, GraphStage::Sequential, endpoints.len(), 1);
        let mut routes = Vec::with_capacity(endpoints.len());
        for (&(source, target), &ports) in endpoints.iter().zip(ports) {
            routes.push(grid.route_edge_with_ports(source, target, ports, &routes));
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

    resolve_graph_route_conflicts(grid, endpoints, ports, candidates, workers, progress)
}

fn resolve_graph_route_conflicts(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    candidates: Vec<Vec<Pos2>>,
    workers: usize,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    let mut routes = Vec::with_capacity(endpoints.len());
    begin_graph_stage(progress, GraphStage::Conflicts, endpoints.len(), workers);
    // Fixed wave boundaries keep the result independent of worker count and completion order.
    for batch_start in (0..endpoints.len()).step_by(GRAPH_MAX_ROUTING_WORKERS) {
        let batch_end = (batch_start + GRAPH_MAX_ROUTING_WORKERS).min(endpoints.len());
        while routes.len() < batch_end {
            let start = routes.len();
            let mut wave_end = start + 1;
            while wave_end < batch_end
                && !graph_route_conflicts(&candidates[wave_end], &candidates[start..wave_end])
            {
                wave_end += 1;
            }
            let mut proposals = vec![Vec::new(); wave_end - start];
            let chunk_size = proposals.len().div_ceil(workers);
            let prepare = |offset: usize, chunk: &mut [Vec<Pos2>]| {
                for (local_index, proposal) in chunk.iter_mut().enumerate() {
                    let index = start + offset + local_index;
                    *proposal = if graph_route_conflicts(&candidates[index], &routes) {
                        let (source, target) = endpoints[index];
                        grid.route_edge_with_ports(source, target, ports[index], &routes)
                    } else {
                        candidates[index].clone()
                    };
                }
            };
            if proposals.len() == 1 {
                prepare(0, &mut proposals);
            } else {
                thread::scope(|scope| {
                    for (chunk_index, chunk) in proposals.chunks_mut(chunk_size).enumerate() {
                        let prepare = &prepare;
                        scope.spawn(move || prepare(chunk_index * chunk_size, chunk));
                    }
                });
            }
            // At least the first proposal is valid for the snapshot. Recompute the remaining
            // suffix if a newly accepted route invalidates a speculative proposal.
            for proposal in proposals {
                if graph_route_conflicts(&proposal, &routes[start..]) {
                    break;
                }
                routes.push(proposal);
                advance_graph_progress(progress);
            }
        }
    }
    routes
}

fn graph_route_conflicts(candidate: &[Pos2], routes: &[Vec<Pos2>]) -> bool {
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

pub(super) fn relationship_graph_fingerprint(graph: &RelationshipGraph) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for node in &graph.nodes {
        node.id.hash(&mut hasher);
        node.label.hash(&mut hasher);
        node.path.hash(&mut hasher);
        node.partition.hash(&mut hasher);
    }
    for edge in &graph.edges {
        edge.source.hash(&mut hasher);
        edge.target.hash(&mut hasher);
        edge.label.hash(&mut hasher);
    }
    graph.directed.hash(&mut hasher);
    graph.partition_names.hash(&mut hasher);
    hasher.finish()
}

pub(in crate::app::views) fn graph_edge_color(label: &str, colors: SyntaxColors) -> egui::Color32 {
    let hash = label.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let palette = [
        colors.number,
        colors.string,
        colors.boolean,
        colors.metadata,
        colors.comment,
        colors.null,
        colors.matched,
        colors.error,
    ];
    palette[(hash % palette.len() as u64) as usize]
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(in crate::app::views) enum GraphNodeSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl GraphNodeSide {
    fn for_direction(direction: Vec2) -> Self {
        let half_size = GRAPH_NODE_SIZE / 2.0;
        let horizontal_distance = if direction.x.abs() > 0.0 {
            half_size.x / direction.x.abs()
        } else {
            f32::INFINITY
        };
        let vertical_distance = if direction.y.abs() > 0.0 {
            half_size.y / direction.y.abs()
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

    fn max_lane_offset(self) -> f32 {
        match self {
            Self::Left | Self::Right => GRAPH_NODE_SIZE.y / 2.0 - 10.0,
            Self::Top | Self::Bottom => GRAPH_NODE_SIZE.x / 2.0 - 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::app::views) struct GraphEdgePorts {
    pub(in crate::app::views) source_side: GraphNodeSide,
    pub(in crate::app::views) target_side: GraphNodeSide,
    pub(in crate::app::views) source_offset: f32,
    pub(in crate::app::views) target_offset: f32,
}

struct GraphPortEndpoint {
    edge_index: usize,
    is_source: bool,
    opposite_coordinate: f32,
}

pub(in crate::app::views) fn graph_edge_ports(
    node_positions: &[Pos2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let mut groups: HashMap<(usize, GraphNodeSide), Vec<GraphPortEndpoint>> = HashMap::new();
    let mut ports = Vec::with_capacity(edge_endpoints.len());

    for (edge_index, &(source, target)) in edge_endpoints.iter().enumerate() {
        let direction = (node_positions[target] - node_positions[source]).normalized();
        let source_side = GraphNodeSide::for_direction(direction);
        let target_side = GraphNodeSide::for_direction(-direction);
        ports.push(GraphEdgePorts {
            source_side,
            target_side,
            source_offset: 0.0,
            target_offset: 0.0,
        });
        let opposite_coordinate = |node: usize, side: GraphNodeSide| match side {
            GraphNodeSide::Left | GraphNodeSide::Right => node_positions[node].y,
            GraphNodeSide::Top | GraphNodeSide::Bottom => node_positions[node].x,
        };
        groups
            .entry((source, source_side))
            .or_default()
            .push(GraphPortEndpoint {
                edge_index,
                is_source: true,
                opposite_coordinate: opposite_coordinate(target, source_side),
            });
        groups
            .entry((target, target_side))
            .or_default()
            .push(GraphPortEndpoint {
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
        let lane_spacing = (2.0 * side.max_lane_offset()
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

    ports
}

pub(in crate::app::views) const GRAPH_ROUTE_CLEARANCE: f32 = 18.0;
pub(in crate::app::views) const GRAPH_EDGE_CLEARANCE: f32 = 8.0;
const GRAPH_ROUTE_TURN_PENALTY: f32 = 48.0;
const GRAPH_EDGE_ROUTE_PENALTY: f32 = 500.0;
const GRAPH_EDGE_CROSSING_PENALTY: f32 = 10_000.0;
const GRAPH_ROUTE_DIRECTIONS: usize = 3;
const GRAPH_ROUTE_HORIZONTAL: usize = 0;
const GRAPH_ROUTE_VERTICAL: usize = 1;
const GRAPH_ROUTE_NO_DIRECTION: usize = 2;

#[derive(Clone, Copy)]
struct GraphRoutePort {
    card: Pos2,
    route: Pos2,
}

#[derive(Debug)]
struct GraphRouteQueueEntry {
    estimated_total: f32,
    distance: f32,
    state: usize,
}

impl PartialEq for GraphRouteQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.estimated_total.total_cmp(&other.estimated_total) == Ordering::Equal
            && self.distance.total_cmp(&other.distance) == Ordering::Equal
            && self.state == other.state
    }
}

impl Eq for GraphRouteQueueEntry {}

impl PartialOrd for GraphRouteQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GraphRouteQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .total_cmp(&self.estimated_total)
            .then_with(|| other.distance.total_cmp(&self.distance))
            .then_with(|| other.state.cmp(&self.state))
    }
}

pub(in crate::app::views) struct GraphRoutingGrid {
    node_positions: Vec<Pos2>,
    node_rects: Vec<egui::Rect>,
    obstacles: Vec<egui::Rect>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
}

impl GraphRoutingGrid {
    pub(in crate::app::views) fn new(node_positions: &[Pos2]) -> Self {
        let node_rects = node_positions
            .iter()
            .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
            .collect::<Vec<_>>();
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(GRAPH_ROUTE_CLEARANCE))
            .collect::<Vec<_>>();
        let mut x_coordinates = Vec::with_capacity(node_positions.len() * 3);
        let mut y_coordinates = Vec::with_capacity(node_positions.len() * 3);

        for (center, obstacle) in node_positions.iter().zip(&obstacles) {
            x_coordinates.extend([obstacle.left(), center.x, obstacle.right()]);
            y_coordinates.extend([obstacle.top(), center.y, obstacle.bottom()]);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Self {
            node_positions: node_positions.to_vec(),
            node_rects,
            obstacles,
            x_coordinates,
            y_coordinates,
        }
    }

    #[cfg(test)]
    pub(in crate::app::views) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        let direction = (self.node_positions[target] - self.node_positions[source]).normalized();
        self.route_edge_with_ports(
            source,
            target,
            GraphEdgePorts {
                source_side: GraphNodeSide::for_direction(direction),
                target_side: GraphNodeSide::for_direction(-direction),
                source_offset: 0.0,
                target_offset: 0.0,
            },
            &[],
        )
    }

    pub(in crate::app::views) fn route_edge_with_ports(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let source_port = self.route_port(source, edge_ports.source_side, edge_ports.source_offset);
        let target_port = self.route_port(target, edge_ports.target_side, edge_ports.target_offset);
        let line_start = source_port.card;
        let line_end = target_port.card;
        let crosses_another_node = self.obstacles.iter().enumerate().any(|(index, obstacle)| {
            index != source
                && index != target
                && segment_intersects_rect(line_start, line_end, *obstacle)
        });
        let overlaps_another_edge =
            routed_edges
                .iter()
                .flat_map(|route| route.windows(2))
                .any(|segment| {
                    segments_within_clearance(
                        line_start,
                        line_end,
                        segment[0],
                        segment[1],
                        GRAPH_EDGE_CLEARANCE,
                    )
                });
        if !crosses_another_node && !overlaps_another_edge {
            return vec![line_start, line_end];
        }

        let mut x_coordinates = self.x_coordinates.clone();
        x_coordinates.extend([source_port.route.x, target_port.route.x]);
        let mut y_coordinates = self.y_coordinates.clone();
        y_coordinates.extend([source_port.route.y, target_port.route.y]);
        // Existing obstacle boundaries alone cannot separate detoured parallel edges.
        let track_spacing = GRAPH_EDGE_CLEARANCE + 2.0;
        for point in routed_edges.iter().flatten() {
            x_coordinates.extend([point.x - track_spacing, point.x + track_spacing]);
            y_coordinates.extend([point.y - track_spacing, point.y + track_spacing]);
        }
        x_coordinates.retain(|coordinate| *coordinate >= 0.0);
        y_coordinates.retain(|coordinate| *coordinate >= 0.0);
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);
        let route = self.find_orthogonal_path(
            source_port.route,
            target_port.route,
            &x_coordinates,
            &y_coordinates,
            routed_edges,
        );

        let mut points = Vec::with_capacity(route.len() + 2);
        points.push(source_port.card);
        points.extend(route);
        points.push(target_port.card);
        Self::simplify_graph_route(points)
    }

    fn simplify_graph_route(points: Vec<Pos2>) -> Vec<Pos2> {
        let mut simplified: Vec<Pos2> = Vec::with_capacity(points.len());
        for point in points {
            if simplified.last() == Some(&point) {
                continue;
            }
            while simplified.len() >= 2 {
                let start = simplified[simplified.len() - 2];
                let middle = simplified[simplified.len() - 1];
                let first = middle - start;
                let second = point - middle;
                if cross_product(first, second) != 0.0 || first.dot(second) <= 0.0 {
                    break;
                }
                simplified.pop();
            }
            simplified.push(point);
        }
        simplified
    }

    fn route_port(&self, node: usize, side: GraphNodeSide, offset: f32) -> GraphRoutePort {
        let center = self.node_positions[node];
        let card = self.node_rects[node];
        let route = self.obstacles[node];
        match side {
            GraphNodeSide::Left => GraphRoutePort {
                card: Pos2::new(card.left(), center.y + offset),
                route: Pos2::new(route.left(), center.y + offset),
            },
            GraphNodeSide::Right => GraphRoutePort {
                card: Pos2::new(card.right(), center.y + offset),
                route: Pos2::new(route.right(), center.y + offset),
            },
            GraphNodeSide::Top => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.top()),
                route: Pos2::new(center.x + offset, route.top()),
            },
            GraphNodeSide::Bottom => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.bottom()),
                route: Pos2::new(center.x + offset, route.bottom()),
            },
        }
    }

    fn find_orthogonal_path(
        &self,
        source: Pos2,
        target: Pos2,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        let state_count = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let mut distances = vec![f32::INFINITY; state_count];
        let mut previous_states = vec![usize::MAX; state_count];
        let segment_index = GraphRouteSegmentIndex::new(routed_edges);
        let mut segment_costs = HashMap::new();
        let goal_vertex = Self::vertex_index(target, x_coordinates, y_coordinates);
        let start_vertex = Self::vertex_index(source, x_coordinates, y_coordinates);
        let mut queue = BinaryHeap::new();
        let start_state = start_vertex * GRAPH_ROUTE_DIRECTIONS + GRAPH_ROUTE_NO_DIRECTION;
        distances[start_state] = 0.0;
        queue.push(GraphRouteQueueEntry {
            estimated_total: route_heuristic(source, &[target]),
            distance: 0.0,
            state: start_state,
        });

        let mut goal_state = None;
        while let Some(entry) = queue.pop() {
            if entry.distance > distances[entry.state] {
                continue;
            }

            let current_vertex = entry.state / GRAPH_ROUTE_DIRECTIONS;
            if current_vertex == goal_vertex {
                goal_state = Some(entry.state);
                break;
            }

            let column = current_vertex % width;
            let row = current_vertex / width;
            let current_point = Self::point_at(current_vertex, x_coordinates, y_coordinates);
            let mut neighbors = [None; 4];
            if column > 0 {
                neighbors[0] = Some((
                    current_vertex - 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column] - x_coordinates[column - 1],
                ));
            }
            if column + 1 < width {
                neighbors[1] = Some((
                    current_vertex + 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column + 1] - x_coordinates[column],
                ));
            }
            if row > 0 {
                neighbors[2] = Some((
                    current_vertex - width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row] - y_coordinates[row - 1],
                ));
            }
            if row + 1 < y_coordinates.len() {
                neighbors[3] = Some((
                    current_vertex + width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row + 1] - y_coordinates[row],
                ));
            }

            for (next_vertex, direction, segment_length) in neighbors.into_iter().flatten() {
                let next_point = Self::point_at(next_vertex, x_coordinates, y_coordinates);
                if self.obstacles.iter().any(|obstacle| {
                    segment_crosses_rect_interior(current_point, next_point, *obstacle)
                }) {
                    continue;
                }
                let edge_key = (
                    current_vertex.min(next_vertex),
                    current_vertex.max(next_vertex),
                );
                let edge_overlap_penalty = *segment_costs
                    .entry(edge_key)
                    .or_insert_with(|| segment_index.penalty(current_point, next_point));

                let turn_penalty = if entry.state % GRAPH_ROUTE_DIRECTIONS
                    != GRAPH_ROUTE_NO_DIRECTION
                    && entry.state % GRAPH_ROUTE_DIRECTIONS != direction
                {
                    GRAPH_ROUTE_TURN_PENALTY
                } else {
                    0.0
                };
                let next_distance =
                    entry.distance + segment_length + turn_penalty + edge_overlap_penalty;
                let next_state = next_vertex * GRAPH_ROUTE_DIRECTIONS + direction;
                if next_distance >= distances[next_state] {
                    continue;
                }

                distances[next_state] = next_distance;
                previous_states[next_state] = entry.state;
                queue.push(GraphRouteQueueEntry {
                    estimated_total: next_distance + route_heuristic(next_point, &[target]),
                    distance: next_distance,
                    state: next_state,
                });
            }
        }

        let mut state = goal_state.expect("graph nodes must have an obstacle-free route");
        let mut route = Vec::new();
        loop {
            route.push(Self::point_at(
                state / GRAPH_ROUTE_DIRECTIONS,
                x_coordinates,
                y_coordinates,
            ));
            let previous = previous_states[state];
            if previous == usize::MAX {
                break;
            }
            state = previous;
        }
        route.reverse();
        route
    }

    fn vertex_index(point: Pos2, x_coordinates: &[f32], y_coordinates: &[f32]) -> usize {
        let column = x_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.x))
            .expect("graph route x coordinates must include every port");
        let row = y_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.y))
            .expect("graph route y coordinates must include every port");
        row * x_coordinates.len() + column
    }

    fn point_at(vertex: usize, x_coordinates: &[f32], y_coordinates: &[f32]) -> Pos2 {
        let width = x_coordinates.len();
        Pos2::new(x_coordinates[vertex % width], y_coordinates[vertex / width])
    }
}

fn sort_unique_coordinates(coordinates: &mut Vec<f32>) {
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn route_heuristic(point: Pos2, goals: &[Pos2]) -> f32 {
    goals
        .iter()
        .map(|goal| (point.x - goal.x).abs() + (point.y - goal.y).abs())
        .fold(f32::INFINITY, f32::min)
}

pub(super) fn segment_intersects_rect(start: Pos2, end: Pos2, rect: egui::Rect) -> bool {
    let direction = end - start;
    let mut first_intersection: f32 = 0.0;
    let mut last_intersection: f32 = 1.0;
    for (origin, delta, minimum, maximum) in [
        (start.x, direction.x, rect.left(), rect.right()),
        (start.y, direction.y, rect.top(), rect.bottom()),
    ] {
        if delta == 0.0 {
            if origin <= minimum || origin >= maximum {
                return false;
            }
            continue;
        }

        let first = (minimum - origin) / delta;
        let last = (maximum - origin) / delta;
        first_intersection = first_intersection.max(first.min(last));
        last_intersection = last_intersection.min(first.max(last));
        if first_intersection >= last_intersection {
            return false;
        }
    }
    first_intersection < last_intersection
}

pub(in crate::app::views) struct GraphRouteSegmentIndex {
    segments: Vec<[Pos2; 2]>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl GraphRouteSegmentIndex {
    fn cells(start: Pos2, end: Pos2) -> impl Iterator<Item = (i32, i32)> {
        let bounds = egui::Rect::from_two_pos(start, end).expand(GRAPH_EDGE_CLEARANCE);
        let cell_size = 128.0;
        let left = (bounds.left() / cell_size).floor() as i32;
        let right = (bounds.right() / cell_size).floor() as i32;
        let top = (bounds.top() / cell_size).floor() as i32;
        let bottom = (bounds.bottom() / cell_size).floor() as i32;
        (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
    }

    pub(in crate::app::views) fn new(routes: &[Vec<Pos2>]) -> Self {
        let segments = routes
            .iter()
            .flat_map(|route| route.windows(2))
            .map(|segment| [segment[0], segment[1]])
            .collect::<Vec<_>>();
        let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (index, segment) in segments.iter().enumerate() {
            for cell in Self::cells(segment[0], segment[1]) {
                buckets.entry(cell).or_default().push(index);
            }
        }
        Self { segments, buckets }
    }

    fn penalty(&self, start: Pos2, end: Pos2) -> f32 {
        let mut indices = Self::cells(start, end)
            .filter_map(|cell| self.buckets.get(&cell))
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .map(|index| segment_pair_penalty(start, end, self.segments[index]))
            .sum()
    }
}

fn segment_pair_penalty(start: Pos2, end: Pos2, segment: [Pos2; 2]) -> f32 {
    if segments_intersect(start, end, segment[0], segment[1])
        || segments_within_clearance(start, end, segment[0], segment[1], 1.0)
    {
        GRAPH_EDGE_CROSSING_PENALTY
    } else if segments_within_clearance(start, end, segment[0], segment[1], GRAPH_EDGE_CLEARANCE) {
        GRAPH_EDGE_ROUTE_PENALTY
    } else {
        0.0
    }
}

pub(in crate::app::views) fn segments_within_clearance(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
    clearance: f32,
) -> bool {
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return true;
    }
    [
        point_to_segment_distance(first_start, second_start, second_end),
        point_to_segment_distance(first_end, second_start, second_end),
        point_to_segment_distance(second_start, first_start, first_end),
        point_to_segment_distance(second_end, first_start, first_end),
    ]
    .into_iter()
    .any(|distance| distance <= clearance)
}

pub(super) fn segments_intersect(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
) -> bool {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let denominator = cross_product(first_direction, second_direction);
    if denominator == 0.0 {
        return false;
    }

    let between_starts = second_start - first_start;
    let first_position = cross_product(between_starts, second_direction) / denominator;
    let second_position = cross_product(between_starts, first_direction) / denominator;
    (0.0..=1.0).contains(&first_position) && (0.0..=1.0).contains(&second_position)
}

fn cross_product(first: Vec2, second: Vec2) -> f32 {
    first.x * second.y - first.y * second.x
}

pub(super) fn point_to_segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    point.distance(closest_point_on_segment(point, start, end))
}

pub(super) fn closest_point_on_segment(point: Pos2, start: Pos2, end: Pos2) -> Pos2 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return start;
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    start + segment * projection
}

pub(in crate::app::views) fn segment_crosses_rect_interior(
    start: Pos2,
    end: Pos2,
    rect: egui::Rect,
) -> bool {
    if start.y == end.y {
        start.y > rect.top()
            && start.y < rect.bottom()
            && start.x.min(end.x) < rect.right()
            && start.x.max(end.x) > rect.left()
    } else {
        start.x > rect.left()
            && start.x < rect.right()
            && start.y.min(end.y) < rect.bottom()
            && start.y.max(end.y) > rect.top()
    }
}

#[cfg(test)]
pub(in crate::app::views) fn box_border_offset(direction: Vec2) -> f32 {
    (GRAPH_NODE_SIZE.x / 2.0 / direction.x.abs()).min(GRAPH_NODE_SIZE.y / 2.0 / direction.y.abs())
}

pub(super) fn draw_arrow_head(
    painter: &egui::Painter,
    tip: Pos2,
    direction: Vec2,
    stroke: Stroke,
    zoom: f32,
) {
    let arrow_length = 9.0 * zoom;
    for angle_offset in [2.55, -2.55] {
        let wing = tip + Vec2::angled(direction.angle() + angle_offset) * arrow_length;
        painter.line_segment([tip, wing], stroke);
    }
}

#[cfg(test)]
mod tests;
