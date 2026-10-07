use super::*;
#[cfg(test)]
use struct_view_routing::orthogonal::segment_pair_penalty as route_segment_pair_penalty;
use struct_view_routing::orthogonal::{
    Point as RoutePoint, Rect as RouteRect, RouteIndex,
    closest_point_on_segment as route_closest_point_on_segment,
    point_to_segment_distance as route_point_to_segment_distance,
    segment_intersects_rect as route_segment_intersects_rect,
    segments_intersect as route_segments_intersect,
    segments_within_clearance as route_segments_within_clearance,
};
#[cfg(test)]
use struct_view_routing::orthogonal::{
    Vector as RouteVector, detour_side_preference_penalty as route_detour_side_preference_penalty,
};

pub(in crate::app::views) struct GraphRouteSegmentIndex {
    pub(super) inner: RouteIndex,
}

impl GraphRouteSegmentIndex {
    fn route_points(routes: &[Vec<Pos2>]) -> Vec<Vec<RoutePoint>> {
        routes
            .iter()
            .map(|route| route.iter().copied().map(route_point).collect())
            .collect()
    }

    pub(in crate::app::views) fn new(routes: &[Vec<Pos2>]) -> Self {
        Self {
            inner: RouteIndex::new(&Self::route_points(routes), GRAPH_EDGE_CLEARANCE)
                .expect("graph route geometry must be finite"),
        }
    }

    pub(super) fn insert_route(&mut self, route: &[Pos2]) {
        self.inner
            .insert_route(&route.iter().copied().map(route_point).collect::<Vec<_>>())
            .expect("graph route geometry must be finite");
    }

    pub(super) fn conflicts_route(&self, route: &[Pos2]) -> bool {
        self.inner
            .conflicts_route(&route.iter().copied().map(route_point).collect::<Vec<_>>())
    }

    #[cfg(test)]
    pub(super) fn first_overlapping_segment(
        &self,
        route: &[Pos2],
        minimum_overlap: f32,
    ) -> Option<(usize, [Pos2; 2], f32)> {
        let (index, shared, length) = self.inner.first_overlapping_segment(
            &route.iter().copied().map(route_point).collect::<Vec<_>>(),
            minimum_overlap,
        )?;
        Some((index, [ui_point(shared[0]), ui_point(shared[1])], length))
    }

    #[cfg(test)]
    pub(super) fn overlaps_segment(&self, start: Pos2, end: Pos2) -> bool {
        self.inner
            .overlaps_segment(route_point(start), route_point(end))
    }

    #[cfg(test)]
    pub(super) fn penalty(&self, start: Pos2, end: Pos2) -> f32 {
        self.inner.penalty(route_point(start), route_point(end))
    }
}

pub(in crate::app::views::graph) fn segment_intersects_rect(
    start: Pos2,
    end: Pos2,
    rect: egui::Rect,
) -> bool {
    route_segment_intersects_rect(route_point(start), route_point(end), route_rect(rect))
}

pub(in crate::app::views::graph) fn segments_intersect(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
) -> bool {
    route_segments_intersect(
        route_point(first_start),
        route_point(first_end),
        route_point(second_start),
        route_point(second_end),
    )
}

pub(in crate::app::views::graph) fn point_to_segment_distance(
    point: Pos2,
    start: Pos2,
    end: Pos2,
) -> f32 {
    route_point_to_segment_distance(route_point(point), route_point(start), route_point(end))
}

pub(in crate::app::views::graph) fn closest_point_on_segment(
    point: Pos2,
    start: Pos2,
    end: Pos2,
) -> Pos2 {
    ui_point(route_closest_point_on_segment(
        route_point(point),
        route_point(start),
        route_point(end),
    ))
}

pub(in crate::app::views) fn segments_within_clearance(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
    clearance: f32,
) -> bool {
    route_segments_within_clearance(
        route_point(first_start),
        route_point(first_end),
        route_point(second_start),
        route_point(second_end),
        clearance,
    )
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

pub(in crate::app::views::graph) const GRAPH_EDGE_OUTLINE_WIDTH: f32 = 1.0;

pub(in crate::app::views::graph) fn draw_arrow_head(
    painter: &egui::Painter,
    tip: Pos2,
    direction: Vec2,
    stroke: Stroke,
    outline_color: Color32,
    zoom: f32,
) {
    let wings = arrow_head_wings(tip, direction, zoom);
    let outline = Stroke::new(stroke.width + 2.0 * GRAPH_EDGE_OUTLINE_WIDTH, outline_color);
    for wing in wings {
        painter.line_segment([tip, wing], outline);
    }
    for wing in wings {
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

#[cfg(test)]
pub(super) fn segment_pair_penalty(start: Pos2, end: Pos2, segment: [Pos2; 2]) -> f32 {
    route_segment_pair_penalty(
        route_point(start),
        route_point(end),
        [route_point(segment[0]), route_point(segment[1])],
    )
}

pub(super) fn route_point(point: Pos2) -> RoutePoint {
    RoutePoint::new(point.x, point.y)
}

pub(super) fn ui_point(point: RoutePoint) -> Pos2 {
    Pos2::new(point.x, point.y)
}

fn route_rect(rect: egui::Rect) -> RouteRect {
    RouteRect::from_min_max(route_point(rect.min), route_point(rect.max))
}

#[cfg(test)]
pub(super) fn detour_side_preference_penalty(
    start: Pos2,
    end: Pos2,
    direct_direction: Vec2,
    obstacles: &[egui::Rect],
) -> f32 {
    route_detour_side_preference_penalty(
        route_point(start),
        route_point(end),
        route_vector(direct_direction),
        &obstacles
            .iter()
            .copied()
            .map(route_rect)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
fn route_vector(vector: Vec2) -> RouteVector {
    RouteVector::new(vector.x, vector.y)
}
