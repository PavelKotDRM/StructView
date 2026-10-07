use super::super::*;

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
