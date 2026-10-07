use super::super::*;

#[test]
fn blocked_and_short_routes_keep_legible_callouts_but_empty_edges_stay_unlabelled() {
    let points = [Pos2::new(40.0, 50.0), Pos2::new(45.0, 50.0)];
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
    assert!(
        layout_graph_edge_label("long_relationship", &points, canvas, &[canvas], &[], &[])
            .is_none()
    );
    let first = graph_edge_label_callout("long_relationship", &points, canvas, &[]).unwrap();
    let second =
        graph_edge_label_callout("other_relationship", &points, canvas, &[first.background])
            .unwrap();
    assert_eq!(first.text, "long_relationship");
    assert!(first.background.left() > canvas.right());
    assert!(first.leader.is_some());
    assert!(!first.background.intersects(second.background));
    assert!(graph_edge_label_callout("", &points, canvas, &[]).is_none());
    assert!(layout_graph_edge_label("", &points, canvas, &[], &[], &[]).is_none());
}

#[test]
fn crossing_label_leaders_do_not_both_remain_visible() {
    let routes = vec![
        vec![Pos2::new(20.0, 0.0), Pos2::new(40.0, 0.0)],
        vec![Pos2::new(20.0, 100.0), Pos2::new(40.0, 100.0)],
    ];
    let mut labels = routes
        .iter()
        .enumerate()
        .map(|(index, route)| {
            let y = if index == 0 { 100.0 } else { 0.0 };
            let position = Pos2::new(200.0, y);
            let mut label = graph_edge_label_layout(
                format!("link {index}"),
                position,
                Align2::LEFT_CENTER,
                aligned_label_rect(position, Vec2::new(60.0, 16.0), Align2::LEFT_CENTER),
                route,
            );
            label.leader = Some([route[1], position]);
            Some(label)
        })
        .collect::<Vec<_>>();
    resolve_graph_label_leaders(&mut labels, &[], &routes);
    assert_eq!(
        labels
            .iter()
            .flatten()
            .filter(|label| label.leader.is_some())
            .count(),
        1
    );
    assert_eq!(
        labels
            .iter()
            .flatten()
            .filter(|label| label.reference.is_some())
            .count(),
        1
    );
}

#[test]
fn dense_graph_keeps_every_nonempty_relationship_label_inside_canvas() {
    let mut graph = layout_graph(r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#);
    let edge = graph.edges[0].clone();
    graph.edges = (0..24)
        .map(|index| {
            let mut edge = edge.clone();
            edge.label = format!("relationship_{index:02}");
            edge
        })
        .collect();
    let layout = build_graph_routing_layout(&graph);
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, layout.content_size);
    assert!(layout.edge_labels.iter().all(Option::is_some));
    assert!(
        layout.edge_labels.iter().flatten().any(|label| {
            label.background.left()
                > layout
                    .node_positions
                    .iter()
                    .map(|point| point.x)
                    .max_by(f32::total_cmp)
                    .unwrap()
                    + GRAPH_NODE_SIZE.x / 2.0
                    + 24.0
        }),
        "Fixture must exercise reserved callout placement"
    );
    let connected_bottom = layout
        .node_positions
        .iter()
        .map(|position| position.y + GRAPH_NODE_SIZE.y / 2.0)
        .chain(layout.edge_paths.iter().flatten().map(|point| point.y))
        .fold(0.0_f32, f32::max)
        + 24.0;
    let callouts = layout
        .edge_labels
        .iter()
        .flatten()
        .filter(|label| {
            label.background.left()
                > layout
                    .node_positions
                    .iter()
                    .map(|point| point.x)
                    .max_by(f32::total_cmp)
                    .unwrap()
                    + GRAPH_NODE_SIZE.x / 2.0
                    + 24.0
        })
        .collect::<Vec<_>>();
    let mut callout_rows = callouts
        .iter()
        .map(|label| label.background.center().y)
        .collect::<Vec<_>>();
    callout_rows.sort_by(f32::total_cmp);
    callout_rows.dedup_by(|left, right| *left == *right);
    assert!(
        callout_rows.len() > 1,
        "Tall callout legends should wrap into multiple rows"
    );
    assert!(
        callouts
            .iter()
            .all(|label| label.background.bottom() <= connected_bottom),
        "Callout legend should fit within the connected graph height"
    );
    let mut occupied = Vec::new();
    for (edge, label) in graph.edges.iter().zip(&layout.edge_labels) {
        let label = label.as_ref().unwrap();
        assert_eq!(label.text, edge.label);
        assert!(canvas.contains_rect(label.background));
        assert!(
            occupied
                .iter()
                .all(|rect| !label.background.intersects(*rect))
        );
        occupied.push(label.background);
        for position in &layout.node_positions {
            assert!(
                !label.background.intersects(
                    egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE).expand(2.0),
                )
            );
        }
        for route in &layout.edge_paths {
            assert!(route.windows(2).all(|segment| {
                !segment_intersects_rect(segment[0], segment[1], label.background.expand(2.0))
            }));
            assert!(!label.background.intersects(egui::Rect::from_center_size(
                *route.last().unwrap(),
                Vec2::splat(20.0),
            )));
        }
    }
    let context = egui::Context::default();
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                layout.content_size + Vec2::splat(200.0),
            )),
            ..Default::default()
        },
        |ui| {
            show_graph(
                ui,
                &graph,
                &layout,
                &SearchState::default(),
                Locale::English,
            );
        },
    );
    let rendered = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let last_line = output.shapes.iter().rposition(|shape| {
        matches!(
            &shape.shape,
            egui::Shape::LineSegment { .. } | egui::Shape::Path(_)
        )
    });
    let first_relationship = output.shapes.iter().position(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.starts_with("relationship_"))
    });
    output.drop_without_applying_deltas();
    for edge in &graph.edges {
        assert!(
            rendered.contains(&edge.label),
            "Relationship must be drawn: {}",
            edge.label
        );
    }
    assert!(
        last_line.unwrap() < first_relationship.unwrap(),
        "Routes and leaders must be drawn before relationship text"
    );
}

