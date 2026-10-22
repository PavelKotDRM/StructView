use super::super::visualization::try_build_relationship_graph;
use super::super::visualization::{RelationshipGraph, VisualizationMode};
use super::server::{ApiOperation, ApiReply};
use super::*;
use crate::app::StructViewApp;
use crate::app::i18n::TextKey;
use egui::{Color32, Pos2, Vec2};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};
use struct_view_core::parser::parse_json;

fn graph() -> RelationshipGraph {
    try_build_relationship_graph(
        &parse_json(
            r#"{
        "graph":{"type":"directed_multigraph"},
        "nodes":[{"id":"a"},{"id":"b"}],
        "edges":[{"source":"a","target":"b"},{"source":"a","target":"a"}]
    }"#,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn graph_descriptor_exposes_flowchart_type_and_node_shapes() {
    let root = parse_json(
        r#"{"graph":{"type":"flowchart"},"nodes":[{"id":"start","label":"Start","shape":"terminator"},{"id":"decision","label":"Decision","shape":"diamond"}],"edges":[{"source":"start","target":"decision","label":"next"}]}"#,
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    let descriptor = graph_descriptor("flowchart-1", &graph);

    assert_eq!(descriptor["type"], "flowchart");
    assert_eq!(descriptor["nodes"][0]["shape"], "terminator");
    assert_eq!(descriptor["nodes"][1]["shape"], "decision");
}

#[test]
fn api_payloads_are_schema_validated_before_checking_active_graph_state() {
    assert!(decode_api_operation(ApiOperation::Play(json!([]))).is_err());
    assert!(decode_api_operation(ApiOperation::Realtime(json!({}))).is_err());
    assert!(matches!(
        decode_api_operation(ApiOperation::Play(
            serde_json::to_value(scenario(AnimationCommand::Clear)).unwrap()
        )),
        Ok(GuiApiOperation::Play(_))
    ));
}

#[test]
fn demo_toml_fixture_exposes_stable_entity_ids_and_shop_relationships() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("Graph.toml");
    let input = std::fs::read_to_string(path).unwrap();
    let (root, _) = struct_view_core::parser::parse_data(
        &input,
        Some(struct_view_core::parser::DataFormat::Toml),
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    let descriptor = graph_descriptor("demo-graph", &graph);
    assert_eq!(descriptor["nodes"].as_array().unwrap().len(), 100);
    assert_eq!(descriptor["edges"].as_array().unwrap().len(), 250);
    for id in [
        "customer",
        "order",
        "order_item",
        "product",
        "category",
        "payment",
        "delivery",
    ] {
        assert!(
            descriptor["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == id)
        );
    }
    for (source, target) in [
        ("customer", "order"),
        ("order", "order_item"),
        ("order_item", "product"),
        ("product", "category"),
        ("order", "payment"),
        ("order", "delivery"),
    ] {
        assert!(
            descriptor["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|edge| edge["source"] == source && edge["target"] == target)
        );
    }
}

#[test]
fn entity_relation_arrays_reject_unknown_endpoints_and_ambiguous_aliases() {
    for input in [
        r#"{"graph":{"directed":true},"entities":[{"id":"a"}],"relations":[{"source":"a","target":"missing"}]}"#,
        r#"{"graph":{"directed":true},"entities":[{"id":"a"}],"nodes":[{"id":"b"}],"relations":[]}"#,
        r#"{"graph":{"directed":true},"entities":[{"id":"a"}],"relations":[],"edges":[]}"#,
    ] {
        assert!(try_build_relationship_graph(&parse_json(input).unwrap()).is_err());
    }
}

#[test]
fn compact_animation_demo_fixture_builds_the_expected_graph() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("GraphAnimationDemo.toml");
    let input = std::fs::read_to_string(path).unwrap();
    let (root, _) = struct_view_core::parser::parse_data(
        &input,
        Some(struct_view_core::parser::DataFormat::Toml),
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    assert_eq!(graph.nodes.len(), 8);
    assert_eq!(graph.edges.len(), 9);
    assert!(graph.edges.iter().any(|edge| {
        graph.nodes[edge.source].id == "order" && graph.nodes[edge.target].id == "payment"
    }));
}

#[test]
fn flowchart_animation_demo_fixture_is_exposed_to_the_api_with_all_shapes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("FlowchartAnimationDemo.toml");
    let input = std::fs::read_to_string(path).unwrap();
    let (root, _) = struct_view_core::parser::parse_data(
        &input,
        Some(struct_view_core::parser::DataFormat::Toml),
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    let descriptor = graph_descriptor("flowchart-demo", &graph);

    assert_eq!(descriptor["type"], "flowchart");
    assert_eq!(descriptor["nodes"].as_array().unwrap().len(), 16);
    assert_eq!(descriptor["edges"].as_array().unwrap().len(), 17);
    let shapes = descriptor["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["shape"].as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        shapes,
        std::collections::HashSet::from([
            "process",
            "terminator",
            "decision",
            "input_output",
            "preparation",
            "subprocess",
            "database",
            "connector",
        ])
    );
    assert!(
        descriptor["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["source"] == "reserve_stock" && edge["target"] == "payment")
    );
}

fn scenario(command: AnimationCommand) -> AnimationScenario {
    AnimationScenario {
        graph_id: "id".into(),
        steps: vec![AnimationStep { at_ms: 0, command }],
    }
}

#[test]
fn protocol_rejects_unknown_fields_invalid_targets_and_excessive_timelines() {
    assert!(serde_json::from_value::<AnimationScenario>(json!({
        "graph_id":"id","steps":[{"command":{"type":"highlight","nodes":["a"],"move_node":true}}]
    })).is_err());
    let graph = graph();
    for command in [
        AnimationCommand::Highlight {
            nodes: vec!["missing".into()],
            edges: vec![],
            color: default_color(),
            duration_ms: 1000,
            pulse: false,
        },
        AnimationCommand::Flow {
            edge: 999,
            color: default_color(),
            duration_ms: 1000,
            reverse: false,
        },
        AnimationCommand::Camera {
            node: None,
            zoom: Some(100.0),
            fit: false,
            duration_ms: 1000,
        },
        AnimationCommand::Camera {
            node: Some("a".into()),
            zoom: None,
            fit: true,
            duration_ms: 1000,
        },
        AnimationCommand::Flow {
            edge: 0,
            color: default_color(),
            duration_ms: 0,
            reverse: false,
        },
    ] {
        assert!(AnimationPlayer::new(scenario(command), &graph, 0.0).is_err());
    }
    let mut input = scenario(AnimationCommand::Clear);
    input.steps[0].at_ms = 600_001;
    assert!(AnimationPlayer::new(input, &graph, 0.0).is_err());
}

#[test]
fn protocol_parses_fractional_zoom_defaults_and_exact_limits() {
    let parse = |zoom: &str| {
        serde_json::from_str::<AnimationScenario>(&format!(
            r#"{{"graph_id":"id","steps":[{{"command":{{"type":"camera","zoom":{zoom}}}}}]}}"#
        ))
    };
    for zoom in ["0.15", "1.5", "3"] {
        let input = parse(zoom).unwrap();
        assert!(AnimationPlayer::new(input, &graph(), 0.0).is_ok());
    }
    for zoom in ["0.14999999999", "3.00000001", "1e1000", "\"1.5\"", "true"] {
        assert!(parse(zoom).is_err(), "{zoom}");
    }
    let input: AnimationScenario = serde_json::from_str(
        r#"{"graph_id":"id","steps":[{"command":{"type":"highlight","nodes":["a"]}}]}"#,
    )
    .unwrap();
    let player = AnimationPlayer::new(input, &graph(), 0.0).unwrap();
    assert_eq!(player.end, 1.0);
    let mut input = scenario(AnimationCommand::Clear);
    input.steps = vec![input.steps[0].clone(); 257];
    assert!(AnimationPlayer::new(input, &graph(), 0.0).is_err());
    let mut duplicated = graph();
    duplicated.nodes.push(duplicated.nodes[0].clone());
    let input = scenario(AnimationCommand::Camera {
        node: Some("a".into()),
        zoom: None,
        fit: false,
        duration_ms: 1000,
    });
    assert!(AnimationPlayer::new(input, &duplicated, 0.0).is_err());
}

#[test]
fn marker_progress_uses_route_length_and_handles_loops() {
    let path = [
        Pos2::new(0.0, 0.0),
        Pos2::new(30.0, 0.0),
        Pos2::new(30.0, 70.0),
    ];
    assert_eq!(point_on_path(&path, 0.5), Some(Pos2::new(30.0, 20.0)));
    assert_eq!(point_on_path(&path, 1.0), Some(Pos2::new(30.0, 70.0)));
    let loop_path = [Pos2::ZERO, Pos2::new(10.0, 0.0), Pos2::ZERO];
    assert_eq!(point_on_path(&loop_path, 0.5), Some(Pos2::new(10.0, 0.0)));
    assert!(point_on_path(&[], 0.5).is_none());
}

#[test]
fn scheduled_percentage_flow_draws_the_requested_segment_and_rejects_backwards_ranges() {
    let graph = graph();
    for (reverse, expected) in [(false, 37.5), (true, 62.5)] {
        let input: AnimationScenario = serde_json::from_value(json!({
            "graph_id":"id","steps":[{"command":{
                "type":"flow_percent","edge":0,"from_percent":25.0,"to_percent":75.0,
                "reverse":reverse,"duration_ms":1000,"color":[255,0,0]
            }}]
        }))
        .unwrap();
        let player = AnimationPlayer::new(input, &graph, 0.0).unwrap();
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            player.paint(
                ui.painter(),
                0.25,
                AnimationCanvas {
                    positions: &[Pos2::ZERO, Pos2::new(100.0, 0.0)],
                    sizes: &[Vec2::splat(20.0); 2],
                    paths: &[vec![Pos2::ZERO, Pos2::new(100.0, 0.0)]],
                    zoom: 1.0,
                    offset: Vec2::ZERO,
                },
            );
        });
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,egui::Shape::Circle(circle)
                if circle.center == Pos2::new(expected,0.0) && circle.fill == Color32::RED
        )));
        output.drop_without_applying_deltas();
    }
    for (from_percent, to_percent) in [(80.0, 20.0), (-1.0, 100.0), (0.0, 101.0)] {
        let input = scenario(AnimationCommand::FlowPercent {
            edge: 0,
            from_percent,
            to_percent,
            color: default_color(),
            duration_ms: 1000,
            reverse: false,
        });
        assert!(AnimationPlayer::new(input, &graph, 0.0).is_err());
    }
}

