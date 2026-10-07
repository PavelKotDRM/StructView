use super::*;
use crate::{RoutingError, RoutingResult};
use std::collections::{HashMap, HashSet};

mod search;

/// Default clearance around node rectangles.
pub const DEFAULT_ROUTE_CLEARANCE: f32 = 18.0;
/// Default spacing between unrelated routes.
pub const DEFAULT_EDGE_CLEARANCE: f32 = 8.0;
/// Default number of additional track coordinates per axis.
pub const DEFAULT_ROUTE_TRACK_LIMIT: usize = 128;

const ROUTING_OBSTACLE_CELL_SIZE: f32 = 256.0;
const ROUTE_PORT_LEAD_EXTRA: f32 = 2.0;
/// Cost added for each turn, including turns at the endpoint port leads.
pub const GRAPH_ROUTE_TURN_PENALTY: f32 = 96.0;
/// Cost for taking the less-preferred side around an obstacle.
pub const GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY: f32 = 240.0;
const GRAPH_ROUTE_DIRECTIONS: usize = 2;
const GRAPH_ROUTE_HORIZONTAL: usize = 0;
const GRAPH_ROUTE_VERTICAL: usize = 1;

#[derive(Clone, Copy)]
struct PortGeometry {
    card: Point,
    route: Point,
    escape: Point,
    outward: Vector,
}

#[derive(Clone, Copy)]
struct RoutePreferences<'a> {
    direct_direction: Vector,
    detoured_obstacles: &'a [Rect],
}

#[derive(Clone, Copy)]
struct RouteSearchOptions<'a> {
    segment_index: &'a RouteIndex,
    preferences: RoutePreferences<'a>,
    conflict_clearance: Option<f32>,
    start_direction: usize,
    goal_direction: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrthogonalRouterOptions {
    /// A* implementation used to search the routing grid.
    pub search_backend: RoutingSearchBackend,
    /// Node card size used for obstacle bounds and edge ports.
    pub node_size: Size,
    /// Clearance around node rectangles.
    pub obstacle_clearance: f32,
    /// Clearance used to separate unrelated routes.
    pub edge_clearance: f32,
    /// Layout step used to bound local searches.
    pub grid_step: Size,
    /// Maximum additional track coordinates retained on each axis.
    pub track_limit: usize,
}

impl OrthogonalRouterOptions {
    /// Create options with default clearances and track count.
    pub fn new(node_size: Size, grid_step: Size) -> Self {
        Self {
            search_backend: RoutingSearchBackend::Builtin,
            node_size,
            obstacle_clearance: DEFAULT_ROUTE_CLEARANCE,
            edge_clearance: DEFAULT_EDGE_CLEARANCE,
            grid_step,
            track_limit: DEFAULT_ROUTE_TRACK_LIMIT,
        }
    }

    fn is_valid(self) -> bool {
        self.node_size.is_valid()
            && self.grid_step.is_valid()
            && self.obstacle_clearance.is_finite()
            && self.obstacle_clearance >= 0.0
            && self.edge_clearance.is_finite()
            && self.edge_clearance >= 0.0
            && self.track_limit > 0
    }
}

/// Builds obstacle-aware orthogonal routes between positioned nodes.
#[derive(Debug)]
pub struct OrthogonalRouter {
    node_positions: Vec<Point>,
    routed_nodes: Vec<bool>,
    node_rects: Vec<Rect>,
    obstacles: Vec<Rect>,
    obstacle_buckets: HashMap<(i32, i32), Vec<usize>>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
    options: OrthogonalRouterOptions,
}