#[test]
fn graph_label_leaders_use_references_when_nodes_routes_or_labels_block_them() {
    let own_route = vec![Pos2::new(50.0, -40.0), Pos2::new(50.0, 40.0)];
    let make_label = || {
        let mut label = graph_edge_label_layout(
            "connection".into(),
            Pos2::new(200.0, 0.0),
            Align2::LEFT_CENTER,
            aligned_label_rect(
                Pos2::new(200.0, 0.0),
                Vec2::new(80.0, 16.0),
                Align2::LEFT_CENTER,
            ),
            &own_route,
        );
        label.leader = Some([Pos2::new(50.0, 0.0), Pos2::new(200.0, 0.0)]);
        label
    };
    let mut clear = vec![Some(make_label())];
    resolve_graph_label_leaders(&mut clear, &[], std::slice::from_ref(&own_route));
    assert!(clear[0].as_ref().unwrap().leader.is_some());
    assert!(clear[0].as_ref().unwrap().reference.is_none());
    for blocker in 0..3 {
        let mut labels = vec![Some(make_label())];
        let mut routes = vec![own_route.clone()];
        let mut nodes = Vec::new();
        match blocker {
            0 => nodes.push(egui::Rect::from_center_size(
                Pos2::new(120.0, 0.0),
                Vec2::splat(30.0),
            )),
            1 => routes.push(vec![Pos2::new(120.0, -30.0), Pos2::new(120.0, 30.0)]),
            _ => {
                let mut other = make_label();
                other.background =
                    egui::Rect::from_center_size(Pos2::new(120.0, 0.0), Vec2::splat(30.0));
                other.leader = None;
                labels.push(Some(other));
                routes.push(own_route.clone());
            }
        }
        resolve_graph_label_leaders(&mut labels, &nodes, &routes);
        let label = labels[0].as_ref().unwrap();
        assert!(label.leader.is_none());
        let (number, anchor) = label.reference.unwrap();
        assert_eq!(number, 1);
        assert_eq!(
            point_to_segment_distance(anchor, own_route[0], own_route[1]),
            0.0
        );
        assert_eq!(label.text, "connection");
    }
}

#[test]
fn numbered_markers_clear_own_and_neighbor_arrowheads_even_when_routes_are_crowded() {
    for direction in [
        Vec2::X,
        -Vec2::X,
        Vec2::Y,
        -Vec2::Y,
        Vec2::new(1.0, 1.0).normalized(),
    ] {
        for length in [4.0, 100.0] {
            for crowded in [false, true] {
                let start = Pos2::new(100.0, 100.0);
                let tip = start + direction * length;
                let own = vec![start, tip];
                let neighbor = vec![
                    tip - direction * 50.0 + Vec2::new(3.0, 3.0),
                    tip + Vec2::new(3.0, 3.0),
                ];
                let mut routes = vec![own.clone(), neighbor];
                if crowded {
                    routes.push(own.clone());
                }
                let position = Pos2::new(400.0, 300.0);
                let mut label = graph_edge_label_layout(
                    "blocked".into(),
                    position,
                    Align2::LEFT_CENTER,
                    aligned_label_rect(position, Vec2::new(80.0, 16.0), Align2::LEFT_CENTER),
                    &own,
                );
                label.leader = Some([tip, position]);
                let blocker =
                    egui::Rect::from_center_size(tip.lerp(position, 0.5), Vec2::splat(20.0));
                let mut labels = vec![Some(label)];
                resolve_graph_label_leaders(&mut labels, &[blocker], &routes);
                let (_, anchor) = labels[0].as_ref().unwrap().reference.unwrap();
                let marker = egui::Rect::from_center_size(anchor, Vec2::splat(18.0));
                for route in &routes {
                    for (tip, direction) in edge_arrowheads(route, EdgeDirection::Bidirectional) {
                        for wing in arrow_head_wings(tip, direction, 1.0) {
                            assert!(
                                !segment_intersects_rect(tip, wing, marker),
                                "marker overlaps an arrow: {direction:?}, length {length}, crowded {crowded}"
                            );
                        }
                    }
                }
            }
        }
    }
}