#[test]
fn camera_interpolates_and_highlights_clear_at_scheduled_time() {
    let mut input = scenario(AnimationCommand::Camera {
        node: Some("b".into()),
        zoom: Some(2.0),
        fit: false,
        duration_ms: 1000,
    });
    input.steps.push(AnimationStep {
        at_ms: 500,
        command: AnimationCommand::Clear,
    });
    let mut player = AnimationPlayer::new(input, &graph(), 10.0).unwrap();
    let mut zoom = 1.0;
    let mut pan = Vec2::ZERO;
    player.camera(
        10.25,
        Vec2::new(800.0, 600.0),
        Vec2::new(500.0, 400.0),
        &[Pos2::ZERO, Pos2::new(100.0, 100.0)],
        &mut zoom,
        &mut pan,
    );
    assert!(zoom > 1.0 && zoom < 2.0);
    assert!(pan.x > 0.0);
    assert_eq!(player.visible_steps(10.5).count(), 0);
    assert!(!player.is_running(10.5));
    assert!(!player.is_running(11.0));
}

#[test]
fn visual_effects_are_drawn_and_finish_without_mutating_graph() {
    let mut input = scenario(AnimationCommand::Highlight {
        nodes: vec!["a".into()],
        edges: vec![0],
        color: [10, 20, 30],
        duration_ms: 1000,
        pulse: true,
    });
    input.steps.push(AnimationStep {
        at_ms: 0,
        command: AnimationCommand::Flow {
            edge: 0,
            color: [255, 0, 0],
            duration_ms: 1000,
            reverse: false,
        },
    });
    let player = AnimationPlayer::new(input, &graph(), 0.0).unwrap();
    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        player.paint(
            ui.painter(),
            0.5,
            AnimationCanvas {
                positions: &[Pos2::ZERO, Pos2::new(100.0, 0.0)],
                sizes: &[Vec2::splat(20.0); 2],
                paths: &[vec![Pos2::ZERO, Pos2::new(100.0, 0.0)]],
                zoom: 1.0,
                offset: Vec2::ZERO,
            },
        );
    });
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape, egui::Shape::Circle(circle)
            if circle.center == Pos2::new(50.0,0.0) && circle.fill == Color32::RED
    )));
    output.drop_without_applying_deltas();
    assert!(!player.is_running(1.0));
}

