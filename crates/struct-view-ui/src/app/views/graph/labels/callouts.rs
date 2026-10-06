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
    ))
}

pub(in crate::app::views::graph) fn graph_edge_label_callout_at(
    label: &str,
    points: &[Pos2],
    position: Pos2,
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let text = shorten_graph_edge_label(label, 144.0);
    let size = Vec2::new(
        text.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let background = aligned_label_rect(position, size, Align2::LEFT_CENTER).expand(3.0);
    Some(graph_edge_label_layout(
        text,
        position,
        Align2::LEFT_CENTER,
        background,
        points,
    ))
}

pub(in crate::app::views::graph) fn graph_edge_label_callout_text(
    text: String,
    points: &[Pos2],
    canvas: egui::Rect,
    occupied_label_rects: &[egui::Rect],
) -> GraphEdgeLabelLayout {
    let size = Vec2::new(
        text.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    // A reserved column beyond all routes guarantees a visible label even in dense graphs.
    let mut position = Pos2::new(
        canvas.right() + 24.0,
        points[points.len() / 2].y.max(size.y / 2.0 + 24.0),
    );
    loop {
        let background = aligned_label_rect(position, size, Align2::LEFT_CENTER).expand(3.0);
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
    canvas: egui::Rect,
    node_rects: &[egui::Rect],
    occupied_label_rects: &[egui::Rect],
    routed_edges: &[Vec<Pos2>],
) -> Option<egui::Rect> {
    if label.is_empty() {
        return None;
    }

    let size = Vec2::new(
        label.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let label_rect = aligned_label_rect(position, size, alignment).expand(3.0);
    if label_rect.left() < canvas.left()
        || label_rect.right() > canvas.right()
        || label_rect.top() < canvas.top()
        || label_rect.bottom() > canvas.bottom()
        || node_rects
            .iter()
            .any(|rect| label_rect.intersects(rect.expand(2.0)))
        || occupied_label_rects
            .iter()
            .any(|rect| label_rect.intersects(*rect))
        || routed_edges.iter().any(|route| {
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
