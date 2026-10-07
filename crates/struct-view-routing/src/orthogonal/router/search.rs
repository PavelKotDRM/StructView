use super::*;
use crate::{IndexedNeighbor, search::a_star_indexed_with_backend};

impl OrthogonalRouter {
    pub(super) fn segment_crosses_obstacle(
        &self,
        start: Point,
        end: Point,
        excluded_nodes: Option<(usize, usize)>,
    ) -> bool {
        routing_grid_cells(Rect::from_two_points(start, end)).any(|cell| {
            self.obstacle_buckets.get(&cell).is_some_and(|indices| {
                indices.iter().any(|&index| {
                    !excluded_nodes
                        .is_some_and(|(source, target)| index == source || index == target)
                        && segment_intersects_rect(start, end, self.obstacles[index])
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
            preferences,
            conflict_clearance: route_conflict_clearance,
            start_direction,
            goal_direction,
        } = options;
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        // A virtual sink charges for the final turn into the target port.
        let goal_state = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let state_count = goal_state + 1;
        let mut segment_costs = HashMap::new();
        let goal_vertex = Self::vertex_index(target, x_coordinates, y_coordinates);
        let start_vertex = Self::vertex_index(source, x_coordinates, y_coordinates);
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
                        cost: f64::from(if direction_before != goal_direction {
                            GRAPH_ROUTE_TURN_PENALTY
                        } else {
                            0.0
                        }),
                    });
                    return;
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
                    let near_route_port = [source, target].into_iter().any(|port| {
                        let exemption = self.options.edge_clearance + 4.0;
                        current_point.distance(port) <= exemption
                            && next_point.distance(port) <= exemption
                    });
                    if self.segment_crosses_obstacle(current_point, next_point, None)
                        || route_conflict_clearance.is_some_and(|clearance| {
                            !near_route_port
                                && segment_index.conflicts_segment(
                                    current_point,
                                    next_point,
                                    clearance,
                                )
                        })
                    {
                        continue;
                    }
                    let edge_key = (
                        current_vertex.min(next_vertex),
                        current_vertex.max(next_vertex),
                    );
                    let edge_overlap_penalty = *segment_costs
                        .entry(edge_key)
                        .or_insert_with(|| segment_index.penalty(current_point, next_point));

                    let turn_penalty = if direction_before != direction {
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
                    let cost = segment_length
                        + turn_penalty
                        + edge_overlap_penalty
                        + side_preference_penalty;
                    outgoing.push(IndexedNeighbor {
                        node: next_vertex * GRAPH_ROUTE_DIRECTIONS + direction,
                        cost: f64::from(cost),
                    });
                }
            },
            |state| {
                if state == goal_state {
                    return 0.0;
                }
                let vertex = state / GRAPH_ROUTE_DIRECTIONS;
                f64::from(route_heuristic(
                    Self::point_at(vertex, x_coordinates, y_coordinates),
                    &[target],
                ))
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

    fn vertex_index(point: Point, x_coordinates: &[f32], y_coordinates: &[f32]) -> usize {
        let column = x_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.x))
            .expect("graph route x coordinates must include every port");
        let row = y_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.y))
            .expect("graph route y coordinates must include every port");
        row * x_coordinates.len() + column
    }

    fn point_at(vertex: usize, x_coordinates: &[f32], y_coordinates: &[f32]) -> Point {
        let width = x_coordinates.len();
        Point::new(x_coordinates[vertex % width], y_coordinates[vertex / width])
    }
}