impl OrthogonalRouter {
    /// Create a router. Only `routed_nodes` become obstacles to edge paths.
    pub fn new(
        node_positions: &[Point],
        routed_nodes: &[usize],
        options: OrthogonalRouterOptions,
    ) -> RoutingResult<Self> {
        if !options.is_valid() || node_positions.iter().any(|point| !point.is_finite()) {
            return Err(RoutingError::InvalidGeometry);
        }
        let mut unique_routed_nodes = Vec::with_capacity(routed_nodes.len());
        let mut seen_routed_nodes = HashSet::with_capacity(routed_nodes.len());
        for &index in routed_nodes {
            if index >= node_positions.len() {
                return Err(RoutingError::InvalidNodeIndex);
            }
            if seen_routed_nodes.insert(index) {
                unique_routed_nodes.push(index);
            }
        }
        let mut routed_node_mask = vec![false; node_positions.len()];
        for &index in &unique_routed_nodes {
            routed_node_mask[index] = true;
        }

        let node_rects = node_positions
            .iter()
            .map(|center| Rect::from_center_size(*center, options.node_size))
            .collect::<Vec<_>>();
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(options.obstacle_clearance))
            .collect::<Vec<_>>();
        if node_rects
            .iter()
            .chain(&obstacles)
            .any(|rect| !rect.is_valid())
        {
            return Err(RoutingError::InvalidGeometry);
        }
        let mut obstacle_buckets = HashMap::<(i32, i32), Vec<usize>>::new();
        for &index in &unique_routed_nodes {
            for cell in routing_grid_cells(obstacles[index]) {
                obstacle_buckets.entry(cell).or_default().push(index);
            }
        }
        let mut x_coordinates = Vec::with_capacity(unique_routed_nodes.len() * 3);
        let mut y_coordinates = Vec::with_capacity(unique_routed_nodes.len() * 3);
        for &index in &unique_routed_nodes {
            let center = node_positions[index];
            let obstacle = obstacles[index];
            x_coordinates.extend([obstacle.left(), center.x, obstacle.right()]);
            y_coordinates.extend([obstacle.top(), center.y, obstacle.bottom()]);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Ok(Self {
            node_positions: node_positions.to_vec(),
            routed_nodes: routed_node_mask,
            node_rects,
            obstacles,
            obstacle_buckets,
            x_coordinates,
            y_coordinates,
            options,
        })
    }

    /// Route an edge using an index of previously accepted routes.
    pub fn route_edge(
        &self,
        source: usize,
        target: usize,
        edge_ports: EdgePorts,
        routed_edge_index: &RouteIndex,
    ) -> RoutingResult<Vec<Point>> {
        if source >= self.node_positions.len()
            || target >= self.node_positions.len()
            || !self.routed_nodes[source]
            || !self.routed_nodes[target]
        {
            return Err(RoutingError::InvalidNodeIndex);
        }
        if !edge_ports.source_offset.is_finite() || !edge_ports.target_offset.is_finite() {
            return Err(RoutingError::InvalidGeometry);
        }
        let route = self.route_edge_with_track_limit(
            source,
            target,
            edge_ports,
            routed_edge_index,
            self.options.track_limit,
        )?;
        Ok(self.detour_shared_route_segments(route, routed_edge_index))
    }

    /// Route an edge using a list of previously accepted routes.
    pub fn route_edge_with_routes(
        &self,
        source: usize,
        target: usize,
        edge_ports: EdgePorts,
        routed_edges: &[Vec<Point>],
    ) -> RoutingResult<Vec<Point>> {
        let index = RouteIndex::new(routed_edges, self.options.edge_clearance)?;
        self.route_edge(source, target, edge_ports, &index)
    }

    fn detour_shared_route_segments(
        &self,
        mut route: Vec<Point>,
        routed_edge_index: &RouteIndex,
    ) -> Vec<Point> {
        for _ in 0..GRAPH_ROUTE_SHARED_SEGMENT_DETOUR_LIMIT {
            let Some((segment_index, shared_segment, _)) = routed_edge_index
                .first_overlapping_segment(&route, GRAPH_ROUTE_SHARED_SEGMENT_VISIBLE_THRESHOLD)
            else {
                break;
            };
            let Some(detoured_route) = self.detour_shared_route_segment(
                &route,
                segment_index,
                shared_segment,
                routed_edge_index,
            ) else {
                break;
            };
            route = Self::simplify_graph_route(detoured_route);
        }
        route
    }

