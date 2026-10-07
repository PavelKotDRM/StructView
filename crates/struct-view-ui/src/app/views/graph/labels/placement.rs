use super::super::*;

pub(in crate::app::views::graph) struct GraphEdgeLabelLayout {
    pub(in crate::app::views::graph) text: String,
    pub(in crate::app::views::graph) position: Pos2,
    pub(in crate::app::views::graph) alignment: Align2,
    pub(in crate::app::views::graph) background: egui::Rect,
    pub(in crate::app::views::graph) leader: Option<[Pos2; 2]>,
    pub(in crate::app::views::graph) reference: Option<(usize, Pos2)>,
}

fn edge_label_placement(points: &[Pos2], node_rects: &[egui::Rect]) -> (Pos2, Align2, f32) {
    if points.len() == 2 {
        let line_start = points[0];
        let line_end = points[1];
        return (
            Pos2::new(
                (line_start.x + line_end.x) / 2.0,
                (line_start.y + line_end.y) / 2.0 - 8.0,
            ),
            Align2::CENTER_CENTER,
            (line_end - line_start).length() - 8.0,
        );
    }

    let longest_horizontal = points
        .windows(2)
        .filter(|segment| segment[0].y == segment[1].y)
        .max_by(|left, right| {
            (left[0].x - left[1].x)
                .abs()
                .total_cmp(&(right[0].x - right[1].x).abs())
        });
    if let Some(segment) = longest_horizontal
        && (segment[0].x - segment[1].x).abs() >= 32.0
    {
        return (
            Pos2::new((segment[0].x + segment[1].x) / 2.0, segment[0].y - 8.0),
            Align2::CENTER_CENTER,
            (segment[0].x - segment[1].x).abs() - 8.0,
        );
    }

    let segment = points
        .windows(2)
        .filter(|segment| segment[0].x == segment[1].x)
        .max_by(|left, right| {
            (left[0].y - left[1].y)
                .abs()
                .total_cmp(&(right[0].y - right[1].y).abs())
        })
        .expect("a detoured graph route must contain a vertical segment");
    let midpoint = (segment[0].y + segment[1].y) / 2.0;
    let left_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.right() <= segment[0].x
        })
        .map(|rect| segment[0].x - rect.right())
        .fold(f32::INFINITY, f32::min);
    let right_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.left() >= segment[0].x
        })
        .map(|rect| rect.left() - segment[0].x)
        .fold(f32::INFINITY, f32::min);
    let (position, alignment, available_space) = if right_space >= left_space {
        (
            Pos2::new(segment[0].x + 6.0, midpoint),
            Align2::LEFT_CENTER,
            right_space,
        )
    } else {
        (
            Pos2::new(segment[0].x - 6.0, midpoint),
            Align2::RIGHT_CENTER,
            left_space,
        )
    };
    let width = (available_space - 12.0).clamp(0.0, GRAPH_NODE_SIZE.x - 16.0);
    (position, alignment, width)
}

