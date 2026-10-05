use super::*;

pub(super) struct GraphEdgeLabelLayout {
    pub(super) text: String,
    pub(super) position: Pos2,
    pub(super) alignment: Align2,
    pub(super) background: egui::Rect,
    pub(super) leader: Option<[Pos2; 2]>,
    pub(super) reference: Option<(usize, Pos2)>,
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

fn shorten_graph_edge_label(label: &str, max_width: f32) -> String {
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

pub(super) fn layout_graph_edge_label(
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

pub(super) fn graph_edge_label_layout(
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

pub(super) fn resolve_graph_label_leaders(
    labels: &mut [Option<GraphEdgeLabelLayout>],
    node_rects: &[egui::Rect],
    routes: &[Vec<Pos2>],
) {
    // Reserve both ends so source arrowheads in reverse/mutual edges stay unobstructed.
    let arrowheads = routes
        .iter()
        .flat_map(|route| {
            edge_arrowheads(route, EdgeDirection::Bidirectional)
                .into_iter()
                .map(|(tip, direction)| {
                    let wings = arrow_head_wings(tip, direction, 1.0);
                    egui::Rect::from_points(&[tip, wings[0], wings[1]]).expand(2.0)
                })
        })
        .collect::<Vec<_>>();
    let backgrounds = labels
        .iter()
        .flatten()
        .map(|label| label.background)
        .collect::<Vec<_>>();
    let mut accepted: Vec<[Pos2; 2]> = Vec::new();
    let mut reference = 0;
    let mut markers: Vec<egui::Rect> = Vec::new();
    for (index, label) in labels.iter_mut().enumerate() {
        let Some(label) = label else { continue };
        let Some([start, end]) = label.leader else {
            continue;
        };
        let blocked = node_rects
            .iter()
            .any(|rect| segment_intersects_rect(start, end, rect.expand(3.0)))
            || backgrounds.iter().any(|rect| {
                *rect != label.background && segment_intersects_rect(start, end, rect.expand(2.0))
            })
            || routes.iter().enumerate().any(|(route_index, route)| {
                route.windows(2).any(|segment| {
                    if route_index == index {
                        // Touching the owning route at the attachment point is intentional.
                        segments_intersect(
                            start + (end - start).normalized() * 3.0,
                            end,
                            segment[0],
                            segment[1],
                        )
                    } else {
                        segments_within_clearance(start, end, segment[0], segment[1], 3.0)
                    }
                })
            })
            || accepted.iter().any(|&[other_start, other_end]| {
                segments_within_clearance(start, end, other_start, other_end, 3.0)
            });
        if blocked {
            reference += 1;
            label.leader = None;
            let candidates = routes[index]
                .windows(2)
                .flat_map(|segment| {
                    (1..16)
                        .map(|step| segment[0] + (segment[1] - segment[0]) * (step as f32 / 16.0))
                })
                .collect::<Vec<_>>();
            let clears_arrowheads = |point: Pos2| {
                let marker = egui::Rect::from_center_size(point, Vec2::splat(18.0));
                !arrowheads.iter().any(|rect| marker.intersects(*rect))
            };
            let anchor = candidates
                .iter()
                .copied()
                .filter(|point| {
                    let marker = egui::Rect::from_center_size(*point, Vec2::splat(18.0));
                    clears_arrowheads(*point)
                        && !node_rects
                            .iter()
                            .chain(&backgrounds)
                            .any(|rect| marker.intersects(*rect))
                        && !markers.iter().any(|rect| marker.intersects(*rect))
                        && !routes.iter().enumerate().any(|(route_index, route)| {
                            route_index != index
                                && route.windows(2).any(|segment| {
                                    segment_intersects_rect(segment[0], segment[1], marker)
                                })
                        })
                })
                .min_by(|left, right| left.distance_sq(start).total_cmp(&right.distance_sq(start)))
                .or_else(|| {
                    // Dense routes can lack a fully clear position. Never relax arrow clearance.
                    candidates
                        .iter()
                        .copied()
                        .chain(std::iter::once(start))
                        .filter(|point| clears_arrowheads(*point))
                        .min_by(|left, right| {
                            left.distance_sq(start).total_cmp(&right.distance_sq(start))
                        })
                })
                .unwrap_or_else(|| {
                    // A very short route may be entirely covered by an arrowhead.
                    let mut point = start;
                    while !clears_arrowheads(point) {
                        point.x += 18.0;
                    }
                    point
                });
            label.reference = Some((reference, anchor));
            markers.push(egui::Rect::from_center_size(anchor, Vec2::splat(18.0)));
        } else {
            accepted.push([start, end]);
        }
    }
}

pub(super) fn graph_edge_label_callout(
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

pub(super) fn graph_edge_label_callout_text(
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

pub(super) fn place_edge_label(
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

pub(super) fn aligned_label_rect(position: Pos2, size: Vec2, alignment: Align2) -> egui::Rect {
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

pub(super) fn shorten_to_width(
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
