use super::*;
use crate::app::i18n::Locale;

fn graph() -> RelationshipGraph {
    crate::app::visualization::try_build_relationship_graph(
        &struct_view_core::parser::parse_json(
            r#"{"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b"},{"source":"b","target":"a"}]}"#,
        ).unwrap(),
    ).unwrap()
}

fn update(commands: Vec<Value>) -> JobUpdate {
    serde_json::from_value(json!({"graph_id":"graph","commands":commands})).unwrap()
}

fn job(id: &str, stage: &str, edge: Option<usize>, percent: f64) -> Value {
    json!({"type":"upsert","id":id,"label":format!("Task {id}"),
        "stage":stage,"status":"running","nodes":["a"],"edge":edge,"percent":percent})
}

#[test]
fn jobs_share_targets_but_have_independent_lifecycle_and_stage_progress() {
    let graph = graph();
    let mut state = JobsState::apply(
        None,
        update(vec![
            job("one", "Sending", Some(0), 70.0),
            job("two", "Sending", Some(0), 40.0),
        ]),
        &graph,
        0.0,
    )
    .unwrap();
    assert_eq!(state.jobs.len(), 2);
    assert_eq!(state.visuals.statuses_for_node(0).len(), 2);
    let current = state.descriptor(0.3)["jobs"][0]["current_percent"]
        .as_f64()
        .unwrap();
    state = JobsState::apply(
        Some(state),
        update(vec![job("one", "Sending", Some(0), 20.0)]),
        &graph,
        0.3,
    )
    .unwrap();
    assert_eq!(state.jobs["one"].percent, 70.0);
    assert_eq!(
        state.descriptor(0.3)["jobs"][0]["current_percent"]
            .as_f64()
            .unwrap(),
        current
    );
    state = JobsState::apply(
        Some(state),
        update(vec![job("one", "Retrying", Some(0), 10.0)]),
        &graph,
        0.3,
    )
    .unwrap();
    assert_eq!(state.jobs["one"].percent, 10.0);
    assert_eq!(state.descriptor(0.3)["jobs"][0]["current_percent"], 0.0);
    state = JobsState::apply(
        Some(state),
        update(vec![job("one", "Sending", Some(1), 35.0)]),
        &graph,
        0.3,
    )
    .unwrap();
    assert_eq!(state.jobs["one"].edge, Some(1));
    state = JobsState::apply(
        Some(state),
        update(vec![json!({"type":"remove","id":"one"})]),
        &graph,
        0.3,
    )
    .unwrap();
    assert_eq!(state.jobs.keys().collect::<Vec<_>>(), vec!["two"]);
    assert_eq!(state.visuals.statuses_for_edge(0).1.len(), 1);
    state = JobsState::apply(
        Some(state),
        update(vec![json!({"type":"clear"})]),
        &graph,
        0.3,
    )
    .unwrap();
    assert!(state.jobs.is_empty());
    assert!(state.visuals.statuses_for_node(0).is_empty());
}

#[test]
fn invalid_batches_leave_prior_jobs_unchanged_and_limits_are_enforced() {
    let graph = graph();
    let mut state = JobsState::apply(
        None,
        update(vec![job("one", "Sending", Some(0), 50.0)]),
        &graph,
        0.0,
    )
    .unwrap();
    let before = state.descriptor(0.0);
    assert!(
        JobsState::apply(
            Some(state.clone()),
            update(vec![
                job("one", "Changed", Some(0), 10.0),
                job("invalid", "Sending", Some(99), 10.0),
            ]),
            &graph,
            0.0
        )
        .is_err()
    );
    assert_eq!(state.descriptor(0.0), before);
    for command in [
        job("", "Sending", Some(0), 0.0),
        job("one", " ", Some(0), 0.0),
        json!({"type":"remove","id":"unknown"}),
        json!({"type":"upsert","id":"one","label":"Task","stage":"Sending","status":"running"}),
    ] {
        assert!(JobsState::apply(Some(state.clone()), update(vec![command]), &graph, 0.0).is_err());
    }
    let full = JobsState::apply(
        None,
        update(
            (0..MAX_JOBS)
                .map(|index| job(&format!("j{index}"), "Sending", Some(0), 0.0))
                .collect(),
        ),
        &graph,
        0.0,
    )
    .unwrap();
    assert!(
        JobsState::apply(
            Some(full),
            update(vec![job("overflow", "Sending", Some(0), 0.0)]),
            &graph,
            0.0
        )
        .is_err()
    );
    for value in [
        json!({"graph_id":"graph","commands":[]}),
        json!({"graph_id":"graph","commands":[{"type":"upsert","id":"one","label":"Task","stage":"Sending","status":"running","percent":101}]}),
        json!({"graph_id":"graph","commands":[{"type":"remove","id":"one","extra":true}]}),
    ] {
        if let Ok(update) = serde_json::from_value::<JobUpdate>(value) {
            assert!(JobsState::apply(None, update, &graph, 0.0).is_err());
        }
    }
}

