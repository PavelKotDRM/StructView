use super::{
    DEFAULT_EDGE_CLEARANCE, Point, Rect,
    geometry::{cross_product, segments_intersect, segments_within_clearance},
};
use crate::{RoutingError, RoutingResult, orthogonal::geometry::validate_point};
use std::collections::HashMap;

const ROUTE_INDEX_CELL_SIZE: f32 = 128.0;

/// Minimum search threshold used to reject short shared sections.
pub const GRAPH_ROUTE_SHARED_SEGMENT_SEARCH_THRESHOLD: f32 = 8.0;
/// Shared path sections longer than this are locally detoured when possible.
pub const GRAPH_ROUTE_SHARED_SEGMENT_VISIBLE_THRESHOLD: f32 = 32.0;
/// Maximum number of local attempts to separate a shared segment.
pub const GRAPH_ROUTE_SHARED_SEGMENT_DETOUR_LIMIT: usize = 16;
/// Penalty for routes closer than the configured edge clearance.
pub const GRAPH_EDGE_ROUTE_PENALTY: f32 = 2_000.0;
/// Penalty for crossing another route.
pub const GRAPH_EDGE_CROSSING_PENALTY: f32 = 50_000.0;
/// Linear factor for collinear overlap beyond the search threshold.
pub const GRAPH_EDGE_SHARED_SEGMENT_PENALTY_SCALE: f32 = 10_000.0;
/// Minimum penalty for a shared segment.
pub const GRAPH_EDGE_SHARED_SEGMENT_PENALTY_MINIMUM: f32 = 8_000.0;
/// Maximum penalty for a shared segment.
pub const GRAPH_EDGE_SHARED_SEGMENT_PENALTY_LIMIT: f32 = 5_000_000.0;

/// A spatial index of already-routed segments.
///
/// Use `insert_route` after accepting a route, then pass the index to
/// [`super::OrthogonalRouter::route_edge`] to steer later routes away from it.
pub struct RouteIndex {
    segments: Vec<[Point; 2]>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
    edge_clearance: f32,
}

impl RouteIndex {
    /// Build an index containing the supplied routes.
    pub fn new(routes: &[Vec<Point>], edge_clearance: f32) -> RoutingResult<Self> {
        if !edge_clearance.is_finite() || edge_clearance < 0.0 {
            return Err(RoutingError::InvalidGeometry);
        }
        let mut index = Self {
            segments: Vec::new(),
            buckets: HashMap::new(),
            edge_clearance,
        };
        for route in routes {
            index.insert_route(route)?;
        }
        Ok(index)
    }

    /// Add all segments in an accepted route to the index.
    pub fn insert_route(&mut self, route: &[Point]) -> RoutingResult<()> {
        for &point in route {
            validate_point(point)?;
        }
        for segment in route.windows(2) {
            let index = self.segments.len();
            let segment = [segment[0], segment[1]];
            for cell in Self::cells(segment[0], segment[1], self.edge_clearance) {
                self.buckets.entry(cell).or_default().push(index);
            }
            self.segments.push(segment);
        }
        Ok(())
    }

    /// Whether any part of `route` conflicts with an indexed route.
    pub fn conflicts_route(&self, route: &[Point]) -> bool {
        route
            .windows(2)
            .any(|segment| self.conflicts_segment(segment[0], segment[1], self.edge_clearance))
    }

    /// Whether the segment is within `clearance` of an indexed segment.
    pub fn conflicts_segment(&self, start: Point, end: Point, clearance: f32) -> bool {
        let padding = if clearance.is_finite() && clearance >= 0.0 {
            clearance.max(self.edge_clearance)
        } else {
            return true;
        };
        self.indices_near(start, end, padding)
            .into_iter()
            .any(|index| {
                segments_within_clearance(
                    start,
                    end,
                    self.segments[index][0],
                    self.segments[index][1],
                    clearance,
                )
            })
    }

    /// Whether `route` intersects any indexed route.
    pub fn intersects_route(&self, route: &[Point]) -> bool {
        route
            .windows(2)
            .any(|segment| self.intersects_segment(segment[0], segment[1]))
    }

    /// Whether a segment intersects an indexed segment.
    pub fn intersects_segment(&self, start: Point, end: Point) -> bool {
        self.indices_near(start, end, self.edge_clearance)
            .into_iter()
            .any(|index| {
                segments_intersect(start, end, self.segments[index][0], self.segments[index][1])
                    || collinear_segments_overlap(
                        start,
                        end,
                        self.segments[index][0],
                        self.segments[index][1],
                    )
            })
    }

    /// Find the first collinear shared section longer than `minimum_overlap`.
    pub fn first_overlapping_segment(
        &self,
        route: &[Point],
        minimum_overlap: f32,
    ) -> Option<(usize, [Point; 2], f32)> {
        for (route_segment_index, segment) in route.windows(2).enumerate() {
            if let Some((shared_segment, overlap_length)) = self
                .indices_near(segment[0], segment[1], self.edge_clearance)
                .into_iter()
                .find_map(|index| {
                    let overlap = collinear_segment_overlap(
                        segment[0],
                        segment[1],
                        self.segments[index][0],
                        self.segments[index][1],
                    )?;
                    (overlap.1 > minimum_overlap).then_some(overlap)
                })
            {
                return Some((route_segment_index, shared_segment, overlap_length));
            }
        }
        None
    }