    fn route_edge_with_track_limit(
        &self,
        source: usize,
        target: usize,
        edge_ports: EdgePorts,
        routed_edge_index: &RouteIndex,
        track_limit: usize,
    ) -> RoutingResult<Vec<Point>> {
        let source_port = self.route_port(
            source,
            edge_ports.source_side,
            edge_ports.source_offset,
            self.options.edge_clearance + ROUTE_PORT_LEAD_EXTRA,
        );
        let target_port = self.route_port(
            target,
            edge_ports.target_side,
            edge_ports.target_offset,
            self.options.edge_clearance + ROUTE_PORT_LEAD_EXTRA,
        );
        if [
            source_port.card,
            source_port.route,
            source_port.escape,
            target_port.card,
            target_port.route,
            target_port.escape,
        ]
        .into_iter()
        .any(|point| !point.is_finite())
        {
            return Err(RoutingError::InvalidGeometry);
        }
        let line_start = source_port.card;
        let line_end = target_port.card;
        let crosses_another_node =
            self.segment_crosses_obstacle(line_start, line_end, Some((source, target)));
        let conflicts_with_another_edge =
            routed_edge_index.conflicts_route(&[line_start, line_end]);
        if source != target && !crosses_another_node && !conflicts_with_another_edge {
            return Ok(vec![line_start, line_end]);
        }

        let mut x_coordinates = self.x_coordinates.clone();
        x_coordinates.extend([source_port.escape.x, target_port.escape.x]);
        let mut y_coordinates = self.y_coordinates.clone();
        y_coordinates.extend([source_port.escape.y, target_port.escape.y]);
        // Existing obstacle boundaries alone cannot separate detoured parallel edges.
        let track_spacing = self.options.edge_clearance + 2.0;
        let track_bounds = Rect::from_two_points(source_port.escape, target_port.escape)
            .expand(self.options.obstacle_clearance + self.options.node_size.width / 2.0);
        let mut track_x = Vec::new();
        let mut track_y = Vec::new();
        for segment in routed_edge_index.segments_near(track_bounds) {
            for point in segment {
                for offset in [
                    track_spacing,
                    2.0 * track_spacing,
                    3.0 * track_spacing,
                    4.0 * track_spacing,
                ] {
                    track_x.extend([point.x - offset, point.x + offset]);
                    track_y.extend([point.y - offset, point.y + offset]);
                }
                track_x.push(point.x);
                track_y.push(point.y);
            }
        }
        // Keep the Cartesian search grid bounded while preserving nearby alternatives.
        retain_nearest_route_tracks(
            &mut track_x,
            source_port.escape.x,
            target_port.escape.x,
            track_limit,
            self.options.edge_clearance,
        );
        retain_nearest_route_tracks(
            &mut track_y,
            source_port.escape.y,
            target_port.escape.y,
            track_limit,
            self.options.edge_clearance,
        );
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
        let preferences = RoutePreferences {
            direct_direction,
            detoured_obstacles: &detoured_obstacles,
        };
        let port_direction = |port: PortGeometry| {
            if port.outward.x != 0.0 {
                GRAPH_ROUTE_HORIZONTAL
            } else {
                GRAPH_ROUTE_VERTICAL
            }
        };
        let search_options = |conflict_clearance| RouteSearchOptions {
            segment_index: routed_edge_index,
            preferences,
            conflict_clearance,
            start_direction: port_direction(source_port),
            goal_direction: port_direction(target_port),
        };
        let route_from_path = |path: Vec<Point>| {
            let mut points = Vec::with_capacity(path.len() + 6);
            points.push(source_port.card);
            points.push(source_port.route);
            points.push(source_port.escape);
            points.extend(path);
            points.push(target_port.escape);
            points.push(target_port.route);
            points.push(target_port.card);
            Self::simplify_graph_route(points)
        };
        for vertical_scale in [1.0, 2.0, 4.0, 8.0] {
            let margin = Vector::new(
                self.options.grid_step.width * 2.0,
                self.options.grid_step.height * vertical_scale,
            );
            let local_bounds = Rect::from_min_max(
                Point::new(
                    source_port.escape.x.min(target_port.escape.x) - margin.x,
                    source_port.escape.y.min(target_port.escape.y) - margin.y,
                ),
                Point::new(
                    source_port.escape.x.max(target_port.escape.x) + margin.x,
                    source_port.escape.y.max(target_port.escape.y) + margin.y,
                ),
            );
            let local_x_coordinates = x_coordinates
                .iter()
                .copied()
                .filter(|coordinate| {
                    (local_bounds.left()..=local_bounds.right()).contains(coordinate)
                })
                .collect::<Vec<_>>();
            let local_y_coordinates = y_coordinates
                .iter()
                .copied()
                .filter(|coordinate| {
                    (local_bounds.top()..=local_bounds.bottom()).contains(coordinate)
                })
                .collect::<Vec<_>>();
            let first_path = self.find_orthogonal_path(
                source_port.escape,
                target_port.escape,
                &local_x_coordinates,
                &local_y_coordinates,
                search_options(Some(self.options.edge_clearance)),
            )?;
            let path = if first_path.is_some() {
                first_path
            } else {
                self.find_orthogonal_path(
                    source_port.escape,
                    target_port.escape,
                    &local_x_coordinates,
                    &local_y_coordinates,
                    search_options(Some(0.0)),
                )?
            };
            if let Some(path) = path {
                let route = route_from_path(path);
                if !routed_edge_index.intersects_route(&route)
                    && routed_edge_index
                        .first_overlapping_segment(
                            &route,
                            GRAPH_ROUTE_SHARED_SEGMENT_SEARCH_THRESHOLD,
                        )
                        .is_none()
                {
                    return Ok(route);
                }
            }
            if vertical_scale == 8.0
                && let Some(path) = self.find_orthogonal_path(
                    source_port.escape,
                    target_port.escape,
                    &local_x_coordinates,
                    &local_y_coordinates,
                    search_options(None),
                )?
            {
                return Ok(route_from_path(path));
            }
        }
        let mut global_x_coordinates = x_coordinates.clone();
        let mut global_y_coordinates = y_coordinates.clone();
        global_x_coordinates.push(x_coordinates.last().copied().unwrap_or(0.0) + track_spacing);
        global_y_coordinates.push(y_coordinates.last().copied().unwrap_or(0.0) + track_spacing);
        let mut path = self.find_orthogonal_path(
            source_port.escape,
            target_port.escape,
            &global_x_coordinates,
            &global_y_coordinates,
            search_options(Some(self.options.edge_clearance)),
        )?;
        if path.is_none() {
            path = self.find_orthogonal_path(
                source_port.escape,
                target_port.escape,
                &global_x_coordinates,
                &global_y_coordinates,
                search_options(Some(0.0)),
            )?;
        }
        if path.is_none() {
            path = self.find_orthogonal_path(
                source_port.escape,
                target_port.escape,
                &global_x_coordinates,
                &global_y_coordinates,
                search_options(None),
            )?;
        }
        path.map(route_from_path)
            .ok_or(RoutingError::NoOrthogonalPath)
    }