#[test]
fn completed_camera_releases_manual_control_while_other_effects_continue() {
    let mut input = scenario(AnimationCommand::Camera {
        node: Some("b".into()),
        zoom: Some(2.0),
        fit: false,
        duration_ms: 1000,
    });
    input.steps.push(AnimationStep {
        at_ms: 0,
        command: AnimationCommand::Flow {
            edge: 0,
            color: default_color(),
            duration_ms: 5000,
            reverse: false,
        },
    });
    let mut player = AnimationPlayer::new(input, &graph(), 0.0).unwrap();
    let positions = [Pos2::ZERO, Pos2::new(100.0, 100.0)];
    let viewport = Vec2::new(800.0, 600.0);
    let content = Vec2::new(500.0, 400.0);
    let mut zoom = 1.0;
    let mut pan = Vec2::ZERO;
    player.camera(0.0, viewport, content, &positions, &mut zoom, &mut pan);
    player.camera(0.5, viewport, content, &positions, &mut zoom, &mut pan);
    assert_eq!(zoom, 1.5);
    player.camera(1.0, viewport, content, &positions, &mut zoom, &mut pan);
    assert_eq!(zoom, 2.0);
    assert_eq!(pan, Vec2::new(200.0, 100.0));
    zoom = 0.75;
    pan = Vec2::new(12.0, 23.0);
    player.camera(2.0, viewport, content, &positions, &mut zoom, &mut pan);
    assert_eq!(zoom, 0.75);
    assert_eq!(pan, Vec2::new(12.0, 23.0));
    assert!(player.is_running(2.0));
}

fn raw(server: &AnimationApiServer, headers: String, body: String) -> thread::JoinHandle<String> {
    let address = server.address();
    thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream, "{headers}\r\n{body}").unwrap();
        let mut output = String::new();
        stream.read_to_string(&mut output).unwrap();
        output
    })
}

fn header(server: &AnimationApiServer, method: &str, path: &str) -> String {
    format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n",
        server.address(),
        server.token()
    )
}

fn post_json(server: &AnimationApiServer, path: &str, value: Value) -> thread::JoinHandle<String> {
    let body = value.to_string();
    raw(
        server,
        format!(
            "{}Content-Type: application/json\r\nContent-Length: {}\r\n",
            header(server, "POST", path),
            body.len()
        ),
        body,
    )
}

#[test]
fn http_api_requires_auth_rejects_browser_requests_and_enforces_limits() {
    let server = AnimationApiServer::start(0).unwrap();
    assert!(server.address().ip().is_loopback());
    assert_eq!(server.token().len(), 64);
    for (headers, status) in [
        (
            format!("GET /v1/graph HTTP/1.1\r\nHost: {}\r\n", server.address()),
            401,
        ),
        (
            format!(
                "{}Origin: http://example.com\r\n",
                header(&server, "GET", "/v1/graph")
            ),
            403,
        ),
        (
            format!(
                "{}Content-Length: 65537\r\n",
                header(&server, "POST", "/v1/animation")
            ),
            413,
        ),
        (
            format!(
                "{}Content-Length: 1\r\nContent-Length: 1\r\n",
                header(&server, "POST", "/v1/animation")
            ),
            400,
        ),
        (header(&server, "GET", "/unknown"), 404),
    ] {
        let reply = raw(&server, headers, String::new()).join().unwrap();
        assert!(reply.starts_with(&format!("HTTP/1.1 {status}")), "{reply}");
        assert!(server.try_request().unwrap().is_none());
    }
}

