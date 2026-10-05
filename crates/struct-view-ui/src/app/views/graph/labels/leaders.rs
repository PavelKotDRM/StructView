use super::super::*;

pub(in crate::app::views::graph) fn resolve_graph_label_leaders(
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
