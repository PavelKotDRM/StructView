use super::super::*;
use super::placement::aligned_label_rect;
use super::placement::shorten_graph_edge_label;

#[cfg(test)]
pub(in crate::app::views::graph) fn graph_edge_label_callout(
    label: &str,
    points: &[Pos2],
    canvas: egui::Rect,
    occupied_label_rects: &[egui::Rect],
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let text = shorten_graph_edge_label(label, 144.0);
    Some(graph_edge_label_callout_text(
        text,
        points,
        canvas,
        occupied_label_rects,
        8.0,
    ))
}

pub(in crate::app::views::graph) fn graph_edge_label_callout_at(
    label: &str,
    points: &[Pos2],
    position: Pos2,
    reference_radius: f32,
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let text = shorten_graph_edge_label(label, 144.0);
    let size = Vec2::new(
        text.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let background =
        aligned_label_rect(position, size, Align2::LEFT_CENTER, reference_radius).expand(3.0);
    Some(graph_edge_label_callout_layout(
        text,
        points,
        background,
        reference_radius,
    ))
}

pub(in crate::app::views::graph) fn graph_edge_label_callout_layout(
    text: String,
    points: &[Pos2],
    background: egui::Rect,
    reference_radius: f32,
) -> GraphEdgeLabelLayout {
    let marker = Pos2::new(
        background.left() + 3.0 + reference_radius,
        background.center().y,
    );
    let position = marker + Vec2::new(reference_radius + 8.0, 0.0);
    let mut label =
        graph_edge_label_layout(text, position, Align2::LEFT_CENTER, background, points);
    label.callout_marker = Some(marker);
    label
}

#[cfg(test)]
fn graph_edge_label_callout_text(
    text: String,
    points: &[Pos2],
    canvas: egui::Rect,
    occupied_label_rects: &[egui::Rect],
    reference_radius: f32,
) -> GraphEdgeLabelLayout {
    let size = Vec2::new(
        text.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT.max(reference_radius * 2.0),
    );
    // A reserved column beyond all routes guarantees a visible label even in dense graphs.
    let mut position = Pos2::new(
        canvas.right() + 24.0,
        points[points.len() / 2].y.max(size.y / 2.0 + 24.0),
    );
    loop {
        let background =
            aligned_label_rect(position, size, Align2::LEFT_CENTER, reference_radius).expand(3.0);
        let bottom = occupied_label_rects
            .iter()
            .filter(|rect| background.intersects(rect.expand(2.0)))
            .map(egui::Rect::bottom)
            .max_by(f32::total_cmp);
        if let Some(bottom) = bottom {
            position.y = bottom + size.y / 2.0 + 9.0;
        } else {
            return graph_edge_label_layout(
                text,
                position,
                Align2::LEFT_CENTER,
                background,
                points,
            );
        }
    }
}

pub(in crate::app::views::graph) fn place_edge_label(
    label: &str,
    position: Pos2,
    alignment: Align2,
    context: &EdgeLabelPlacementContext<'_>,
) -> Option<egui::Rect> {
    if label.is_empty() {
        return None;
    }

    let size = Vec2::new(
        label.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let label_rect =
        aligned_label_rect(position, size, alignment, context.reference_radius).expand(3.0);
    if label_rect.left() < context.canvas.left()
        || label_rect.right() > context.canvas.right()
        || label_rect.top() < context.canvas.top()
        || label_rect.bottom() > context.canvas.bottom()
        || context
            .node_rects
            .iter()
            .any(|rect| label_rect.intersects(rect.expand(2.0)))
        || context
            .occupied_label_rects
            .iter()
            .any(|rect| label_rect.intersects(*rect))
        || context.routed_edges.iter().any(|route| {
            route.windows(2).any(|segment| {
                segment_intersects_rect(segment[0], segment[1], label_rect.expand(2.0))
            }) || route.first().into_iter().chain(route.last()).any(|tip| {
                label_rect.intersects(egui::Rect::from_center_size(*tip, Vec2::splat(20.0)))
            })
        })
    {
        return None;
    }
    Some(label_rect)
}

pub(in crate::app::views::graph) struct EdgeLabelPlacementContext<'a> {
    pub(in crate::app::views::graph) canvas: egui::Rect,
    pub(in crate::app::views::graph) node_rects: &'a [egui::Rect],
    pub(in crate::app::views::graph) occupied_label_rects: &'a [egui::Rect],
    pub(in crate::app::views::graph) routed_edges: &'a [Vec<Pos2>],
    pub(in crate::app::views::graph) reference_radius: f32,
}