    fn detour_shared_route_segment(
        &self,
        route: &[Point],
        segment_index: usize,
        shared_segment: [Point; 2],
        routed_edge_index: &RouteIndex,
    ) -> Option<Vec<Point>> {
        let start = route[segment_index];
        let end = route[segment_index + 1];
        let direction = (end - start).normalized();
        let normal = Vector::new(-direction.y, direction.x);
        for distance in [
            self.options.edge_clearance + 2.0,
            self.options.obstacle_clearance,
            32.0,
            48.0,
            64.0,
            96.0,
            128.0,
            192.0,
        ] {
            for sign in [1.0, -1.0] {
                let offset = normal * (distance * sign);
                let shifted_start = shared_segment[0] + offset;
                let shifted_end = shared_segment[1] + offset;
                let mut replacement = Vec::with_capacity(6);
                for point in [
                    start,
                    shared_segment[0],
                    shifted_start,
                    shifted_end,
                    shared_segment[1],
                    end,
                ] {
                    if replacement.last() != Some(&point) {
                        replacement.push(point);
                    }
                }
                if replacement
                    .iter()
                    .any(|point| point.x < 0.0 || point.y < 0.0)
                {
                    continue;
                }
                if replacement
                    .windows(2)
                    .any(|segment| self.segment_crosses_obstacle(segment[0], segment[1], None))
                    || routed_edge_index.overlaps_segment(shifted_start, shifted_end)
                {
                    continue;
                }

                let mut detoured_route = Vec::with_capacity(route.len() + 4);
                detoured_route.extend_from_slice(&route[..=segment_index]);
                for &point in replacement.iter().skip(1) {
                    if detoured_route.last() != Some(&point) {
                        detoured_route.push(point);
                    }
                }
                detoured_route.extend_from_slice(&route[segment_index + 2..]);
                return Some(detoured_route);
            }
        }
        None
    }

