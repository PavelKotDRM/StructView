use super::*;
use std::collections::HashSet;

fn graph() -> (RelationshipGraph, GraphRoutingLayout) {
    let root = struct_view_core::parser::parse_json(r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"},{"id":"c","name":"Other"}]"#).unwrap();
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);
    (graph, routing)
}

#[test]
fn flowchart_job_labels_and_markers_show_full_job_information_on_hover() {
    use crate::app::graph_animation::{ElementStatus, RealtimeCommand, RealtimeUpdate, realtime};

    let root = struct_view_core::parser::parse_json(
        r#"{"graph":{"type":"flowchart"},"nodes":[{"id":"a","label":"Receive","shape":"input_output"},{"id":"b","label":"Pay","shape":"process"}],"edges":[{"source":"a","target":"b"}]}"#,
    )
    .unwrap();
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);
    let label = "Order processing with complete customer information";
    let stage = "Checking all payment details before approval";
    for marker_label in [false, true] {
        let ctx = egui::Context::default();
        ctx.style_mut_of(ctx.theme(), |style| style.interaction.tooltip_delay = 0.0);
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::unique(("graph-interaction", routing.graph_fingerprint)),
                interaction::GraphInteractionState {
                    pan: Vec2::new(100.0, 140.0),
                    ..Default::default()
                },
            );
        });
        let state = realtime::RealtimeState::apply(
            None,
            RealtimeUpdate {
                graph_id: format!("{:016x}", routing.graph_fingerprint),
                commands: vec![
                    RealtimeCommand::Element {
                        id: "order-42".into(),
                        nodes: vec!["b".into()],
                        edges: vec![0],
                        status: Some(ElementStatus::Running),
                        color: None,
                        pulse: None,
                        label: Some(label.into()),
                        stage: Some(stage.into()),
                    },
                    RealtimeCommand::Marker {
                        id: "order-42".into(),
                        edge: 0,
                        percent: 80.0,
                        smoothing_ms: 300,
                        color: [40, 160, 255],
                        reverse: false,
                        label: Some(label.into()),
                        stage: Some(stage.into()),
                    },
                ],
            },
            &graph,
            0.0,
        )
        .unwrap();
        realtime::store(&ctx, state);
        let output = render(&ctx, &graph, &routing, vec![]);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text().starts_with("Order processing")
                        && text.galley.text().contains('%') == marker_label =>
                {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("painted job label");
        output.drop_without_applying_deltas();
        for _ in 0..3 {
            render(
                &ctx,
                &graph,
                &routing,
                vec![egui::Event::PointerMoved(point)],
            )
            .drop_without_applying_deltas();
        }
        let output = render(&ctx, &graph, &routing, vec![]);
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains(label), "missing full job label: {text}");
        assert!(text.contains(stage), "missing full job stage: {text}");
        assert!(text.contains("Target: 80%"), "missing job progress: {text}");
        output.drop_without_applying_deltas();
    }
}

#[test]
fn graph_paths_and_arrowheads_are_drawn_with_panel_fill_casing() {
    let (graph, routing) = graph();
    let ctx = egui::Context::default();
    let mut panel_fill = egui::Color32::TRANSPARENT;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                panel_fill = ui.visuals().panel_fill;
                show_graph(
                    ui,
                    &graph,
                    &routing,
                    &SearchState::default(),
                    Locale::English,
                );
            });
        },
    );
    let arrow_outline_count = output
        .shapes
        .iter()
        .filter(|clipped| {
            matches!(
                &clipped.shape,
                egui::Shape::LineSegment { stroke, .. }
                    if stroke.color == panel_fill && stroke.width > 2.0
            )
        })
        .count();
    assert_eq!(arrow_outline_count, 2, "Both arrowhead wings need a casing");
    let path_outline_count = output
        .shapes
        .iter()
        .filter(|clipped| {
            matches!(
                &clipped.shape,
                egui::Shape::Path(path)
                    if path.stroke.color == egui::epaint::ColorMode::Solid(panel_fill)
                        && path.stroke.width > 2.0
            )
        })
        .count();
    assert_eq!(
        path_outline_count, 1,
        "The route needs a panel-colored casing"
    );
    output.drop_without_applying_deltas();
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

#[test]
fn flowchart_canvas_keeps_long_labels_and_wraps_them_into_the_node() {
    let label = "A very long label that needs multiple lines to fit";
    let input = serde_json::json!({
        "graph": {"type": "flowchart"},
        "nodes": [{"id": "node-id", "label": label}],
        "edges": []
    });
    let root = struct_view_core::parser::parse_json(&input.to_string()).unwrap();
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);
    let output = render(&egui::Context::default(), &graph, &routing, Vec::new());
    let expected = flowchart_label_lines(label, graph.nodes[0].flowchart_shape()).join("\n");
    let rendered_label = output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.text() == expected => Some(text.galley.text()),
            _ => None,
        });

    assert_eq!(rendered_label, Some(expected.as_str()));
    assert!(expected.contains('\n'));
    output.drop_without_applying_deltas();
}

