use super::*;

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
fn graph_connection_hit_testing_handles_bends_crossings_and_zoom() {
    let routes = vec![
        vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
        ],
        vec![Pos2::new(50.0, -50.0), Pos2::new(50.0, 50.0)],
    ];
    assert_eq!(
        graph_edges_at_pointer(&routes, Pos2::new(50.0, 0.0), 6.0),
        [0, 1]
    );
    assert_eq!(
        graph_edges_at_pointer(&routes, Pos2::new(100.0, 80.0), 6.0),
        [0]
    );
    assert!(graph_edges_at_pointer(&routes, Pos2::new(70.0, 70.0), 6.0).is_empty());
    for zoom in [0.1, 1.0, 3.0] {
        assert_eq!(
            graph_edges_at_pointer(
                &[routes[0].clone()],
                Pos2::new(20.0, 5.0 / zoom),
                6.0 / zoom
            ),
            [0]
        );
        assert!(
            graph_edges_at_pointer(
                &[routes[0].clone()],
                Pos2::new(20.0, -7.0 / zoom),
                6.0 / zoom
            )
            .is_empty()
        );
    }
}

#[test]
fn hovering_graph_connection_line_shows_endpoint_information() {
    let graph = layout_graph(
        r#"{"graph":{"type":"directed"},"nodes":[{"id":"a","label":"Source"},{"id":"b","label":"Target"}],"edges":[{"source":"a","target":"b","label":"depends_on","status":"online","active":false,"config":{"latency":12}}]}"#,
    );
    let routing = build_graph_routing_layout(&graph);
    let context = egui::Context::default();
    let render = |time, events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000.0, 600.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                show_graph(
                    ui,
                    &graph,
                    &routing,
                    &SearchState::default(),
                    Locale::English,
                );
            },
        )
    };
    let output = render(0.0, Vec::new());
    let pointer = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, .. } if points[0].distance(points[1]) > 80.0 => {
                Some(points[0] + (points[1] - points[0]) * 0.25)
            }
            _ => None,
        })
        .expect("Graph must draw a connection");
    output.drop_without_applying_deltas();
    let mut visible = false;
    let mut last_texts = Vec::new();
    for frame in 1..=15 {
        let output = render(
            f64::from(frame) * 0.1,
            if frame == 1 {
                vec![egui::Event::PointerMoved(pointer)]
            } else {
                Vec::new()
            },
        );
        let texts = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        visible = texts.contains(&"Directed link")
            && texts.contains(&"depends_on")
            && texts.contains(&"Endpoint A / source: Source (a)")
            && texts.contains(&"Endpoint B / target: Target (b)")
            && texts.contains(&graph.nodes[0].path.as_str())
            && texts.contains(&graph.nodes[1].path.as_str())
            && texts.contains(&"status: \"online\"")
            && texts.contains(&"active: false")
            && texts.contains(&"config.latency: 12");
        last_texts = texts
            .iter()
            .map(|text| text.to_string())
            .collect::<Vec<_>>();
        output.drop_without_applying_deltas();
        if visible {
            break;
        }
    }
    assert!(
        visible,
        "Hovering a connection must expose its label and both endpoints: {pointer:?} {last_texts:?}"
    );
}

#[test]
fn hovering_graph_node_shows_custom_and_nested_attributes() {
    let graph = layout_graph(
        r#"{"graph":{"type":"directed","nodes":[{"id":"a","label":"Alpha","role":"Gateway","active":false,"config":{"region":"West","ports":[80,443]}}],"edges":[]}}"#,
    );
    let routing = build_graph_routing_layout(&graph);
    let context = egui::Context::default();
    let render = |time, events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000.0, 700.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                show_graph(
                    ui,
                    &graph,
                    &routing,
                    &SearchState::default(),
                    Locale::English,
                );
            },
        )
    };
    let output = render(0.0, Vec::new());
    let pointer = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == "Alpha" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("Node label must be visible");
    output.drop_without_applying_deltas();
    let mut visible = false;
    let mut last_texts = Vec::new();
    for frame in 1..=15 {
        let output = render(
            f64::from(frame) * 0.1,
            if frame == 1 {
                vec![egui::Event::PointerMoved(pointer)]
            } else {
                Vec::new()
            },
        );
        last_texts = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        visible = [
            "role: \"Gateway\"",
            "active: false",
            "config.region: \"West\"",
            "config.ports[1]: 443",
        ]
        .iter()
        .all(|expected| last_texts.iter().any(|text| text == expected));
        output.drop_without_applying_deltas();
        if visible {
            break;
        }
    }
    assert!(
        visible,
        "Node hover must expose custom fields: {last_texts:?}"
    );
}