    pub(super) fn simplify_graph_route(points: Vec<Point>) -> Vec<Point> {
        simplify_route(points)
    }

    fn route_port(&self, node: usize, side: NodeSide, offset: f32, lead: f32) -> PortGeometry {
        let center = self.node_positions[node];
        let card = self.node_rects[node];
        let route = self.obstacles[node];
        let (card, route, outward) = match side {
            NodeSide::Left => (
                Point::new(card.left(), center.y + offset),
                Point::new(route.left(), center.y + offset),
                -Vector::X,
            ),
            NodeSide::Right => (
                Point::new(card.right(), center.y + offset),
                Point::new(route.right(), center.y + offset),
                Vector::X,
            ),
            NodeSide::Top => (
                Point::new(center.x + offset, card.top()),
                Point::new(center.x + offset, route.top()),
                -Vector::Y,
            ),
            NodeSide::Bottom => (
                Point::new(center.x + offset, card.bottom()),
                Point::new(center.x + offset, route.bottom()),
                Vector::Y,
            ),
        };
        let escape = route + outward * lead;
        PortGeometry {
            card,
            route,
            escape: Point::new(escape.x.max(0.0), escape.y.max(0.0)),
            outward,
        }
    }
}

fn routing_grid_cells(rect: Rect) -> impl Iterator<Item = (i32, i32)> {
    let left = (rect.left() / ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let right = (rect.right() / ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let top = (rect.top() / ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    let bottom = (rect.bottom() / ROUTING_OBSTACLE_CELL_SIZE).floor() as i32;
    (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
}

fn sort_unique_coordinates(coordinates: &mut Vec<f32>) {
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn retain_nearest_route_tracks(
    coordinates: &mut Vec<f32>,
    first: f32,
    second: f32,
    limit: usize,
    minimum_spacing: f32,
) {
    coordinates.retain(|coordinate| *coordinate >= 0.0);
    sort_unique_coordinates(coordinates);
    coordinates.sort_by(|left, right| {
        let left_distance = (left - first).abs().min((left - second).abs());
        let right_distance = (right - first).abs().min((right - second).abs());
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| left.total_cmp(right))
    });
    let mut selected = Vec::with_capacity(limit);
    for coordinate in std::mem::take(coordinates) {
        if selected
            .iter()
            .all(|existing: &f32| (coordinate - existing).abs() >= minimum_spacing)
        {
            selected.push(coordinate);
            if selected.len() == limit {
                break;
            }
        }
    }
    *coordinates = selected;
    sort_unique_coordinates(coordinates);
}

fn route_heuristic(point: Point, goals: &[Point]) -> f32 {
    goals
        .iter()
        .map(|goal| (point.x - goal.x).abs() + (point.y - goal.y).abs())
        .fold(f32::INFINITY, f32::min)
}

pub fn detour_side_preference_penalty(
    start: Point,
    end: Point,
    direct_direction: Vector,
    obstacles: &[Rect],
) -> f32 {
    let mostly_horizontal = direct_direction.x.abs() >= direct_direction.y.abs();
    obstacles
        .iter()
        .filter(|obstacle| {
            if mostly_horizontal
                && start.y == end.y
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

/// Remove duplicate points and collinear points that continue in the same direction.
pub fn simplify_route(points: Vec<Point>) -> Vec<Point> {
    let mut simplified: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if simplified.last() == Some(&point) {
            continue;
        }
        while simplified.len() >= 2 {
            let start = simplified[simplified.len() - 2];
            let middle = simplified[simplified.len() - 1];
            let first = middle - start;
            let second = point - middle;
            if cross_product(first, second) != 0.0 {
                break;
            }
            simplified.pop();
        }
        if simplified.last() != Some(&point) {
            simplified.push(point);
        }
    }
    simplified
}
