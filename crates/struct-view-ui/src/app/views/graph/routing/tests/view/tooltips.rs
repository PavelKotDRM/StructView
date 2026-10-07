use super::super::*;

fn route_pointer(output: &egui::FullOutput) -> Option<Pos2> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Path(path) => path.points.windows(2).find_map(|points| {
            (points[0].distance(points[1]) > 80.0)
                .then(|| points[0] + (points[1] - points[0]) * 0.25)
        }),
        egui::Shape::LineSegment { points, .. } if points[0].distance(points[1]) > 80.0 => {
            Some(points[0] + (points[1] - points[0]) * 0.25)
        }
        _ => None,
    })
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
    let pointer = route_pointer(&output).expect("Graph must draw a connection");
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
            route_pointer(&output).expect("Graph must draw a connection")
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
        let mut scroll_start_frame = 16;
        if !node_tooltip {
            let ctrl = egui::Modifiers {
                ctrl: true,
                ..egui::Modifiers::NONE
            };
            let output = render(1.6, vec![egui::Event::ModifiersChanged(ctrl)]);
            assert!(
                text_position(&output, "extra00: value00").is_some(),
                "The route tooltip should still be visible when Ctrl is pressed"
            );
            output.drop_without_applying_deltas();

            let output = render(
                1.7,
                vec![
                    egui::Event::ModifiersChanged(ctrl),
                    egui::Event::PointerMoved(Pos2::new(999.0, 699.0)),
                ],
            );
            assert!(
                text_position(&output, "extra00: value00").is_some(),
                "Holding Ctrl should keep the route tooltip open while moving to it"
            );
            output.drop_without_applying_deltas();

            let output = render(
                1.8,
                vec![
                    egui::Event::ModifiersChanged(ctrl),
                    egui::Event::PointerMoved(pointer),
                ],
            );
            assert!(text_position(&output, "extra00: value00").is_some());
            output.drop_without_applying_deltas();
            scroll_start_frame = 19;
        }
        let mut last_field_visible = false;
        let mut last_texts = Vec::new();
        for frame in scroll_start_frame..=scroll_start_frame + 9 {
            let mut events = vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -1000.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            if frame == scroll_start_frame {
                events.insert(0, egui::Event::ModifiersChanged(egui::Modifiers::NONE));
            }
            let output = render(f64::from(frame) * 0.1, events);
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