#[test]
fn http_api_round_trip_delivers_commands_and_returns_gui_errors() {
    use super::server::ServerOptions;

    let wake_count = Arc::new(AtomicUsize::new(0));
    let wake_signal = Arc::clone(&wake_count);
    let server = AnimationApiServer::start_with_waker(
        ServerOptions {
            port: 0,
            ..Default::default()
        },
        move || {
            wake_signal.fetch_add(1, Ordering::Relaxed);
        },
    )
    .unwrap();
    let body = serde_json::to_string(&scenario(AnimationCommand::Flow {
        edge: 0,
        color: default_color(),
        duration_ms: 1000,
        reverse: false,
    }))
    .unwrap();
    let client = raw(
        &server,
        format!(
            "{}Content-Type: application/json\r\nContent-Length: {}\r\n",
            header(&server, "POST", "/v1/animation"),
            body.len()
        ),
        body,
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let request = loop {
        if let Some(request) = server.try_request().unwrap() {
            break request;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    };
    assert!(wake_count.load(Ordering::Relaxed) > 0);
    assert!(matches!(request.operation, ApiOperation::Play(_)));
    request
        .reply
        .send(ApiReply {
            status: 409,
            value: json!({"error":"stale graph"}),
        })
        .unwrap();
    let reply = client.join().unwrap();
    assert!(reply.starts_with("HTTP/1.1 409"));
    assert!(reply.contains("stale graph"));
}

#[test]
fn api_token_rotation_revokes_old_credentials_without_restarting_listener() {
    let mut server = AnimationApiServer::start(0).unwrap();
    let address = server.address();
    let old_header = header(&server, "GET", "/unknown");
    let old_token = server.token().to_owned();
    for invalid in [
        String::new(),
        "short".into(),
        "contains space!!".into(),
        "contains\nnewline".into(),
        "токен".repeat(16),
        "x".repeat(257),
    ] {
        assert!(server.set_token(invalid).is_err());
        assert_eq!(server.token(), old_token);
    }
    server
        .set_token("custom-token-for-test-123".into())
        .unwrap();
    assert_eq!(server.address(), address);
    assert!(
        raw(&server, old_header, String::new())
            .join()
            .unwrap()
            .starts_with("HTTP/1.1 401")
    );
    let custom_header = header(&server, "GET", "/unknown");
    assert!(
        raw(&server, custom_header.clone(), String::new())
            .join()
            .unwrap()
            .starts_with("HTTP/1.1 404")
    );
    server.regenerate_token().unwrap();
    assert_eq!(server.token().len(), 64);
    assert_ne!(server.token(), old_token);
    assert_eq!(server.address(), address);
    assert!(
        raw(&server, custom_header, String::new())
            .join()
            .unwrap()
            .starts_with("HTTP/1.1 401")
    );
    assert!(
        raw(&server, header(&server, "GET", "/unknown"), String::new())
            .join()
            .unwrap()
            .starts_with("HTTP/1.1 404")
    );
    for valid in ["a".repeat(16), "a".repeat(256)] {
        server.set_token(valid).unwrap();
    }
}

fn pump_reply(
    app: &mut StructViewApp,
    ctx: &egui::Context,
    client: thread::JoinHandle<String>,
) -> (u16, Value) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !client.is_finished() {
        ctx.run_ui(
            egui::RawInput {
                time: Some(ctx.input(|input| input.time).max(10.0)),
                ..Default::default()
            },
            |ui| {
                app.graph_calculation.poll(ui.ctx());
                app.poll_graph_animation_api(ui.ctx());
            },
        )
        .drop_without_applying_deltas();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }

    let response = client.join().unwrap();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = response.split_once("\r\n\r\n").unwrap().1;
    (status, serde_json::from_str(body).unwrap())
}

fn realtime_update(commands: Vec<RealtimeCommand>) -> RealtimeUpdate {
    RealtimeUpdate {
        graph_id: "id".into(),
        commands,
    }
}

fn marker(id: &str, edge: usize, percent: f64) -> RealtimeCommand {
    RealtimeCommand::Marker {
        id: id.into(),
        edge,
        percent,
        smoothing_ms: 300,
        color: default_color(),
        reverse: false,
        label: None,
        stage: None,
    }
}

#[test]
fn realtime_runs_keep_independent_metadata_and_separate_markers_on_shared_edges() {
    let graph = graph();
    let commands = vec![
        RealtimeCommand::Marker {
            id: "fulfillment".into(),
            edge: 0,
            percent: 50.0,
            smoothing_ms: 16,
            color: [255, 0, 0],
            reverse: false,
            label: Some("Order fulfillment".into()),
            stage: Some("Reserving stock".into()),
        },
        RealtimeCommand::Marker {
            id: "fraud-check".into(),
            edge: 0,
            percent: 50.0,
            smoothing_ms: 16,
            color: [0, 0, 255],
            reverse: false,
            label: Some("Fraud review".into()),
            stage: Some("Checking payment".into()),
        },
        RealtimeCommand::Element {
            id: "fulfillment".into(),
            nodes: vec!["a".into()],
            edges: vec![0],
            status: Some(ElementStatus::Running),
            color: None,
            pulse: None,
            label: Some("Order fulfillment".into()),
            stage: Some("Reserving stock".into()),
        },
        RealtimeCommand::Element {
            id: "fraud-check".into(),
            nodes: vec!["a".into()],
            edges: vec![0],
            status: Some(ElementStatus::Running),
            color: None,
            pulse: None,
            label: Some("Fraud review".into()),
            stage: Some("Checking payment".into()),
        },
    ];
    let mut state =
        realtime::RealtimeState::apply(None, realtime_update(commands), &graph, 0.0).unwrap();
    state.advance(1.0);
    state = realtime::RealtimeState::apply(
        Some(state),
        realtime_update(vec![RealtimeCommand::Marker {
            id: "fulfillment".into(),
            edge: 0,
            percent: 50.0,
            smoothing_ms: 16,
            color: [255, 0, 0],
            reverse: false,
            label: None,
            stage: None,
        }]),
        &graph,
        1.0,
    )
    .unwrap();
    let (statuses, markers) = state.statuses_for_edge(0);
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].1.as_deref(), Some("Fraud review"));
    assert_eq!(statuses[0].2.as_deref(), Some("Checking payment"));
    assert_eq!(statuses[1].1.as_deref(), Some("Order fulfillment"));
    assert_eq!(statuses[1].2.as_deref(), Some("Reserving stock"));
    assert_eq!(markers.len(), 2);
    let descriptor = state.descriptor(1.0, &graph);
    assert_eq!(descriptor["markers"][0]["label"], "Fraud review");
    assert_eq!(descriptor["markers"][1]["stage"], "Reserving stock");

    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        state.paint(
            ui.painter(),
            1.0,
            AnimationCanvas {
                positions: &[Pos2::ZERO, Pos2::new(100.0, 0.0)],
                sizes: &[Vec2::splat(20.0); 2],
                paths: &[vec![Pos2::ZERO, Pos2::new(100.0, 0.0)]],
                zoom: 1.0,
                offset: Vec2::ZERO,
            },
            super::super::i18n::Locale::English,
        );
    });
    let marker_centers = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle)
                if circle.fill == Color32::RED || circle.fill == Color32::BLUE =>
            {
                Some(circle.center)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(marker_centers.len(), 2);
    assert!((marker_centers[0].distance(marker_centers[1]) - 16.0).abs() < 0.1);
    output.drop_without_applying_deltas();
}

#[test]
fn realtime_percent_updates_preserve_position_follow_maximum_and_finish_exactly() {
    let graph = graph();
    let mut state = realtime::RealtimeState::apply(
        None,
        realtime_update(vec![marker("job", 0, 40.0)]),
        &graph,
        0.0,
    )
    .unwrap();
    assert_eq!(state.descriptor(0.0, &graph)["markers"][0]["percent"], 0.0);
    let value = state.descriptor(0.3, &graph);
    let current = value["markers"][0]["percent"].as_f64().unwrap();
    assert!((current - 40.0 * (1.0 - (-1.0_f64).exp())).abs() < 1e-8);
    state = realtime::RealtimeState::apply(
        Some(state),
        realtime_update(vec![marker("job", 0, 90.0)]),
        &graph,
        0.3,
    )
    .unwrap();
    assert_eq!(
        state.descriptor(0.3, &graph)["markers"][0]["percent"]
            .as_f64()
            .unwrap(),
        current,
        "Retargeting must not move the marker on acceptance"
    );
    let mut last = current;
    for index in 1..=60 {
        let now = 0.3 + f64::from(index) * 0.001;
        state = realtime::RealtimeState::apply(
            Some(state),
            realtime_update(vec![marker(
                "job",
                0,
                if index % 2 == 0 { 100.0 } else { 10.0 },
            )]),
            &graph,
            now,
        )
        .unwrap();
        let value = state.descriptor(now, &graph);
        let position = value["markers"][0]["percent"].as_f64().unwrap();
        assert!(position >= last && position < 100.0);
        assert!(
            position - last < 1.0,
            "Frequent updates must not cause jumps"
        );
        if index >= 2 {
            assert_eq!(value["markers"][0]["target_percent"], 100.0);
        }
        last = position;
    }
    state.advance(4.0);
    assert_eq!(
        state.descriptor(4.0, &graph)["markers"][0]["percent"],
        100.0
    );
    assert!(
        !state.needs_repaint(),
        "Settled persistent markers must not repaint forever"
    );
    state = realtime::RealtimeState::apply(
        Some(state),
        realtime_update(vec![marker("job", 0, 0.0)]),
        &graph,
        4.0,
    )
    .unwrap();
    assert_eq!(
        state.descriptor(4.0, &graph)["markers"][0]["percent"],
        100.0
    );
    state = realtime::RealtimeState::apply(
        Some(state),
        realtime_update(vec![
            RealtimeCommand::RemoveMarker { id: "job".into() },
            marker("job", 1, 25.0),
        ]),
        &graph,
        4.0,
    )
    .unwrap();
    let value = state.descriptor(4.0, &graph);
    assert_eq!(value["markers"][0]["percent"], 0.0);
    assert_eq!(value["markers"][0]["edge"], 1);
}