#[test]
fn subprocess_side_lines_scale_with_the_node() {
    let root = struct_view_core::parser::parse_json(
        r#"{"graph":{"type":"flowchart"},"nodes":[{"id":"subprocess","label":"Subprocess","shape":"subprocess"}],"edges":[]}"#,
    )
    .unwrap();
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);

    for zoom in [0.5, 1.0, 2.0] {
        let ctx = egui::Context::default();
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::unique(("graph-interaction", routing.graph_fingerprint)),
                interaction::GraphInteractionState {
                    zoom,
                    ..Default::default()
                },
            );
        });
        let output = render(&ctx, &graph, &routing, Vec::new());
        let side_lines = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::LineSegment { points, .. }
                    if (points[0].x - points[1].x).abs() < f32::EPSILON
                        && (points[0].y - points[1].y).abs() == routing.node_sizes[0].y * zoom =>
                {
                    Some(points[0].x)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(side_lines.len(), 2);
        let spacing = (side_lines[1] - side_lines[0]).abs();
        assert!(
            (spacing - (routing.node_sizes[0].x - 20.0) * zoom).abs() < 0.01,
            "side line inset should scale with node zoom {zoom}: spacing was {spacing}"
        );
        output.drop_without_applying_deltas();
    }
}

#[test]
fn selection_markers_follow_flowchart_node_contours() {
    use crate::app::visualization::FlowchartNodeShape as Shape;

    let rect = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(200.0, 80.0));
    let decision = super::render::graph_selection_marker_center(rect, Some(Shape::Decision), 1.0);
    let input_output =
        super::render::graph_selection_marker_center(rect, Some(Shape::InputOutput), 1.0);
    let preparation =
        super::render::graph_selection_marker_center(rect, Some(Shape::Preparation), 1.0);
    let terminator =
        super::render::graph_selection_marker_center(rect, Some(Shape::Terminator), 1.0);
    let process = super::render::graph_selection_marker_center(rect, Some(Shape::Process), 1.0);

    assert!(decision.x > rect.center().x && decision.y < rect.center().y);
    assert!(decision.x < rect.right() && decision.y > rect.top());
    assert_eq!(input_output, Pos2::new(160.0, -3.0));
    assert_eq!(preparation, Pos2::new(176.0, -3.0));
    assert!(terminator.x < process.x);
    assert!(terminator.y > process.y);
}

#[test]
fn all_flowchart_shape_details_scale_proportionally() {
    use crate::app::visualization::FlowchartNodeShape as Shape;

    let shapes = [
        Shape::Process,
        Shape::Terminator,
        Shape::Decision,
        Shape::InputOutput,
        Shape::Preparation,
        Shape::Subprocess,
        Shape::Database,
        Shape::Connector,
    ];
    let base_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 80.0));
    let center = base_rect.center();
    let base_radii = [
        (Shape::Process, 6.0),
        (Shape::Terminator, 40.0),
        (Shape::Decision, 6.0),
        (Shape::InputOutput, 6.0),
        (Shape::Preparation, 6.0),
        (Shape::Subprocess, 6.0),
        (Shape::Database, 14.0),
        (Shape::Connector, 40.0),
    ];

    for zoom in [0.5, 1.0, 2.0, 4.0] {
        let scaled_rect = egui::Rect::from_center_size(center, base_rect.size() * zoom);
        for shape in shapes {
            let base_marker =
                super::render::graph_selection_marker_center(base_rect, Some(shape), 1.0);
            let scaled_marker =
                super::render::graph_selection_marker_center(scaled_rect, Some(shape), zoom);
            let expected_marker = center + (base_marker - center) * zoom;
            assert!(
                scaled_marker.distance(expected_marker) < 0.01,
                "{shape:?} marker should scale proportionally at {zoom}x"
            );

            let expected_radius = base_radii
                .iter()
                .find_map(|(candidate, radius)| (*candidate == shape).then_some(radius * zoom))
                .unwrap();
            let radius =
                super::render::flowchart_node_corner_radius(scaled_rect, Some(shape), zoom);
            assert!(
                (f32::from(radius) - expected_radius).abs() < 0.51,
                "{shape:?} corner radius should scale proportionally at {zoom}x"
            );
        }
    }
}

fn state(ctx: &egui::Context, routing: &GraphRoutingLayout) -> GraphInteractionState {
    ctx.data(|data| {
        data.get_temp(egui::Id::unique((
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
        .find(|(_, node)| node.label() == Some("Graph and flowchart"))
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
            egui::Id::unique(("graph-interaction", routing.graph_fingerprint)),
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
        .find(|(_, node)| node.label() == Some("Graph and flowchart"))
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
                source: egui::MouseWheelSource::Wheel,
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
