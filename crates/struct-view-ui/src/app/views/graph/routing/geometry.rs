use super::*;

pub(in crate::app::views::graph) fn segment_intersects_rect(
    start: Pos2,
    end: Pos2,
    rect: egui::Rect,
) -> bool {
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
        let left = (bounds.left() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let right = (bounds.right() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let top = (bounds.top() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let bottom = (bounds.bottom() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
    }

    pub(in crate::app::views) fn new(routes: &[Vec<Pos2>]) -> Self {
        let mut index = Self {
            segments: Vec::new(),
            buckets: HashMap::new(),
        };
        for route in routes {
            index.insert_route(route);
        }
        index
    }

    pub(super) fn insert_route(&mut self, route: &[Pos2]) {
        for segment in route.windows(2) {
            let index = self.segments.len();
            let segment = [segment[0], segment[1]];
            for cell in Self::cells(segment[0], segment[1]) {
                self.buckets.entry(cell).or_default().push(index);
            }
            self.segments.push(segment);
        }
    }

    pub(super) fn conflicts_route(&self, route: &[Pos2]) -> bool {
        route.windows(2).any(|segment| {
            let mut indices = Self::cells(segment[0], segment[1])
                .filter_map(|cell| self.buckets.get(&cell))
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            indices.into_iter().any(|index| {
                segments_within_clearance(
                    segment[0],
                    segment[1],
                    self.segments[index][0],
                    self.segments[index][1],
                    GRAPH_EDGE_CLEARANCE,
                )
            })
        })
    }

    pub(super) fn penalty(&self, start: Pos2, end: Pos2) -> f32 {
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

    pub(super) fn segments_near(&self, bounds: egui::Rect) -> Vec<[Pos2; 2]> {
        let bounds = bounds.expand(GRAPH_EDGE_CLEARANCE);
        let left = (bounds.left() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let right = (bounds.right() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let top = (bounds.top() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
        let bottom = (bounds.bottom() / GRAPH_ROUTE_INDEX_CELL_SIZE).floor() as i32;
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
}

pub(in crate::app::views::graph) fn segment_pair_penalty(
    start: Pos2,
    end: Pos2,
    segment: [Pos2; 2],
) -> f32 {
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

pub(in crate::app::views::graph) fn segments_intersect(
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

pub(in crate::app::views::graph) fn cross_product(first: Vec2, second: Vec2) -> f32 {
    first.x * second.y - first.y * second.x
}

pub(in crate::app::views::graph) fn point_to_segment_distance(
    point: Pos2,
    start: Pos2,
    end: Pos2,
) -> f32 {
    point.distance(closest_point_on_segment(point, start, end))
}

pub(in crate::app::views::graph) fn closest_point_on_segment(
    point: Pos2,
    start: Pos2,
    end: Pos2,
) -> Pos2 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return start;
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    start + segment * projection
}

#[cfg(test)]
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

pub(in crate::app::views::graph) fn draw_arrow_head(
    painter: &egui::Painter,
    tip: Pos2,
    direction: Vec2,
    stroke: Stroke,
    zoom: f32,
) {
    for wing in arrow_head_wings(tip, direction, zoom) {
        painter.line_segment([tip, wing], stroke);
    }
}

pub(in crate::app::views::graph) fn arrow_head_wings(
    tip: Pos2,
    direction: Vec2,
    zoom: f32,
) -> [Pos2; 2] {
    [2.55, -2.55].map(|angle| tip + Vec2::angled(direction.angle() + angle) * (9.0 * zoom))
}

pub(in crate::app::views::graph) fn edge_arrowheads(
    route: &[Pos2],
    direction: EdgeDirection,
) -> Vec<(Pos2, Vec2)> {
    let mut arrows = Vec::with_capacity(2);
    if direction.arrow_at_source()
        && let Some(segment) = route.windows(2).next()
    {
        arrows.push((segment[0], (segment[0] - segment[1]).normalized()));
    }
    if direction.arrow_at_target()
        && let Some(segment) = route.windows(2).last()
    {
        arrows.push((segment[1], (segment[1] - segment[0]).normalized()));
    }
    arrows
}