#[test]
fn realtime_protocol_validates_percent_limits_targets_and_resource_caps() {
    let metadata: RealtimeUpdate = serde_json::from_value(json!({
        "graph_id":"id",
        "commands":[
            {"type":"marker","id":"process-1","edge":0,"percent":50,
             "label":"Order processing","stage":"Charging payment"},
            {"type":"element","id":"process-1","nodes":["a"],"edges":[0],
             "status":"running","label":"Order processing","stage":"Charging payment"}
        ]
    }))
    .unwrap();
    let mut metadata_state = realtime::RealtimeState::apply(None, metadata, &graph(), 0.0).unwrap();
    let metadata_descriptor = metadata_state.descriptor(0.0, &graph());
    assert_eq!(
        metadata_descriptor["markers"][0]["label"],
        "Order processing"
    );
    assert_eq!(
        metadata_descriptor["elements"][0]["stage"],
        "Charging payment"
    );

    for value in ["0", "12.5", "100"] {
        let input: RealtimeUpdate = serde_json::from_str(&format!(
                r#"{{"graph_id":"id","commands":[{{"type":"marker","id":"m","edge":0,"percent":{value}}}]}}"#
            )).unwrap();
        assert!(realtime::RealtimeState::apply(None, input, &graph(), 0.0).is_ok());
    }
    for value in ["-1", "100.000001", "1e1000", "\"42\"", "null"] {
        assert!(serde_json::from_str::<RealtimeUpdate>(&format!(
                r#"{{"graph_id":"id","commands":[{{"type":"marker","id":"m","edge":0,"percent":{value}}}]}}"#
            )).is_err(), "{value}");
    }
    assert!(
        serde_json::from_value::<RealtimeUpdate>(json!({
            "graph_id":"id","commands":[{"type":"marker","id":"m","edge":0,"percent":0,"move":true}]
        }))
        .is_err()
    );
    for command in [
        marker("", 0, 10.0),
        marker("m", 999, 10.0),
        marker("m", 0, f64::NAN),
        RealtimeCommand::RemoveMarker {
            id: "unknown".into(),
        },
        RealtimeCommand::Element {
            id: "s".into(),
            nodes: vec!["missing".into()],
            edges: vec![],
            status: None,
            color: None,
            pulse: None,
            label: None,
            stage: None,
        },
        RealtimeCommand::Element {
            id: "s".into(),
            nodes: vec!["a".into()],
            edges: vec![],
            status: None,
            color: None,
            pulse: None,
            label: Some("  ".into()),
            stage: None,
        },
        RealtimeCommand::Marker {
            id: "m".into(),
            edge: 0,
            percent: 10.0,
            smoothing_ms: 300,
            color: default_color(),
            reverse: false,
            label: None,
            stage: Some("x".repeat(257)),
        },
    ] {
        assert!(
            realtime::RealtimeState::apply(None, realtime_update(vec![command]), &graph(), 0.0)
                .is_err()
        );
    }
    for smoothing_ms in [0, 15, 10_001] {
        let command = RealtimeCommand::Marker {
            id: "m".into(),
            edge: 0,
            percent: 50.0,
            smoothing_ms,
            color: default_color(),
            reverse: false,
            label: None,
            stage: None,
        };
        assert!(
            realtime::RealtimeState::apply(None, realtime_update(vec![command]), &graph(), 0.0)
                .is_err()
        );
    }
    let commands = (0..256)
        .map(|index| marker(&format!("m{index}"), 0, 50.0))
        .collect();
    let state =
        realtime::RealtimeState::apply(None, realtime_update(commands), &graph(), 0.0).unwrap();
    assert!(
        realtime::RealtimeState::apply(
            Some(state.clone()),
            realtime_update(vec![marker("overflow", 0, 50.0)]),
            &graph(),
            0.0
        )
        .is_err()
    );
    assert!(
        realtime::RealtimeState::apply(
            Some(state),
            realtime_update(vec![marker("m0", 1, 50.0)]),
            &graph(),
            0.0
        )
        .is_err()
    );
}

#[test]
fn realtime_status_appearance_markers_and_persistent_clear_are_independent_of_scenario() {
    let graph = graph();
    let mut state = realtime::RealtimeState::apply(
        None,
        realtime_update(vec![
            RealtimeCommand::Element {
                id: "status".into(),
                nodes: vec!["a".into()],
                edges: vec![0],
                status: Some(ElementStatus::Running),
                color: None,
                pulse: None,
                label: None,
                stage: None,
            },
            RealtimeCommand::Element {
                id: "waiting".into(),
                nodes: vec!["a".into()],
                edges: vec![],
                status: Some(ElementStatus::Waiting),
                color: None,
                pulse: None,
                label: None,
                stage: None,
            },
            RealtimeCommand::Marker {
                id: "reverse".into(),
                edge: 0,
                percent: 25.0,
                smoothing_ms: 300,
                color: [255, 0, 0],
                reverse: true,
                label: None,
                stage: None,
            },
        ]),
        &graph,
        0.0,
    )
    .unwrap();
    assert_eq!(
        state.statuses_for_node(0),
        vec![
            ("status".into(), None, None, Some(ElementStatus::Running)),
            ("waiting".into(), None, None, Some(ElementStatus::Waiting))
        ]
    );
    assert!(state.statuses_for_node(2).is_empty());
    let (edge_statuses, progress) = state.statuses_for_edge(0);
    assert_eq!(
        edge_statuses,
        vec![("status".into(), None, None, Some(ElementStatus::Running))]
    );
    assert_eq!(progress, vec![("reverse".into(), None, None, 0.0, 25.0)]);
    let value = state.descriptor(4.0, &graph);
    assert_eq!(value["elements"][0]["status"], "running");
    assert_eq!(value["elements"][0]["nodes"][0], "a");
    assert_eq!(value["elements"][0]["pulse"], true);
    assert!(state.needs_repaint());
    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        state.paint(
            ui.painter(),
            4.0,
            AnimationCanvas {
                positions: &[Pos2::ZERO, Pos2::new(100.0, 0.0)],
                sizes: &[Vec2::splat(20.0); 2],
                paths: &[vec![Pos2::ZERO, Pos2::new(100.0, 0.0)]],
                zoom: 1.0,
                offset: Vec2::ZERO,
            },
            super::super::i18n::Locale::English,
        );
    });
    assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,egui::Shape::Circle(circle) if circle.center == Pos2::new(75.0,0.0) && circle.fill == Color32::RED
        )));
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape,egui::Shape::Text(text) if text.galley.text() == "Running"
    )));
    let badge_positions = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if matches!(text.galley.text(), "Running" | "Waiting") => {
                Some(text.pos.y)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(badge_positions.len(), 2);
    assert_ne!(badge_positions[0], badge_positions[1]);
    output.drop_without_applying_deltas();
    realtime::store(&ctx, state.clone());
    store_player(
        &ctx,
        AnimationPlayer::new(
            scenario(AnimationCommand::Flow {
                edge: 0,
                color: default_color(),
                duration_ms: 1000,
                reverse: false,
            }),
            &graph,
            0.0,
        )
        .unwrap(),
    );
    stop_scenario(&ctx);
    assert!(realtime::state(&ctx).is_some());
    for status in [
        ElementStatus::Waiting,
        ElementStatus::Success,
        ElementStatus::Error,
    ] {
        state = realtime::RealtimeState::apply(
            Some(state),
            realtime_update(vec![RealtimeCommand::Element {
                id: "status".into(),
                nodes: vec!["a".into()],
                edges: vec![],
                status: Some(status),
                color: None,
                pulse: Some(false),
                label: None,
                stage: None,
            }]),
            &graph,
            4.0,
        )
        .unwrap();
        assert!(!state.needs_repaint());
    }
    state = realtime::RealtimeState::apply(
        Some(state),
        realtime_update(vec![RealtimeCommand::Clear]),
        &graph,
        4.0,
    )
    .unwrap();
    assert_eq!(state.descriptor(4.0, &graph)["markers"], json!([]));
    realtime::store(&ctx, state);
    stop(&ctx);
    assert!(realtime::state(&ctx).is_none());
}

