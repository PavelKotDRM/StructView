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
/// Legacy bend scoring weight used by geometry-scoring helpers.
pub const GRAPH_ROUTE_TURN_PENALTY: f32 = 96.0;
/// Search cost of one bend, in the same units as route length.
pub const GRAPH_ROUTE_BEND_COST: f32 = 48.0;
/// Tie-breaking cost per unit of length run along another port's straight exit.
const PORT_EXIT_RAY_COST: f64 = 0.01;
/// Cost for traversing the full span of an obstacle on its less-preferred side.
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

/// Nodes whose port-lead band a detour must stay out of.
#[derive(Clone, Copy)]
enum LeadZones {
    /// Only prevents a route from looping back around its own source and target cards.
    Endpoints(usize, usize),
    None,
}

#[derive(Clone, Copy)]
struct RouteSearchOptions<'a> {
    segment_index: &'a RouteIndex,
    reserved_port_leads: &'a RouteIndex,
    port_exit_rays: &'a RouteIndex,
    own_exit_rays: [[Point; 2]; 2],
    lead_zones: LeadZones,
    lane_clearance: f32,
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
    /// Layout spacing supplied by the caller; retained for API compatibility.
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
    lead_zones: Vec<Rect>,
    obstacle_buckets: HashMap<(i32, i32), Vec<usize>>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
    reserved_port_leads: RouteIndex,
    port_exit_rays: RouteIndex,
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
        // Detours stay outside the port-lead band so they cannot loop back over port leads.
        let lead_zones = obstacles
            .iter()
            .map(|rect| rect.expand(options.edge_clearance + ROUTE_PORT_LEAD_EXTRA))
            .collect::<Vec<_>>();
        let mut obstacle_buckets = HashMap::<(i32, i32), Vec<usize>>::new();
        for &index in &unique_routed_nodes {
            for cell in routing_grid_cells(lead_zones[index]) {
                obstacle_buckets.entry(cell).or_default().push(index);
            }
        }
        let mut x_coordinates = Vec::with_capacity(unique_routed_nodes.len() * 3);
        let mut y_coordinates = Vec::with_capacity(unique_routed_nodes.len() * 3);
        for &index in &unique_routed_nodes {
            let center = node_positions[index];
            for zone in [obstacles[index], lead_zones[index]] {
                x_coordinates.extend([zone.left(), zone.right()]);
                y_coordinates.extend([zone.top(), zone.bottom()]);
            }
            x_coordinates.push(center.x);
            y_coordinates.push(center.y);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Ok(Self {
            node_positions: node_positions.to_vec(),
            routed_nodes: routed_node_mask,
            node_rects,
            obstacles,
            lead_zones,
            obstacle_buckets,
            x_coordinates,
            y_coordinates,
            reserved_port_leads: RouteIndex::new(&[], options.edge_clearance)?,
            port_exit_rays: RouteIndex::new(&[], options.edge_clearance)?,
            options,
        })
    }

    /// Reserve the port leads of every edge. Routes may cross a reserved lead but never run
    /// along it, so an edge routed later always finds its own port entry free.
    pub fn reserve_port_leads(
        &mut self,
        edge_endpoints: &[(usize, usize)],
        edge_ports: &[EdgePorts],
    ) -> RoutingResult<()> {
        if edge_endpoints.len() != edge_ports.len() {
            return Err(RoutingError::InvalidGeometry);
        }
        let mut leads = RouteIndex::new(&[], self.options.edge_clearance)?;
        let mut rays = RouteIndex::new(&[], self.options.edge_clearance)?;
        for (&(source, target), ports) in edge_endpoints.iter().zip(edge_ports) {
            for (node, side, offset) in [
                (source, ports.source_side, ports.source_offset),
                (target, ports.target_side, ports.target_offset),
            ] {
                if node >= self.node_positions.len() {
                    return Err(RoutingError::InvalidNodeIndex);
                }
                if !offset.is_finite() {
                    return Err(RoutingError::InvalidGeometry);
                }
                let port = self.route_port(node, side, offset, self.port_lead());
                leads.insert_route(&[port.card, port.escape])?;
                let ray = self.port_exit_ray(node, port);
                if ray[0] != ray[1] {
                    rays.insert_route(&ray)?;
                }
            }
        }
        self.reserved_port_leads = leads;
        self.port_exit_rays = rays;
        Ok(())
    }

    /// The straight continuation of a port lead up to the next node's lead zone. Other routes
    /// may use it, but prefer equally good alternatives so the port keeps a straight exit.
    fn port_exit_ray(&self, node: usize, port: PortGeometry) -> [Point; 2] {
        let reach = 2.0
            * self
                .options
                .node_size
                .width
                .max(self.options.node_size.height);
        let far = port.escape + port.outward * reach;
        let length = routing_grid_cells(Rect::from_two_points(port.escape, far))
            .filter_map(|cell| self.obstacle_buckets.get(&cell))
            .flatten()
            .filter(|&&other| other != node)
            .filter(|&&other| segment_intersects_rect(port.escape, far, self.lead_zones[other]))
            .map(|&other| {
                let zone = self.lead_zones[other];
                let entry = if port.outward.x > 0.0 {
                    zone.left() - port.escape.x
                } else if port.outward.x < 0.0 {
                    port.escape.x - zone.right()
                } else if port.outward.y > 0.0 {
                    zone.top() - port.escape.y
                } else {
                    port.escape.y - zone.bottom()
                };
                entry.max(0.0)
            })
            .fold(reach, f32::min);
        [port.escape, port.escape + port.outward * length]
    }

    fn port_lead(&self) -> f32 {
        self.options.edge_clearance + ROUTE_PORT_LEAD_EXTRA
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
        self.route_edge_with_track_limit(
            source,
            target,
            edge_ports,
            routed_edge_index,
            self.options.track_limit,
        )
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
            self.port_lead(),
        );
        let target_port = self.route_port(
            target,
            edge_ports.target_side,
            edge_ports.target_offset,
            self.port_lead(),
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
        let crosses_another_node = self.segment_crosses_obstacle(
            line_start,
            line_end,
            Some((source, target)),
            LeadZones::None,
        );
        // Own leads are collinear parts of a straight line, so only the span between them is
        // checked against the leads reserved for other edges.
        let conflicts_with_another_edge = routed_edge_index
            .parallel_conflicts_route(&[line_start, line_end])
            || self
                .reserved_port_leads
                .overlaps_segment(source_port.escape, target_port.escape);
        if source != target && !crosses_another_node && !conflicts_with_another_edge {
            return Ok(vec![line_start, line_end]);
        }
        for lead in [
            [source_port.card, source_port.escape],
            [target_port.escape, target_port.card],
        ] {
            if routed_edge_index.overlaps_segment(lead[0], lead[1]) {
                return Err(RoutingError::NoOrthogonalPath);
            }
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
        x_coordinates.retain(|coordinate| *coordinate >= 0.0);
        y_coordinates.retain(|coordinate| *coordinate >= 0.0);
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);
        let port_direction = |port: PortGeometry| {
            if port.outward.x != 0.0 {
                GRAPH_ROUTE_HORIZONTAL
            } else {
                GRAPH_ROUTE_VERTICAL
            }
        };
        let search_options = RouteSearchOptions {
            segment_index: routed_edge_index,
            reserved_port_leads: &self.reserved_port_leads,
            port_exit_rays: &self.port_exit_rays,
            own_exit_rays: [
                self.port_exit_ray(source, source_port),
                self.port_exit_ray(target, target_port),
            ],
            lead_zones: LeadZones::Endpoints(source, target),
            lane_clearance: self.options.edge_clearance,
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
        x_coordinates.push(x_coordinates.last().copied().unwrap_or(0.0) + track_spacing);
        y_coordinates.push(y_coordinates.last().copied().unwrap_or(0.0) + track_spacing);
        let mut path = self.find_orthogonal_path(
            source_port.escape,
            target_port.escape,
            &x_coordinates,
            &y_coordinates,
            search_options,
        )?;
        if path.is_none() {
            // Tight layouts may leave no corridor outside the endpoint lead bands.
            path = self.find_orthogonal_path(
                source_port.escape,
                target_port.escape,
                &x_coordinates,
                &y_coordinates,
                RouteSearchOptions {
                    lead_zones: LeadZones::None,
                    ..search_options
                },
            )?;
        }
        if path.is_none() {
            add_intermediate_route_tracks(&mut x_coordinates);
            add_intermediate_route_tracks(&mut y_coordinates);
            path = self.find_orthogonal_path(
                source_port.escape,
                target_port.escape,
                &x_coordinates,
                &y_coordinates,
                RouteSearchOptions {
                    lane_clearance: 0.0,
                    lead_zones: LeadZones::None,
                    ..search_options
                },
            )?;
        }
        let route = path
            .map(route_from_path)
            .ok_or(RoutingError::NoOrthogonalPath)?;
        if routed_edge_index
            .first_overlapping_segment(&route, 0.0)
            .is_some()
        {
            return Err(RoutingError::NoOrthogonalPath);
        }
        Ok(route)
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
    for coordinate in coordinates.iter_mut() {
        // `total_cmp` orders -0.0 before 0.0, which would let dedup drop a port coordinate.
        if *coordinate == 0.0 {
            *coordinate = 0.0;
        }
    }
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn add_intermediate_route_tracks(coordinates: &mut Vec<f32>) {
    let tracks = coordinates
        .windows(2)
        .filter_map(|pair| {
            let middle = pair[0] + (pair[1] - pair[0]) / 2.0;
            (middle > pair[0] && middle < pair[1]).then_some(middle)
        })
        .collect::<Vec<_>>();
    coordinates.extend(tracks);
    sort_unique_coordinates(coordinates);
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

/// Score the covered obstacle span so splitting a segment does not change its cost.
pub fn detour_side_preference_penalty(
    start: Point,
    end: Point,
    direct_direction: Vector,
    obstacles: &[Rect],
) -> f32 {
    let mostly_horizontal = direct_direction.x.abs() >= direct_direction.y.abs();
    obstacles
        .iter()
        .map(|obstacle| {
            if mostly_horizontal && start.y == end.y && start.y >= obstacle.bottom() {
                let overlap = (start.x.max(end.x).min(obstacle.right())
                    - start.x.min(end.x).max(obstacle.left()))
                .max(0.0);
                let width = obstacle.right() - obstacle.left();
                if width > 0.0 { overlap / width } else { 0.0 }
            } else if !mostly_horizontal && start.x == end.x && start.x >= obstacle.right() {
                let overlap = (start.y.max(end.y).min(obstacle.bottom())
                    - start.y.min(end.y).max(obstacle.top()))
                .max(0.0);
                let height = obstacle.bottom() - obstacle.top();
                if height > 0.0 { overlap / height } else { 0.0 }
            } else {
                0.0
            }
        })
        .sum::<f32>()
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
