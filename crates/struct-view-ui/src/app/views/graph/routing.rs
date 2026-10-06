use super::calculation::{
    GraphProgressTracker, GraphStage, advance_graph_progress, begin_graph_stage,
};
use super::*;

mod geometry;

#[cfg(test)]
use super::calculation::{GraphProgress, build_graph_calculation};
pub(in crate::app::views) use geometry::GraphRouteSegmentIndex;
#[cfg(test)]
pub(in crate::app::views) use geometry::box_border_offset;
#[cfg(test)]
pub(in crate::app::views) use geometry::segment_crosses_rect_interior;
#[cfg(test)]
pub(super) use geometry::segment_pair_penalty;
pub(in crate::app::views) use geometry::segments_within_clearance;
pub(super) use geometry::{
    arrow_head_wings, closest_point_on_segment, cross_product, draw_arrow_head, edge_arrowheads,
    point_to_segment_distance, segment_intersects_rect, segments_intersect,
};
#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::Duration;

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};
use std::thread;

const GRAPH_ROUTE_INDEX_CELL_SIZE: f32 = 128.0;

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
        edge.direction.hash(&mut hasher);
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
        let (source_side, target_side) = if source == target {
            (GraphNodeSide::Right, GraphNodeSide::Bottom)
        } else {
            let direction = (node_positions[target] - node_positions[source]).normalized();
            (
                GraphNodeSide::for_direction(direction),
                GraphNodeSide::for_direction(-direction),
            )
        };
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
const GRAPH_ROUTING_OBSTACLE_CELL_SIZE: f32 = 256.0;
const GRAPH_ROUTE_TRACK_LIMIT: usize = 12;
const GRAPH_ROUTE_TURN_PENALTY: f32 = 48.0;
const GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY: f32 = 240.0;
const GRAPH_EDGE_ROUTE_PENALTY: f32 = 2_000.0;
const GRAPH_EDGE_CROSSING_PENALTY: f32 = 50_000.0;
const GRAPH_ROUTE_DIRECTIONS: usize = 3;
const GRAPH_ROUTE_HORIZONTAL: usize = 0;
const GRAPH_ROUTE_VERTICAL: usize = 1;
const GRAPH_ROUTE_NO_DIRECTION: usize = 2;

#[derive(Clone, Copy)]
struct GraphRoutePort {
    card: Pos2,
    route: Pos2,
}

struct GraphRoutePreferences<'a> {
    direct_direction: Vec2,
    detoured_obstacles: &'a [egui::Rect],
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
    obstacle_buckets: HashMap<(i32, i32), Vec<usize>>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
}

impl GraphRoutingGrid {
    #[cfg(test)]
    pub(in crate::app::views) fn new(node_positions: &[Pos2]) -> Self {
        let routed_nodes = (0..node_positions.len()).collect::<Vec<_>>();
        Self::new_for_graph(node_positions, &routed_nodes)
    }

    pub(super) fn new_for_graph(node_positions: &[Pos2], routed_nodes: &[usize]) -> Self {
        let node_rects = node_positions
            .iter()
            .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
            .collect::<Vec<_>>();
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(GRAPH_ROUTE_CLEARANCE))
            .collect::<Vec<_>>();
        let mut obstacle_buckets = HashMap::<(i32, i32), Vec<usize>>::new();
        for &index in routed_nodes {
            for cell in routing_grid_cells(obstacles[index]) {
                obstacle_buckets.entry(cell).or_default().push(index);
            }
        }
        let mut x_coordinates = Vec::with_capacity(routed_nodes.len() * 3);
        let mut y_coordinates = Vec::with_capacity(routed_nodes.len() * 3);

        for &index in routed_nodes {
            let center = node_positions[index];
            let obstacle = obstacles[index];
            x_coordinates.extend([obstacle.left(), center.x, obstacle.right()]);
            y_coordinates.extend([obstacle.top(), center.y, obstacle.bottom()]);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Self {
            node_positions: node_positions.to_vec(),
            node_rects,
            obstacles,
            obstacle_buckets,
            x_coordinates,
            y_coordinates,
        }
    }

    #[cfg(test)]
    pub(in crate::app::views) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        self.route_edge_with_ports(
            source,
            target,
            graph_edge_ports(&self.node_positions, &[(source, target)])[0],
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
        let routed_edge_index = GraphRouteSegmentIndex::new(routed_edges);
        self.route_edge_with_index(source, target, edge_ports, &routed_edge_index)
    }