fn reply_to_next(server: &AnimationApiServer, status: u16, value: Value) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(request) = server.try_request().unwrap() {
            request.reply.send(ApiReply { status, value }).unwrap();
            return;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn configured_http_host_network_opt_in_and_slow_clients_are_bounded() {
    use super::server::ServerOptions;
    assert!(
        AnimationApiServer::start_with(ServerOptions {
            bind: "0.0.0.0".parse().unwrap(),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        AnimationApiServer::start_with(ServerOptions {
            bind: "0.0.0.0".parse().unwrap(),
            allow_network: true,
            ..Default::default()
        })
        .is_err()
    );
    for host in [
        "http://example.com",
        "example.com:80",
        "bad/host",
        "0.0.0.0",
    ] {
        assert!(
            AnimationApiServer::start_with(ServerOptions {
                host: Some(host.into()),
                ..Default::default()
            })
            .is_err()
        );
    }
    let server = AnimationApiServer::start_with(ServerOptions {
        host: Some("api.example.test".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        server.url(),
        format!("http://api.example.test:{}", server.address().port())
    );
    let mut slow = TcpStream::connect(server.address()).unwrap();
    slow.write_all(b"GET ").unwrap();
    thread::sleep(Duration::from_millis(30));
    let start = Instant::now();
    let client = raw(
        &server,
        format!(
            "GET /v1/realtime HTTP/1.1\r\nHost: api.example.test:{}\r\nAuthorization: Bearer {}\r\n",
            server.address().port(),
            server.token()
        ),
        String::new(),
    );
    reply_to_next(&server, 200, json!({"markers":[]}));
    assert!(client.join().unwrap().starts_with("HTTP/1.1 200"));
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "One slow connection must not block other clients"
    );
    let client = raw(
        &server,
        format!(
            "GET /v1/graph HTTP/1.1\r\nHost: unconfigured.example:{}\r\nAuthorization: Bearer {}\r\n",
            server.address().port(),
            server.token()
        ),
        String::new(),
    );
    assert!(client.join().unwrap().starts_with("HTTP/1.1 403"));
    drop(slow);
}

#[test]
fn api_client_limit_returns_explicit_error_and_shutdown_releases_listener() {
    let server = AnimationApiServer::start(0).unwrap();
    let address = server.address();
    let clients = (0..8)
        .map(|_| {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(b"GET ").unwrap();
            stream
        })
        .collect::<Vec<_>>();
    thread::sleep(Duration::from_millis(100));
    let client = raw(&server, header(&server, "GET", "/v1/graph"), String::new());
    assert!(client.join().unwrap().starts_with("HTTP/1.1 503"));
    let started = Instant::now();
    drop(server);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "Shutdown must be bounded by client deadlines"
    );
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err());
    drop(clients);
}

#[test]
fn api_settings_show_network_and_tls_fields_and_wire_selected_configuration() {
    use super::super::i18n::Locale;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    app.visualization = VisualizationMode::Graph;
    app.graph_animation_window_open = true;
    let render = |app: &mut StructViewApp, events| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1100.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.show_graph_animation_window(&ctx),
        )
    };
    render(&mut app, vec![]).drop_without_applying_deltas();
    render(&mut app, vec![]).drop_without_applying_deltas();
    let output = render(&mut app, vec![]);
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| {
            node.label()
                .is_some_and(|label| label.starts_with("Graph animation API"))
        })
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let pos = Pos2::new(
        (bounds.x0 + bounds.x1) as f32 * 0.5,
        (bounds.y0 + bounds.y1) as f32 * 0.5,
    );
    output.drop_without_applying_deltas();
    let events = vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ];
    render(&mut app, events).drop_without_applying_deltas();
    app.graph_animation.https = true;
    app.graph_animation.network = true;
    let output = render(&mut app, vec![]);
    let mut labels = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_string))
        .collect::<Vec<_>>();
    labels.extend(output.shapes.iter().filter_map(|shape| match &shape.shape {
        egui::Shape::Text(text) => Some(text.galley.text().to_string()),
        _ => None,
    }));
    output.drop_without_applying_deltas();
    for key in [
        TextKey::GraphApiBind,
        TextKey::GraphApiHost,
        TextKey::GraphApiCertificate,
        TextKey::GraphApiPrivateKey,
    ] {
        assert!(
            labels.iter().any(|label| label == app.locale.text(key)),
            "{key:?}: {labels:?}"
        );
    }
    app.graph_animation.bind = "::1".into();
    app.graph_animation.host = "graph.example.test".into();
    app.graph_animation.port = 8443;
    app.graph_animation.certificate = r"C:\certs\chain.pem".into();
    app.graph_animation.private_key = r"C:\certs\key.pem".into();
    let options = app.graph_animation.server_options().unwrap();
    assert!(options.bind.is_loopback());
    assert_eq!(options.port, 8443);
    assert_eq!(options.host.as_deref(), Some("graph.example.test"));
    assert!(options.allow_network);
    let tls = options.tls.unwrap();
    assert_eq!(
        tls.certificate,
        std::path::PathBuf::from(r"C:\certs\chain.pem")
    );
    app.graph_animation.bind = "not an IP".into();
    assert!(app.graph_animation.server_options().is_err());
    app.graph_animation.server = Some(AnimationApiServer::start(0).unwrap());
    let output = render(&mut app, vec![]);
    let labels = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for key in [
        TextKey::GraphApiActiveToken,
        TextKey::GraphApiConnections,
        TextKey::GraphApiQueue,
        TextKey::GraphApiSuccessfulRequests,
        TextKey::GraphApiFailedRequests,
        TextKey::GraphApiRecentRequests,
        TextKey::GraphApiNoRequests,
    ] {
        assert!(
            labels
                .iter()
                .any(|label| label.contains(app.locale.text(key))),
            "{key:?}: {labels:?}"
        );
    }
    output.drop_without_applying_deltas();
    app.graph_animation.server = None;
}