pub(in crate::app::views::graph) fn shorten_graph_edge_label(
    label: &str,
    max_width: f32,
) -> String {
    let label = single_line_text(label).into_owned();
    let max_chars = (max_width / GRAPH_EDGE_LABEL_CHAR_WIDTH).floor().max(0.0) as usize;
    let max_chars = max_chars.min(18);
    let characters = label.chars().collect::<Vec<_>>();
    if characters.len() <= max_chars {
        return label;
    }
    if max_chars == 0 {
        return String::new();
    }

    let mut shortened = characters
        .into_iter()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

pub(in crate::app::views::graph) fn layout_graph_edge_label(
    label: &str,
    points: &[Pos2],
    canvas: egui::Rect,
    node_rects: &[egui::Rect],
    occupied_label_rects: &[egui::Rect],
    routed_edges: &[Vec<Pos2>],
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let full_text = single_line_text(label).into_owned();
    let prefer_callout = full_text.chars().count() <= 18;
    let minimum_width =
        single_line_text(label).chars().count().min(6) as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH;
    let mut candidates = vec![edge_label_placement(points, node_rects)];
    let mut segments = points.windows(2).collect::<Vec<_>>();
    segments.sort_by(|left, right| {
        (right[1] - right[0])
            .length_sq()
            .total_cmp(&(left[1] - left[0]).length_sq())
    });
    for segment in segments {
        let direction = segment[1] - segment[0];
        for fraction in [0.5, 0.25, 0.75] {
            let midpoint = segment[0] + direction * fraction;
            for distance in [18.0, 36.0, 54.0, 90.0, 126.0] {
                if direction.x == 0.0 {
                    candidates.extend([
                        (
                            midpoint + Vec2::new(distance, 0.0),
                            Align2::LEFT_CENTER,
                            160.0,
                        ),
                        (
                            midpoint - Vec2::new(distance, 0.0),
                            Align2::RIGHT_CENTER,
                            160.0,
                        ),
                    ]);
                } else {
                    let normal = Vec2::new(direction.y, -direction.x).normalized() * distance;
                    let width = direction.length() - 8.0;
                    candidates.extend([
                        (midpoint + normal, Align2::CENTER_CENTER, width),
                        (midpoint - normal, Align2::CENTER_CENTER, width),
                    ]);
                }
            }
        }
    }
    for (position, alignment, width) in candidates {
        if width < minimum_width {
            continue;
        }
        let text = shorten_graph_edge_label(label, width);
        if prefer_callout && text != full_text {
            continue;
        }
        if let Some(background) = place_edge_label(
            &text,
            position,
            alignment,
            canvas,
            node_rects,
            occupied_label_rects,
            routed_edges,
        ) {
            return Some(graph_edge_label_layout(
                text, position, alignment, background, points,
            ));
        }
    }
    None
}

pub(in crate::app::views::graph) fn graph_edge_label_layout(
    text: String,
    position: Pos2,
    alignment: Align2,
    background: egui::Rect,
    points: &[Pos2],
) -> GraphEdgeLabelLayout {
    let center = background.center();
    let anchor = points
        .windows(2)
        .map(|segment| closest_point_on_segment(center, segment[0], segment[1]))
        .min_by(|left, right| {
            left.distance_sq(center)
                .total_cmp(&right.distance_sq(center))
        })
        .expect("graph labels must belong to a route");
    let direction = anchor - center;
    let border_distance = (background.width() / 2.0 / direction.x.abs())
        .min(background.height() / 2.0 / direction.y.abs());
    let leader = (direction.length() > 24.0)
        .then(|| [anchor, center + direction * border_distance.min(1.0)]);
    GraphEdgeLabelLayout {
        text,
        position,
        alignment,
        background,
        leader,
        reference: None,
    }
}

pub(in crate::app::views::graph) fn aligned_label_rect(
    position: Pos2,
    size: Vec2,
    alignment: Align2,
) -> egui::Rect {
    let min = if alignment == Align2::LEFT_CENTER {
        Pos2::new(position.x, position.y - size.y / 2.0)
    } else if alignment == Align2::RIGHT_CENTER {
        Pos2::new(position.x - size.x, position.y - size.y / 2.0)
    } else {
        position - size / 2.0
    };
    let mut rect = egui::Rect::from_min_size(min, size);
    rect.max.x += 24.0;
    rect
}

pub(in crate::app::views::graph) fn shorten_to_width(
    painter: &egui::Painter,
    text: &str,
    max_chars: usize,
    font_id: &FontId,
    max_width: f32,
    color: egui::Color32,
) -> String {
    let text = single_line_text(text);
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    let needs_ellipsis = characters.next().is_some();
    let candidate = if needs_ellipsis {
        format!("{prefix}…")
    } else {
        prefix.clone()
    };
    if painter
        .layout_no_wrap(candidate.clone(), font_id.clone(), color)
        .size()
        .x
        <= max_width
    {
        return candidate;
    }

    let ellipsis = "…";
    if painter
        .layout_no_wrap(ellipsis.to_string(), font_id.clone(), color)
        .size()
        .x
        > max_width
    {
        return String::new();
    }

    let mut result = String::new();
    for character in prefix.chars() {
        let next = format!("{result}{character}{ellipsis}");
        if painter
            .layout_no_wrap(next, font_id.clone(), color)
            .size()
            .x
            > max_width
        {
            break;
        }
        result.push(character);
    }
    result.push_str(ellipsis);
    result
}
