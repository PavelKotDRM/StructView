use super::*;

fn graph() -> (RelationshipGraph, GraphRoutingLayout) {
    let root = struct_view_core::parser::parse_json(r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"},{"id":"c","name":"Other"}]"#).unwrap();
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);
    (graph, routing)
}

fn render(
    ctx: &egui::Context,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                show_graph(ui, graph, routing, &SearchState::default(), Locale::English)
            });
        },
    )
}

fn state(ctx: &egui::Context, routing: &GraphRoutingLayout) -> GraphInteractionState {
    ctx.data(|data| {
        data.get_temp(egui::Id::new((
            "graph-interaction",
            routing.graph_fingerprint,
        )))
    })
    .unwrap()
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click(point: Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn graph_canvas_keyboard_uses_shared_zoom_fit_and_connected_node_navigation() {
    let (graph, routing) = graph();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut output = render(&ctx, &graph, &routing, Vec::new());
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let bounds = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Relationship graph"))
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let canvas_center = Pos2::new(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    output.drop_without_applying_deltas();
    render(&ctx, &graph, &routing, click(canvas_center)).drop_without_applying_deltas();
    render(&ctx, &graph, &routing, vec![key(egui::Key::Plus)]).drop_without_applying_deltas();
    assert!((state(&ctx, &routing).zoom - crate::app::views::diagram::ZOOM_STEP).abs() < 0.001);
    render(&ctx, &graph, &routing, vec![key(egui::Key::Minus)]).drop_without_applying_deltas();
    assert!((state(&ctx, &routing).zoom - 1.0).abs() < 0.001);
    render(&ctx, &graph, &routing, vec![key(egui::Key::Tab)]).drop_without_applying_deltas();
    render(&ctx, &graph, &routing, vec![key(egui::Key::Space)]).drop_without_applying_deltas();
    assert_eq!(
        state(&ctx, &routing).selected,
        HashSet::from([0]),
        "A focused node must activate once, not toggle twice"
    );
    let source = graph.nodes.iter().position(|node| node.id == "a").unwrap();
    let target = graph.nodes.iter().position(|node| node.id == "b").unwrap();
    let mut interaction = state(&ctx, &routing);
    interaction.cursor = source;
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(("graph-interaction", routing.graph_fingerprint)),
            interaction,
        );
    });
    render(&ctx, &graph, &routing, vec![key(egui::Key::ArrowRight)]).drop_without_applying_deltas();
    assert_eq!(state(&ctx, &routing).selected, HashSet::from([target]));
    render(&ctx, &graph, &routing, vec![key(egui::Key::ArrowLeft)]).drop_without_applying_deltas();
    assert_eq!(state(&ctx, &routing).selected, HashSet::from([source]));
    render(&ctx, &graph, &routing, vec![key(egui::Key::Space)]).drop_without_applying_deltas();
    assert!(state(&ctx, &routing).selected.is_empty());
    render(&ctx, &graph, &routing, vec![key(egui::Key::Enter)]).drop_without_applying_deltas();
    assert_eq!(state(&ctx, &routing).selected, HashSet::from([source]));
    render(&ctx, &graph, &routing, vec![key(egui::Key::ArrowDown)]).drop_without_applying_deltas();
    assert_eq!(
        state(&ctx, &routing).cursor,
        (source + 1).min(graph.nodes.len() - 1)
    );
    render(&ctx, &graph, &routing, vec![key(egui::Key::ArrowUp)]).drop_without_applying_deltas();
    assert_eq!(state(&ctx, &routing).cursor, source);
    render(&ctx, &graph, &routing, vec![key(egui::Key::Home)]).drop_without_applying_deltas();
    assert!(
        routing.content_size.x * state(&ctx, &routing).zoom
            <= bounds.x1 as f32 - bounds.x0 as f32 + 1.0
    );
}

#[test]
fn wheel_zooms_about_pointer_and_unmodified_drag_pans_without_marquee() {
    let (graph, routing) = graph();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut output = render(&ctx, &graph, &routing, Vec::new());
    output.textures_delta.clear();
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Relationship graph"))
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let canvas_min = Vec2::new(bounds.x0 as f32, bounds.y0 as f32);
    output.drop_without_applying_deltas();
    let point = Pos2::new(800.0, 500.0);
    render(
        &ctx,
        &graph,
        &routing,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, 60.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    )
    .drop_without_applying_deltas();
    assert!(state(&ctx, &routing).zoom > 1.0);
    let zoomed = state(&ctx, &routing);
    let anchor = point - canvas_min;
    assert!(((anchor.to_vec2() - zoomed.pan) / zoomed.zoom - anchor.to_vec2()).length() < 0.01);
    for _ in 0..60 {
        render(&ctx, &graph, &routing, Vec::new()).drop_without_applying_deltas();
    }
    let initial = state(&ctx, &routing);
    render(
        &ctx,
        &graph,
        &routing,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    )
    .drop_without_applying_deltas();
    render(
        &ctx,
        &graph,
        &routing,
        vec![egui::Event::PointerMoved(point + Vec2::new(40.0, 30.0))],
    )
    .drop_without_applying_deltas();
    let moved = state(&ctx, &routing);
    assert!(
        (moved.pan - initial.pan).length() > 20.0,
        "pan before {:?}, after {:?}",
        initial.pan,
        moved.pan
    );
    assert!(moved.marquee_start.is_none());
    assert!(moved.selected.is_empty());
}