#[test]
fn long_graph_tooltips_allow_scrolling_to_the_last_field() {
    for node_tooltip in [true, false] {
        let mut graph = layout_graph(
            r#"{"graph":{"type":"directed"},"nodes":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],"edges":[{"source":"a","target":"b"}]}"#,
        );
        let attributes = (0..40)
            .map(|index| (format!("extra{index:02}"), format!("value{index:02}")))
            .collect();
        if node_tooltip {
            graph.nodes[0].attributes = attributes;
        } else {
            graph.edges[0].attributes = attributes;
        }
        let routing = build_graph_routing_layout(&graph);
        let context = egui::Context::default();
        let render = |time, events| {
            context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000.0, 700.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    show_graph(
                        ui,
                        &graph,
                        &routing,
                        &SearchState::default(),
                        Locale::English,
                    );
                },
            )
        };
        let text_position = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == label
                        && shape.clip_rect.intersects(text.visual_bounding_rect()) =>
                {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
        };
        let output = render(0.0, Vec::new());
        let hover_pointer = if node_tooltip {
            text_position(&output, "Alpha").unwrap()
        } else {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, .. }
                        if points[0].distance(points[1]) > 80.0 =>
                    {
                        Some(points[0] + (points[1] - points[0]) * 0.25)
                    }
                    _ => None,
                })
                .expect("Graph must draw a connection")
        };
        output.drop_without_applying_deltas();
        let mut tooltip_pointer = None;
        for frame in 1..=15 {
            let output = render(
                f64::from(frame) * 0.1,
                if frame == 1 {
                    vec![egui::Event::PointerMoved(hover_pointer)]
                } else {
                    Vec::new()
                },
            );
            tooltip_pointer = text_position(&output, "extra00: value00");
            assert!(text_position(&output, "extra39: value39").is_none());
            output.drop_without_applying_deltas();
        }
        let pointer = tooltip_pointer.expect("Scrollable tooltip must open");
        let mut last_field_visible = false;
        let mut last_texts = Vec::new();
        for frame in 16..=25 {
            let output = render(
                f64::from(frame) * 0.1,
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -1000.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            last_field_visible = text_position(&output, "extra39: value39").is_some();
            last_texts = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.job.text.clone(),
                        text.visual_bounding_rect(),
                        shape.clip_rect,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            output.drop_without_applying_deltas();
            if last_field_visible {
                break;
            }
        }
        assert!(
            last_field_visible,
            "Moving into the tooltip and scrolling must reveal the last field: {pointer:?} {last_texts:?}"
        );
    }
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
    let last_line = output
        .shapes
        .iter()
        .rposition(|shape| matches!(shape.shape, egui::Shape::LineSegment { .. }));
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
fn graph_menu_zoom_multi_selection_and_marquee_work_together() {
    let graph =
        layout_graph(r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"}]"#);
    let routing = build_graph_routing_layout(&graph);
    let context = egui::Context::default();
    let render = |mut events: Vec<egui::Event>, modifiers: egui::Modifiers| {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let mut state = GraphInteractionState::default();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                let id = egui::Id::new(("graph-interaction", routing.graph_fingerprint));
                egui::Panel::top("test-graph-menu").show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        crate::app::views::graph::graph_view_menu(ui, &routing, Locale::English)
                    });
                });
                show_graph(
                    ui,
                    &graph,
                    &routing,
                    &SearchState::default(),
                    Locale::English,
                );
                state = ui.ctx().data(|data| data.get_temp(id)).unwrap();
            },
        );
        (output, state)
    };
    let position = |output: &egui::FullOutput, label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing graph control or label: {label}"))
    };
    let pointer = |pos, pressed, modifiers| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    let click = |pos, modifiers| {
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, true, modifiers),
            pointer(pos, false, modifiers),
        ]
    };
    let none = egui::Modifiers::NONE;
    let (output, _) = render(Vec::new(), none);
    let plus = position(&output, Locale::English.text(TextKey::GraphZoomIn));
    output.drop_without_applying_deltas();
    let (output, state) = render(click(plus, none), none);
    assert!((state.zoom - 1.2).abs() < 0.001);
    output.drop_without_applying_deltas();
    let (output, _) = render(Vec::new(), none);
    let reset = position(&output, "100%");
    let source = position(&output, "Source");
    let target = position(&output, "Target");
    assert!((target.x - source.x - GRAPH_STEP.x * 1.2).abs() < 2.0);
    output.drop_without_applying_deltas();
    let (output, state) = render(click(source, none), none);
    assert_eq!(state.selected.len(), 1, "Scaled node must remain clickable");
    output.drop_without_applying_deltas();
    let (output, state) = render(click(source, none), none);
    assert!(state.selected.is_empty());
    output.drop_without_applying_deltas();
    let (output, state) = render(click(reset, none), none);
    assert_eq!(state.zoom, 1.0);
    output.drop_without_applying_deltas();
    let (output, _) = render(Vec::new(), none);
    let source = position(&output, "Source");
    let target = position(&output, "Target");
    output.drop_without_applying_deltas();
    let (output, state) = render(click(source, none), none);
    assert_eq!(state.selected.len(), 1);
    output.drop_without_applying_deltas();
    let ctrl = egui::Modifiers { ctrl: true, ..none };
    let (output, state) = render(click(target, ctrl), ctrl);
    assert_eq!(state.selected.len(), 2);
    output.drop_without_applying_deltas();
    let (output, state) = render(click(source, ctrl), ctrl);
    assert_eq!(state.selected.len(), 1);
    output.drop_without_applying_deltas();
    let (output, _) = render(Vec::new(), none);
    let clear = position(&output, "Clear selection");
    output.drop_without_applying_deltas();
    let (output, state) = render(click(clear, none), none);
    assert!(state.selected.is_empty());
    output.drop_without_applying_deltas();
    let (output, _) = render(Vec::new(), none);
    let select_all = position(&output, "Select all");
    output.drop_without_applying_deltas();
    let (output, state) = render(click(select_all, none), none);
    assert_eq!(state.selected.len(), graph.nodes.len());
    output.drop_without_applying_deltas();
    let (output, state) = render(click(clear, none), none);
    assert!(state.selected.is_empty());
    output.drop_without_applying_deltas();
    let start = source - Vec2::new(120.0, 40.0);
    let end = target + Vec2::new(120.0, 65.0);
    let shift = egui::Modifiers {
        shift: true,
        ..none
    };
    let (output, _) = render(
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, shift),
        ],
        shift,
    );
    output.drop_without_applying_deltas();
    let (output, state) = render(vec![egui::Event::PointerMoved(end)], shift);
    assert_eq!(state.selected.len(), 2, "Marquee must select both nodes");
    output.drop_without_applying_deltas();
    let (output, _) = render(vec![pointer(end, false, shift)], shift);
    output.drop_without_applying_deltas();
    let (output, _) = render(Vec::new(), none);
    let fit = position(&output, Locale::English.text(TextKey::GraphFit));
    output.drop_without_applying_deltas();
    let (output, state) = render(click(fit, none), none);
    assert!(routing.content_size.x * state.zoom <= 1000.0);
    assert!(routing.content_size.y * state.zoom <= 600.0);
    output.drop_without_applying_deltas();
}