#[test]
fn https_validates_certificate_configuration_and_serves_authenticated_json() {
    use super::server::{ServerOptions, TlsFiles};
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let certificate = dir.path().join("server.pem");
    let private_key = dir.path().join("server-key.pem");
    let generated = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    std::fs::write(&certificate, generated.cert.pem()).unwrap();
    std::fs::write(&private_key, generated.signing_key.serialize_pem()).unwrap();
    let options = || ServerOptions {
        tls: Some(TlsFiles {
            certificate: certificate.clone(),
            private_key: private_key.clone(),
        }),
        ..Default::default()
    };
    let server = AnimationApiServer::start_with(options()).unwrap();
    assert!(server.url().starts_with("https://"));
    let mut roots = rustls::RootCertStore::empty();
    roots.add(generated.cert.der().clone()).unwrap();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    let config = Arc::new(config);
    let tls_client = |request: String| {
        let config = Arc::clone(&config);
        let address = server.address();
        thread::spawn(move || {
            let tcp = TcpStream::connect(address).unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let connection =
                rustls::ClientConnection::new(config, "localhost".try_into().unwrap()).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, tcp);
            stream.write_all(request.as_bytes()).unwrap();
            stream.flush().unwrap();
            let mut reply = String::new();
            stream.read_to_string(&mut reply).unwrap();
            reply
        })
    };
    let client = tls_client(format!(
        "GET /v1/graph HTTP/1.1\r\nHost: {}\r\n\r\n",
        server.address()
    ));
    assert!(client.join().unwrap().starts_with("HTTP/1.1 401"));
    let body =
        r#"{"graph_id":"id","commands":[{"type":"marker","id":"m","edge":0,"percent":12.5}]}"#;
    let client = tls_client(format!(
        "{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        header(&server, "POST", "/v1/realtime"),
        body.len()
    ));
    reply_to_next(&server, 200, json!({"accepted":true}));
    let response = client.join().unwrap();
    assert!(
        response.starts_with("HTTP/1.1 200") && response.contains(r#""accepted":true"#),
        "{response}"
    );
    let mut slow_clients = (0..8)
        .map(|_| {
            let tcp = TcpStream::connect(server.address()).unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let connection =
                rustls::ClientConnection::new(Arc::clone(&config), "localhost".try_into().unwrap())
                    .unwrap();
            let mut stream = rustls::StreamOwned::new(connection, tcp);
            stream.write_all(b"GET ").unwrap();
            stream.flush().unwrap();
            stream
        })
        .collect::<Vec<_>>();
    thread::sleep(Duration::from_millis(100));
    let rejected = tls_client(format!(
        "GET /v1/graph HTTP/1.1\r\nHost: {}\r\n\r\n",
        server.address()
    ));
    assert!(
        rejected.join().unwrap().starts_with("HTTP/1.1 503"),
        "TLS clients over the limit must receive an HTTP error response"
    );
    slow_clients.clear();
    let stats = server.stats().unwrap();
    assert!(stats.failed_requests >= 1);
    assert!(stats.successful_requests >= 1);
    assert!(stats.recent_requests.iter().any(|request| {
        request.endpoint == "POST /v1/realtime"
            && request.summary == "commands: 1"
            && request.status == 200
    }));
    drop(server);
    std::fs::write(&certificate, "not a certificate").unwrap();
    assert!(AnimationApiServer::start_with(options()).is_err());
    std::fs::write(&certificate, generated.cert.pem()).unwrap();
    let wrong = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    std::fs::write(&private_key, wrong.signing_key.serialize_pem()).unwrap();
    assert!(AnimationApiServer::start_with(options()).is_err());
}
#[test]
fn application_http_commands_animate_without_editing_and_cancel_on_view_change() {
    use super::server::ServerOptions;

    let ctx = egui::Context::default();
    let mut app = StructViewApp::default();
    let root =
        parse_json(r#"{"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b"}]}"#)
            .unwrap();
    let before = struct_view_core::parser::node_to_value(&root).unwrap();
    app.root = Some(root.clone());
    app.visualization = VisualizationMode::Graph;
    app.graph_calculation.ensure_started(
        &root,
        app.graph_routing_workers,
        app.graph_routing_backend,
        &ctx,
    );
    app.graph_animation.server = Some(
        AnimationApiServer::start_with_waker(
            ServerOptions {
                port: 0,
                ..Default::default()
            },
            {
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            },
        )
        .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.graph_calculation.result().is_none() {
        app.graph_calculation.poll(&ctx);
        assert!(
            app.graph_calculation.error().is_none(),
            "{:?}",
            app.graph_calculation.error()
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(server, header(server, "GET", "/v1/graph"), String::new());
    let (status, descriptor) = pump_reply(&mut app, &ctx, client);
    assert_eq!(status, 200);
    assert_eq!(descriptor["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(descriptor["edges"][0]["index"], 0);
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(
        server,
        header(server, "GET", "/v1/capabilities"),
        String::new(),
    );
    let (_, capabilities) = pump_reply(&mut app, &ctx, client);
    assert_eq!(capabilities["limits"]["jobs"], 64);
    let body = json!({"graph_id":descriptor["graph_id"],"commands":[
        {"type":"upsert","id":"order-42","label":"Order #42","stage":"Charging",
            "status":"running","nodes":["a"],"edge":0,"percent":45},
        {"type":"upsert","id":"review-42","label":"Risk review #42","stage":"Checking",
            "status":"waiting","nodes":["a"],"edge":0,"percent":20}
    ]});
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(server, "/v1/jobs", body.clone());
    let (status, reply) = pump_reply(&mut app, &ctx, client);
    assert_eq!(status, 200, "{reply}");
    assert_eq!(reply["job_count"], 2);
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(server, header(server, "GET", "/v1/jobs"), String::new());
    let (_, job_list) = pump_reply(&mut app, &ctx, client);
    assert_eq!(job_list["jobs"].as_array().unwrap().len(), 2);
    let mut invalid = body.clone();
    invalid["commands"][0]["stage"] = json!("Changed");
    invalid["commands"][1]["edge"] = json!(999);
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(server, "/v1/jobs", invalid);
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 400);
    assert_eq!(
        jobs::state(&ctx).unwrap().jobs["order-42"].stage,
        "Charging"
    );
    let mut stale_jobs = body;
    stale_jobs["graph_id"] = json!("stale");
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(server, "/v1/jobs", stale_jobs);
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 409);
    let mut input = scenario(AnimationCommand::Flow {
        edge: 0,
        color: default_color(),
        duration_ms: 1000,
        reverse: false,
    });
    input.steps.push(AnimationStep {
        at_ms: 0,
        command: AnimationCommand::Camera {
            node: Some(
                descriptor["edges"][0]["source"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            ),
            zoom: Some(1.5),
            fit: false,
            duration_ms: 1000,
        },
    });
    input.graph_id = descriptor["graph_id"].as_str().unwrap().to_string();
    let body = serde_json::to_string(&input).unwrap();
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(
        server,
        format!(
            "{}Content-Type: application/json\r\nContent-Length: {}\r\n",
            header(server, "POST", "/v1/animation"),
            body.len()
        ),
        body,
    );
    let (status, reply) = pump_reply(&mut app, &ctx, client);
    assert_eq!(status, 200, "{reply}");
    assert!(player(&ctx).unwrap().is_running(10.1));
    assert_eq!(
        struct_view_core::parser::node_to_value(app.root.as_ref().unwrap()).unwrap(),
        before
    );
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(
        server,
        header(server, "GET", "/v1/animation"),
        String::new(),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).1["running"], true);
    let mut stale = serde_json::to_value(&input).unwrap();
    stale["graph_id"] = json!("stale");
    let body = stale.to_string();
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(
        server,
        format!(
            "{}Content-Type: application/json\r\nContent-Length: {}\r\n",
            header(server, "POST", "/v1/animation"),
            body.len()
        ),
        body,
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 409);
    assert!(
        player(&ctx).is_some(),
        "Rejected scenario must not replace the active one"
    );
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(
        server,
        "/v1/realtime",
        json!({
            "graph_id":descriptor["graph_id"],"commands":[
                {"type":"marker","id":"live-job","edge":0,"percent":75,"color":[255,0,0]},
                {"type":"element","id":"job-status","nodes":[descriptor["nodes"][0]["id"]],"status":"running"}
            ]
        }),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 200);
    assert!(
        player(&ctx).is_some(),
        "Realtime updates must not replace scheduled playback"
    );
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(
        server,
        "/v1/realtime",
        json!({
            "graph_id":descriptor["graph_id"],"commands":[
                {"type":"marker","id":"live-job","edge":0,"percent":100},
                {"type":"element","id":"invalid","nodes":["missing"],"status":"success"}
            ]
        }),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 400);
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(server, header(server, "GET", "/v1/realtime"), String::new());
    let (status, live) = pump_reply(&mut app, &ctx, client);
    assert_eq!(status, 200);
    assert_eq!(
        live["markers"][0]["target_percent"].as_f64(),
        Some(75.0),
        "Rejected batches must not partially apply"
    );
    assert_eq!(live["elements"][0]["status"], "running");
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(
        server,
        "/v1/realtime",
        json!({
            "graph_id":"stale","commands":[{"type":"clear"}]
        }),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 409);
    let output = ctx.run_ui(
        egui::RawInput {
            time: Some(10.5),
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| app.show_central_panel(ui),
    );
    assert!(
        output.shapes.iter().any(|shape| matches!(
            &shape.shape,egui::Shape::Circle(circle) if circle.fill == Color32::from_rgb(255,180,40)
        )),
        "HTTP flow command must paint a marker on the real graph canvas"
    );
    let painted_text = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(painted_text.contains("Order #42"));
    assert!(painted_text.contains("Charging"));
    assert!(painted_text.contains("Risk review #42"));
    output.drop_without_applying_deltas();
    assert!(
        player(&ctx).unwrap().camera.is_some(),
        "Canvas must persist camera interpolation state"
    );
    assert!(realtime::state(&ctx).is_some());
    let mut legacy_state = realtime::state(&ctx).unwrap();
    assert_eq!(
        legacy_state.descriptor(10.5, &app.graph_calculation.result().unwrap().graph)["markers"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "Display merging must not persist jobs into the legacy layer"
    );
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(
        server,
        "/v1/jobs",
        json!({
            "graph_id":descriptor["graph_id"],"commands":[{"type":"remove","id":"review-42"}]
        }),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).1["job_count"], 1);
    assert!(jobs::state(&ctx).unwrap().jobs.contains_key("order-42"));
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = post_json(
        server,
        "/v1/jobs",
        json!({
            "graph_id":descriptor["graph_id"],"commands":[{"type":"clear"}]
        }),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).1["job_count"], 0);
    assert!(realtime::state(&ctx).is_some());
    assert!(player(&ctx).is_some());
    assert_eq!(
        struct_view_core::parser::node_to_value(app.root.as_ref().unwrap()).unwrap(),
        before
    );
    app.visualization = VisualizationMode::Tree;
    app.poll_graph_animation_api(&ctx);
    assert!(player(&ctx).is_none());
    assert!(realtime::state(&ctx).is_none());
    assert!(jobs::state(&ctx).is_none());
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(
        server,
        header(server, "GET", "/v1/capabilities"),
        String::new(),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 200);
    let server = app.graph_animation.server.as_ref().unwrap();
    let client = raw(server, header(server, "GET", "/v1/graph"), String::new());
    assert_eq!(pump_reply(&mut app, &ctx, client).0, 409);
    let server = app.graph_animation.server.as_ref().unwrap();
    let address = server.address();
    let client = raw(
        server,
        header(server, "POST", "/v1/animation/stop"),
        String::new(),
    );
    assert_eq!(pump_reply(&mut app, &ctx, client).1["stopped"], true);
    app.visualization = VisualizationMode::Graph;
    let active =
        AnimationPlayer::new(input, &app.graph_calculation.result().unwrap().graph, 10.0).unwrap();
    store_player(&ctx, active);
    let server = app.graph_animation.server.as_ref().unwrap();
    let token = server.token().to_string();
    let client = raw(
        server,
        header(server, "POST", "/v1/animation/stop"),
        String::new(),
    );
    let response = client.join().unwrap();
    assert!(response.starts_with("HTTP/1.1 504"), "{response}");
    app.poll_graph_animation_api(&ctx);
    assert!(
        player(&ctx).is_some(),
        "Expired stop command must not execute later"
    );
    app.graph_calculation = super::super::views::GraphCalculationState::default();
    app.poll_graph_animation_api(&ctx);
    assert!(
        player(&ctx).is_none(),
        "Invalidating the graph must cancel playback"
    );
    app.graph_animation.server = None;
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err());
    let replacement = AnimationApiServer::start_with_waker(
        ServerOptions {
            port: 0,
            ..Default::default()
        },
        move || ctx.request_repaint(),
    )
    .unwrap();
    assert_ne!(token, replacement.token());
}