    /// Whether a candidate segment shares any positive-length collinear section.
    pub fn overlaps_segment(&self, start: Point, end: Point) -> bool {
        self.indices_near(start, end, self.edge_clearance)
            .into_iter()
            .any(|index| {
                collinear_segments_overlap(
                    start,
                    end,
                    self.segments[index][0],
                    self.segments[index][1],
                )
            })
    }

    /// Cost of routing a segment near, across, or along existing routes.
    pub fn penalty(&self, start: Point, end: Point) -> f32 {
        self.indices_near(start, end, self.edge_clearance)
            .into_iter()
            .map(|index| {
                segment_pair_penalty_with_clearance(
                    start,
                    end,
                    self.segments[index],
                    self.edge_clearance,
                )
            })
            .sum()
    }

    /// Return indexed segments near the supplied rectangle.
    pub fn segments_near(&self, bounds: Rect) -> Vec<[Point; 2]> {
        let bounds = bounds.expand(self.edge_clearance);
        let left = (bounds.left() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let right = (bounds.right() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let top = (bounds.top() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let bottom = (bounds.bottom() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let mut indices = (top..=bottom)
            .flat_map(|row| (left..=right).map(move |column| (column, row)))
            .filter_map(|cell| self.buckets.get(&cell))
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .map(|index| self.segments[index])
            .collect()
    }

    fn cells(start: Point, end: Point, padding: f32) -> impl Iterator<Item = (i32, i32)> {
        let bounds = Rect::from_two_points(start, end).expand(padding);
        let left = (bounds.left() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let right = (bounds.right() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let top = (bounds.top() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let bottom = (bounds.bottom() / ROUTE_INDEX_CELL_SIZE).floor() as i32;
        (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
    }

    fn indices_near(&self, start: Point, end: Point, padding: f32) -> Vec<usize> {
        let mut indices = Self::cells(start, end, padding)
            .filter_map(|cell| self.buckets.get(&cell))
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
    }
}

/// Penalty for a route segment relative to one existing segment.
pub fn segment_pair_penalty(start: Point, end: Point, segment: [Point; 2]) -> f32 {
    segment_pair_penalty_with_clearance(start, end, segment, DEFAULT_EDGE_CLEARANCE)
}

fn segment_pair_penalty_with_clearance(
    start: Point,
    end: Point,
    segment: [Point; 2],
    edge_clearance: f32,
) -> f32 {
    if let Some((_, overlap_length)) = collinear_segment_overlap(start, end, segment[0], segment[1])
    {
        let visible_overlap =
            (overlap_length - GRAPH_ROUTE_SHARED_SEGMENT_SEARCH_THRESHOLD).max(0.0);
        (GRAPH_EDGE_SHARED_SEGMENT_PENALTY_MINIMUM
            + visible_overlap * GRAPH_EDGE_SHARED_SEGMENT_PENALTY_SCALE)
            .min(GRAPH_EDGE_SHARED_SEGMENT_PENALTY_LIMIT)
    } else if segments_intersect(start, end, segment[0], segment[1])
        || segments_within_clearance(start, end, segment[0], segment[1], 1.0)
    {
        GRAPH_EDGE_CROSSING_PENALTY
    } else if segments_within_clearance(start, end, segment[0], segment[1], edge_clearance) {
        GRAPH_EDGE_ROUTE_PENALTY
    } else {
        0.0
    }
}

fn collinear_segment_overlap(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> Option<([Point; 2], f32)> {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let first_length = first_direction.length();
    let second_length = second_direction.length();
    if first_length <= f32::EPSILON || second_length <= f32::EPSILON {
        return None;
    }

    if cross_product(first_direction, second_direction).abs()
        > first_length * second_length * 0.00001
        || cross_product(first_direction, second_start - first_start).abs() / first_length > 0.01
        || cross_product(first_direction, second_end - first_start).abs() / first_length > 0.01
    {
        return None;
    }

    let direction = first_direction / first_length;
    let second_start_offset = (second_start - first_start).dot(direction);
    let second_end_offset = (second_end - first_start).dot(direction);
    let overlap_start = 0.0_f32.max(second_start_offset.min(second_end_offset));
    let overlap_end = first_length.min(second_start_offset.max(second_end_offset));
    let overlap_length = overlap_end - overlap_start;
    (overlap_length > 0.01).then(|| {
        (
            [
                first_start + direction * overlap_start,
                first_start + direction * overlap_end,
            ],
            overlap_length,
        )
    })
}

fn collinear_segments_overlap(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> bool {
    collinear_segment_overlap(first_start, first_end, second_start, second_end).is_some()
}