#[test]
fn graph_timing_details_submenu_keeps_parent_menu_open() {
    let state = GraphCalculationState::default();
    state
        .progress
        .lock()
        .unwrap()
        .timings
        .push((GraphStage::Layout, Duration::from_secs(2)));
    let context = egui::Context::default();
    let render = |events: Vec<egui::Event>| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1200.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ui.menu_button(Locale::English.text(TextKey::GraphTimings), |ui| {
                    state.show_progress(ui, Locale::English);
                });
            },
        )
    };
    let text_position = |output: &egui::FullOutput, expected: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.job.text == expected).then(|| text.visual_bounding_rect().center())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("Menu text must be visible: {expected}"))
    };
    let click = |position| {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let output = render(Vec::new());
    let position = text_position(&output, "Calculation stage timings");
    output.drop_without_applying_deltas();
    let output = render(click(position));
    output.drop_without_applying_deltas();
    let output = render(Vec::new());
    let details = text_position(&output, "Stage details");
    output.drop_without_applying_deltas();
    let output = render(click(details));
    output.drop_without_applying_deltas();
    let output = render(Vec::new());
    text_position(
        &output,
        "Slowest completed stage: Preparing layout and connection ports (2.00 s)",
    );
    text_position(&output, "Stage details");
    text_position(&output, "Preparing layout and connection ports: 2.00 s");
    output.drop_without_applying_deltas();
}