    fn route_edge_with_index(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edge_index: &GraphRouteSegmentIndex,
    ) -> Vec<Pos2> {
        let source_port = self.route_port(source, edge_ports.source_side, edge_ports.source_offset);
        let target_port = self.route_port(target, edge_ports.target_side, edge_ports.target_offset);
        let line_start = source_port.card;
        let line_end = target_port.card;
        let crosses_another_node =
            self.segment_crosses_obstacle(line_start, line_end, Some((source, target)));
        let overlaps_another_edge = routed_edge_index.conflicts_route(&[line_start, line_end]);
        if source != target && !crosses_another_node && !overlaps_another_edge {
            return vec![line_start, line_end];
        }

        let mut x_coordinates = self.x_coordinates.clone();
        x_coordinates.extend([source_port.route.x, target_port.route.x]);
        let mut y_coordinates = self.y_coordinates.clone();
        y_coordinates.extend([source_port.route.y, target_port.route.y]);
        // Existing obstacle boundaries alone cannot separate detoured parallel edges.
        let track_spacing = GRAPH_EDGE_CLEARANCE + 2.0;
        let track_bounds = egui::Rect::from_two_pos(source_port.route, target_port.route)
            .expand(GRAPH_ROUTE_CLEARANCE + GRAPH_NODE_SIZE.x / 2.0);
        let mut track_x = Vec::new();
        let mut track_y = Vec::new();
        for segment in routed_edge_index.segments_near(track_bounds) {
            for point in segment {
                track_x.extend([point.x - track_spacing, point.x + track_spacing]);
                track_y.extend([point.y - track_spacing, point.y + track_spacing]);
            }
        }
        // Keep the Cartesian search grid bounded while preserving nearby alternatives.
        retain_nearest_route_tracks(&mut track_x, source_port.route.x, target_port.route.x);
        retain_nearest_route_tracks(&mut track_y, source_port.route.y, target_port.route.y);
        x_coordinates.extend(track_x);
        y_coordinates.extend(track_y);
        let direct_direction = line_end - line_start;
        let detoured_obstacles = self
            .obstacles
            .iter()
            .enumerate()
            .filter(|(index, obstacle)| {
                *index != source
                    && *index != target
                    && segment_intersects_rect(line_start, line_end, **obstacle)
            })
            .map(|(_, obstacle)| *obstacle)
            .collect::<Vec<_>>();
        x_coordinates.retain(|coordinate| *coordinate >= 0.0);
        y_coordinates.retain(|coordinate| *coordinate >= 0.0);
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);
        let route = self.find_orthogonal_path(
            source_port.route,
            target_port.route,
            &x_coordinates,
            &y_coordinates,
            routed_edge_index,
            GraphRoutePreferences {
                direct_direction,
                detoured_obstacles: &detoured_obstacles,
            },
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

    fn segment_crosses_obstacle(
        &self,
        start: Pos2,
        end: Pos2,
        excluded_nodes: Option<(usize, usize)>,
    ) -> bool {
        routing_grid_cells(egui::Rect::from_two_pos(start, end)).any(|cell| {
            self.obstacle_buckets.get(&cell).is_some_and(|indices| {
                indices.iter().any(|&index| {
                    !excluded_nodes
                        .is_some_and(|(source, target)| index == source || index == target)
                        && segment_intersects_rect(start, end, self.obstacles[index])
                })
            })
        })
    }

    fn find_orthogonal_path(
        &self,
        source: Pos2,
        target: Pos2,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
        segment_index: &GraphRouteSegmentIndex,
        preferences: GraphRoutePreferences<'_>,
    ) -> Vec<Pos2> {
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        let state_count = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let mut distances = vec![f32::INFINITY; state_count];
        let mut previous_states = vec![usize::MAX; state_count];
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
                if self.segment_crosses_obstacle(current_point, next_point, None) {
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
                let side_preference_penalty = detour_side_preference_penalty(
                    current_point,
                    next_point,
                    preferences.direct_direction,
                    preferences.detoured_obstacles,
                );
                let next_distance = entry.distance
                    + segment_length
                    + turn_penalty
                    + edge_overlap_penalty
                    + side_preference_penalty;
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

fn routing_grid_cells(rect: egui::Rect) -> impl Iterator<Item = (i32, i32)> {
    let left = (rect.left() / GRAPH_ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let right = (rect.right() / GRAPH_ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let top = (rect.top() / GRAPH_ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let bottom = (rect.bottom() / GRAPH_ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
}

fn sort_unique_coordinates(coordinates: &mut Vec<f32>) {
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn retain_nearest_route_tracks(coordinates: &mut Vec<f32>, first: f32, second: f32) {
    coordinates.retain(|coordinate| *coordinate >= 0.0);
    sort_unique_coordinates(coordinates);
    coordinates.sort_by(|left, right| {
        let left_distance = (left - first).abs().min((left - second).abs());
        let right_distance = (right - first).abs().min((right - second).abs());
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| left.total_cmp(right))
    });
    coordinates.truncate(GRAPH_ROUTE_TRACK_LIMIT);
}

fn route_heuristic(point: Pos2, goals: &[Pos2]) -> f32 {
    goals
        .iter()
        .map(|goal| (point.x - goal.x).abs() + (point.y - goal.y).abs())
        .fold(f32::INFINITY, f32::min)
}

fn detour_side_preference_penalty(
    start: Pos2,
    end: Pos2,
    direct_direction: Vec2,
    obstacles: &[egui::Rect],
) -> f32 {
    let mostly_horizontal = direct_direction.x.abs() >= direct_direction.y.abs();
    obstacles
        .iter()
        .filter(|obstacle| {
            if start.y == end.y
                && start.y >= obstacle.bottom()
                && start.x.max(end.x) >= obstacle.left()
                && start.x.min(end.x) <= obstacle.right()
            {
                true
            } else if !mostly_horizontal && start.x == end.x && start.x >= obstacle.right() {
                start.y.max(end.y) >= obstacle.top() && start.y.min(end.y) <= obstacle.bottom()
            } else {
                false
            }
        })
        .count() as f32
        * GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY
}

#[cfg(test)]
mod tests;
