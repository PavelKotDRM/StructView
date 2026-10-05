use super::*;

#[test]
fn relationship_layout_follows_direction_instead_of_document_order() {
    let graph = layout_graph(
        r#"[{"id":"end"},{"id":"middle","depends_on":"end"},{"id":"start","depends_on":"middle"}]"#,
    );
    let positions = graph_node_positions(&graph);
    for edge in &graph.edges {
        assert!(positions[edge.source].x < positions[edge.target].x);
        assert_eq!(positions[edge.source].y, positions[edge.target].y);
    }
    assert_eq!(positions, graph_node_positions(&graph));
}

#[test]
fn relationship_layout_handles_cycles_disconnected_and_undirected_graphs() {
    for input in [
        r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"a","parent_id":"c"},{"id":"c"},{"id":"isolated"}]"#,
        r#"{"a":["b","c"],"b":["a"],"c":["a"],"isolated":[]}"#,
        r#"[]"#,
        r#"[{"id":"alone"}]"#,
    ] {
        let graph = layout_graph(input);
        let positions = graph_node_positions(&graph);
        assert_eq!(positions.len(), graph.nodes.len());
        assert_eq!(positions, graph_node_positions(&graph));
        for (index, &position) in positions.iter().enumerate() {
            assert!(position.x.is_finite() && position.y.is_finite());
            let rect = egui::Rect::from_center_size(position, GRAPH_NODE_SIZE);
            for &other in &positions[..index] {
                assert!(!rect.intersects(egui::Rect::from_center_size(other, GRAPH_NODE_SIZE)));
            }
        }
    }
    let graph = layout_graph(
        r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"a","parent_id":"c"},{"id":"c"}]"#,
    );
    let positions = graph_node_positions(&graph);
    assert_eq!(positions[0].x, positions[1].x);
    assert!(positions[2].x > positions[1].x);
}

#[test]
fn partition_layout_keeps_columns_and_orders_neighbors_to_avoid_crossings() {
    let mut graph = layout_graph(
        r#"[{"id":"a","depends_on":"d"},{"id":"b","depends_on":"c"},{"id":"c"},{"id":"d"}]"#,
    );
    graph.partition_names = Some(vec!["left".into(), "right".into()]);
    for (index, node) in graph.nodes.iter_mut().enumerate() {
        node.partition = Some(usize::from(index >= 2));
    }
    let positions = graph_node_positions(&graph);
    assert_eq!(positions[0].x, positions[1].x);
    assert_eq!(positions[2].x, positions[3].x);
    assert_eq!(positions[0].y, positions[3].y);
    assert_eq!(positions[1].y, positions[2].y);
    assert!(positions[0].x < positions[3].x);
}

#[test]
fn graph_activity_omits_timing_diagnostics() {
    let state = GraphCalculationState::default();
    {
        let mut progress = state.progress.lock().unwrap();
        progress.begin(GraphStage::Conflicts, 10, 4);
        progress.completed = 3;
    }
    let context = egui::Context::default();
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        state.show_activity(ui, Locale::English);
    });
    let texts = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some(text.galley.job.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(texts.contains(&"Checking and resolving route conflicts"));
    assert!(texts.contains(&"3 / 10"));
    assert!(!texts.iter().any(|text| text.contains("Calculation time")
        || text.contains("Routing workers")
        || text.contains("Slowest")));
    output.drop_without_applying_deltas();
}

#[test]
fn graph_progress_records_all_stages_and_completion() {
    let root = struct_view_core::parser::parse_json(r#"[{"id":"a","depends_on":"b"},{"id":"b"}]"#)
        .unwrap();
    let progress = Arc::new(Mutex::new(GraphProgress::default()));
    let result =
        build_graph_calculation(root, GraphRoutingWorkerSetting::Automatic, &progress).unwrap();
    let snapshot = progress.lock().unwrap();
    assert_eq!(
        snapshot
            .timings
            .iter()
            .map(|(stage, _)| *stage)
            .collect::<Vec<_>>(),
        [
            GraphStage::Entities,
            GraphStage::Layout,
            GraphStage::Sequential,
            GraphStage::Labels
        ],
    );
    assert_eq!(snapshot.completed, result.graph.edges.len());
    assert_eq!(snapshot.total, result.graph.edges.len());
    assert!(snapshot.stage.is_none());
    assert!(snapshot.finished.unwrap() >= snapshot.timings.iter().map(|(_, time)| *time).sum());
}

#[test]
fn headless_graph_image_reports_finished_progress_and_stage_timings() {
    let root = struct_view_core::parser::parse_json(r#"[{"id":"a","depends_on":"b"},{"id":"b"}]"#)
        .unwrap();
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let reported = Arc::clone(&snapshots);
    crate::app::headless::graph_image_with_progress(&root, false, false, move |snapshot| {
        reported.lock().unwrap().push(snapshot);
    })
    .unwrap();

    let snapshots = snapshots.lock().unwrap();
    let final_snapshot = snapshots
        .last()
        .expect("A final progress snapshot is reported");
    assert!(final_snapshot.finished);
    assert!(
        final_snapshot
            .timings
            .iter()
            .any(|(stage, _)| *stage == "Routing links sequentially")
    );
    assert!(
        final_snapshot
            .timings
            .iter()
            .any(|(stage, _)| *stage == "Rendering graph image")
    );
}

#[test]
fn parallel_graph_progress_counts_completed_edges_and_preserves_routes() {
    let (grid, endpoints, ports) = routing_fixture(16);
    let progress = Arc::new(Mutex::new(GraphProgress::default()));
    let routes = route_graph_edges_with_progress(&grid, &endpoints, &ports, 4, Some(&progress));
    assert_eq!(routes, route_graph_edges(&grid, &endpoints, &ports, 4));
    let mut snapshot = progress.lock().unwrap();
    assert_eq!(snapshot.stage, Some(GraphStage::Conflicts));
    assert_eq!(snapshot.completed, endpoints.len());
    assert_eq!(snapshot.total, endpoints.len());
    assert_eq!(snapshot.workers, 4);
    assert_eq!(snapshot.timings[0].0, GraphStage::Preliminary);
    snapshot.finish();
    assert_eq!(snapshot.timings[1].0, GraphStage::Conflicts);
}

#[test]
fn graph_progress_ui_shows_live_counts_and_retains_slowest_stage() {
    let state = GraphCalculationState::default();
    {
        let mut progress = state.progress.lock().unwrap();
        progress.begin(GraphStage::Preliminary, 20, 4);
        progress.completed = 7;
        progress
            .timings
            .push((GraphStage::Layout, Duration::from_secs(2)));
    }
    let context = egui::Context::default();
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        state.show_progress(ui, Locale::English);
    });
    let text = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some(text.galley.job.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(text.contains(&"7 / 20"));
    assert!(text.contains(&"Routing workers: 4"));
    assert!(
        text.iter()
            .any(|text| text.contains("Calculating preliminary routes"))
    );
    output.drop_without_applying_deltas();
    state.progress.lock().unwrap().finish();
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        state.show_progress(ui, Locale::English);
    });
    assert!(output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == "Slowest completed stage: Preparing layout and connection ports (2.00 s)")
    }));
    output.drop_without_applying_deltas();
}