#[test]
fn node_only_job_has_progress_and_list_exposes_label_stage_location_and_id() {
    let graph = graph();
    let mut state = JobsState::apply(
        None,
        update(vec![job("one", "Validating", None, 45.0)]),
        &graph,
        0.0,
    )
    .unwrap();
    assert_eq!(state.descriptor(0.0)["jobs"][0]["current_percent"], 45.0);
    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        super::show_jobs(ui, &state, &graph, Locale::English);
    });
    let text = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Task one"));
    assert!(text.contains("Validating"));
    assert!(text.contains("45%"));
    assert!(text.contains("Running"));
    assert!(text.contains("Diagram tasks"));
    assert!(!text.contains('●'));
    let job_color = state.jobs["one"].color;
    let job_color = egui::Color32::from_rgb(job_color[0], job_color[1], job_color[2]);
    assert!(
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Circle(circle)
            if circle.fill == job_color && circle.radius == 4.0)
        }),
        "The job color indicator must be drawn without a font glyph"
    );
    assert_eq!(
        Locale::Russian.text(crate::app::i18n::TextKey::GraphJobs),
        "Задачи схемы"
    );
    let target = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "a" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("job location button");
    output.drop_without_applying_deltas();
    let mut focus = None;
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ui| {
            focus = super::show_jobs(ui, &state, &graph, Locale::English);
        },
    );
    assert_eq!(focus, Some(0));
    output.drop_without_applying_deltas();
}

#[test]
fn jobs_and_legacy_visual_ids_cannot_overwrite_each_other_in_display_state() {
    let graph = graph();
    let state = JobsState::apply(
        None,
        update(vec![job("one", "Sending", Some(0), 45.0)]),
        &graph,
        0.0,
    )
    .unwrap();
    let legacy = RealtimeState::apply(
        None,
        RealtimeUpdate {
            graph_id: "graph".into(),
            commands: vec![RealtimeCommand::Marker {
                id: "job/one".into(),
                edge: 0,
                percent: 25.0,
                smoothing_ms: 300,
                color: [255, 0, 0],
                reverse: false,
                label: None,
                stage: None,
            }],
        },
        &graph,
        0.0,
    )
    .unwrap();
    let display = legacy.with_jobs(&state.visuals);
    assert_eq!(display.statuses_for_edge(0).1.len(), 2);
}

#[test]
fn flowchart_jobs_can_complete_every_route_and_move_to_destination_nodes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("FlowchartAnimationDemo.toml");
    let (root, _) = struct_view_core::parser::parse_data(
        &std::fs::read_to_string(path).unwrap(),
        Some(struct_view_core::parser::DataFormat::Toml),
    )
    .unwrap();
    let graph = crate::app::visualization::try_build_relationship_graph(&root).unwrap();
    for ids in [vec!["order-42"], vec!["order-42", "risk-42"]] {
        let mut state = None;
        for (index, edge) in graph.edges.iter().enumerate() {
            let stage = format!("Step {index}: {}", graph.nodes[edge.target].label);
            for percent in [0.0, 55.0, 100.0] {
                let commands = ids
                    .iter()
                    .map(|id| {
                        json!({
                            "type":"upsert", "id":id, "label":format!("Task {id}"),
                            "stage":stage, "status":"running",
                            "nodes":[graph.nodes[edge.source].id],
                            "edge":index, "percent":percent, "smoothing_ms":70
                        })
                    })
                    .collect();
                state =
                    Some(JobsState::apply(state, update(commands), &graph, index as f64).unwrap());
            }
            let commands = ids
                .iter()
                .map(|id| {
                    json!({
                        "type":"upsert", "id":id, "label":format!("Task {id}"),
                        "stage":stage, "status":"success",
                        "nodes":[graph.nodes[edge.target].id], "edge":null, "percent":100
                    })
                })
                .collect();
            let arrived =
                JobsState::apply(state, update(commands), &graph, index as f64 + 0.9).unwrap();
            assert!(arrived.visuals.statuses_for_edge(index).1.is_empty());
            assert_eq!(
                arrived.visuals.statuses_for_node(edge.target).len(),
                ids.len()
            );
            for id in &ids {
                assert_eq!(
                    arrived.jobs[*id].nodes,
                    [graph.nodes[edge.target].id.clone()]
                );
                assert_eq!(arrived.jobs[*id].percent, 100.0);
                assert!(arrived.jobs[*id].edge.is_none());
            }
            state = Some(arrived);
        }
    }
}
