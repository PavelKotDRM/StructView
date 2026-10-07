use super::*;
use crate::{IndexedNeighbor, search::a_star_indexed_with_backend};

impl OrthogonalRouter {
    pub(super) fn segment_crosses_obstacle(
        &self,
        start: Point,
        end: Point,
        excluded_nodes: Option<(usize, usize)>,
        lead_zones: LeadZones,
    ) -> bool {
        routing_grid_cells(Rect::from_two_points(start, end)).any(|cell| {
            self.obstacle_buckets.get(&cell).is_some_and(|indices| {
                indices.iter().any(|&index| {
                    let in_lead_zone = match lead_zones {
                        LeadZones::Endpoints(source, target) => index == source || index == target,
                        LeadZones::None => false,
                    };
                    let zone = if in_lead_zone {
                        self.lead_zones[index]
                    } else {
                        self.obstacles[index]
                    };
                    !excluded_nodes
                        .is_some_and(|(source, target)| index == source || index == target)
                        && segment_intersects_rect(start, end, zone)
                })
            })
        })
    }

    pub(super) fn find_orthogonal_path(
        &self,
        source: Point,
        target: Point,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
        options: RouteSearchOptions<'_>,
    ) -> RoutingResult<Option<Vec<Point>>> {
        let RouteSearchOptions {
            segment_index,
            reserved_port_leads,
            port_exit_rays,
            own_exit_rays,
            lead_zones,
            lane_clearance,
            start_direction,
            goal_direction,
        } = options;
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        // A virtual sink charges for the final turn into the target port.
        let goal_state = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let state_count = goal_state + 1;
        let (Some(goal_vertex), Some(start_vertex)) = (
            Self::vertex_index(target, x_coordinates, y_coordinates),
            Self::vertex_index(source, x_coordinates, y_coordinates),
        ) else {
            return Err(RoutingError::InvalidGeometry);
        };
        // `None` marks a blocked segment; otherwise the extra tie-breaking cost.
        let mut segment_costs = HashMap::new();
        let mut neighbors = |vertex| {
            let current_point = Self::point_at(vertex, x_coordinates, y_coordinates);
            Self::grid_neighbors(vertex, x_coordinates, y_coordinates)
                .into_iter()
                .flatten()
                .filter_map(|(next_vertex, direction, length)| {
                    let key = (vertex.min(next_vertex), vertex.max(next_vertex));
                    let extra = *segment_costs.entry(key).or_insert_with(|| {
                        let next_point = Self::point_at(next_vertex, x_coordinates, y_coordinates);
                        let near_route_port = [source, target].into_iter().any(|port| {
                            let exemption = self.options.edge_clearance + 4.0;
                            current_point.distance(port) <= exemption
                                && next_point.distance(port) <= exemption
                        });
                        let blocked = self.segment_crosses_obstacle(
                            current_point,
                            next_point,
                            None,
                            lead_zones,
                        ) || reserved_port_leads
                            .overlaps_segment(current_point, next_point)
                            || segment_index.parallel_conflicts_segment(
                                current_point,
                                next_point,
                                if near_route_port { 0.0 } else { lane_clearance },
                            );
                        (!blocked).then(|| {
                            f64::from(port_exit_rays.collinear_overlap_length(
                                current_point,
                                next_point,
                                &own_exit_rays,
                            )) * PORT_EXIT_RAY_COST
                        })
                    });
                    extra.map(|extra| (next_vertex, direction, length + extra))
                })
                .collect::<Vec<_>>()
        };
        let bend_cost = f64::from(GRAPH_ROUTE_BEND_COST);
        let goal_point = target;
        // Port lead directions make endpoint bends part of the search cost.
        let start_state = start_vertex * GRAPH_ROUTE_DIRECTIONS + start_direction;
        let path = a_star_indexed_with_backend(
            state_count,
            start_state,
            self.options.search_backend,
            |state| state == goal_state,
            |state, outgoing| {
                if state == goal_state {
                    return;
                }
                let current_vertex = state / GRAPH_ROUTE_DIRECTIONS;
                let direction_before = state % GRAPH_ROUTE_DIRECTIONS;
                if current_vertex == goal_vertex {
                    outgoing.push(IndexedNeighbor {
                        node: goal_state,
                        cost: if direction_before != goal_direction {
                            bend_cost
                        } else {
                            0.0
                        },
                    });
                    return;
                }
                for (next_vertex, direction, segment_length) in neighbors(current_vertex) {
                    let turn_cost = if direction_before != direction {
                        bend_cost
                    } else {
                        0.0
                    };
                    outgoing.push(IndexedNeighbor {
                        node: next_vertex * GRAPH_ROUTE_DIRECTIONS + direction,
                        cost: segment_length + turn_cost,
                    });
                }
            },
            |state| {
                if state == goal_state {
                    return 0.0;
                }
                let point =
                    Self::point_at(state / GRAPH_ROUTE_DIRECTIONS, x_coordinates, y_coordinates);
                let offset = goal_point - point;
                let turns = if offset.x != 0.0 && offset.y != 0.0 {
                    bend_cost
                } else {
                    0.0
                };
                f64::from(offset.x.abs() + offset.y.abs()) + turns
            },
        )?;

        let Some(path) = path else {
            return Ok(None);
        };
        Ok(Some(
            path.nodes
                .into_iter()
                .filter(|&state| state != goal_state)
                .map(|state| {
                    Self::point_at(state / GRAPH_ROUTE_DIRECTIONS, x_coordinates, y_coordinates)
                })
                .collect(),
        ))
    }

    fn grid_neighbors(
        vertex: usize,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
    ) -> [Option<(usize, usize, f64)>; 4] {
        let width = x_coordinates.len();
        let column = vertex % width;
        let row = vertex / width;
        let mut neighbors = [None; 4];
        if column > 0 {
            neighbors[0] = Some((
                vertex - 1,
                GRAPH_ROUTE_HORIZONTAL,
                f64::from(x_coordinates[column]) - f64::from(x_coordinates[column - 1]),
            ));
        }
        if column + 1 < width {
            neighbors[1] = Some((
                vertex + 1,
                GRAPH_ROUTE_HORIZONTAL,
                f64::from(x_coordinates[column + 1]) - f64::from(x_coordinates[column]),
            ));
        }
        if row > 0 {
            neighbors[2] = Some((
                vertex - width,
                GRAPH_ROUTE_VERTICAL,
                f64::from(y_coordinates[row]) - f64::from(y_coordinates[row - 1]),
            ));
        }
        if row + 1 < y_coordinates.len() {
            neighbors[3] = Some((
                vertex + width,
                GRAPH_ROUTE_VERTICAL,
                f64::from(y_coordinates[row + 1]) - f64::from(y_coordinates[row]),
            ));
        }
        neighbors
    }

    fn vertex_index(point: Point, x_coordinates: &[f32], y_coordinates: &[f32]) -> Option<usize> {
        let find = |coordinates: &[f32], value: f32| {
            coordinates
                .binary_search_by(|coordinate| {
                    coordinate
                        .partial_cmp(&value)
                        .unwrap_or(std::cmp::Ordering::Less)
                })
                .ok()
        };
        let column = find(x_coordinates, point.x)?;
        let row = find(y_coordinates, point.y)?;
        Some(row * x_coordinates.len() + column)
    }

    fn point_at(vertex: usize, x_coordinates: &[f32], y_coordinates: &[f32]) -> Point {
        let width = x_coordinates.len();
        Point::new(x_coordinates[vertex % width], y_coordinates[vertex / width])
    }
}
