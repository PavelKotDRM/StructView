use super::*;

#[test]
fn densely_connected_graph_layout_has_room_for_all_routes() {
    let nodes = (0..10)
        .map(|id| serde_json::json!({"id": id.to_string()}))
        .collect::<Vec<_>>();
    let edges = (0..10)
        .flat_map(|source| {
            (0..10).map(move |target| {
                serde_json::json!({
                    "source": source.to_string(), "target": target.to_string()
                })
            })
        })
        .collect::<Vec<_>>();
    let graph = layout_graph(&serde_json::json!({"nodes": nodes, "edges": edges}).to_string());
    let layout = build_graph_routing_layout(&graph);
    assert_eq!(layout.edge_paths.len(), 100);
    let mut rows = layout
        .node_positions
        .iter()
        .map(|point| point.y)
        .collect::<Vec<_>>();
    rows.sort_by(f32::total_cmp);
    assert!(
        rows.windows(2)
            .all(|pair| pair[1] - pair[0] >= GRAPH_STEP.y + 120.0)
    );
    let mut index = GraphRouteSegmentIndex::new(&[]);
    for route in &layout.edge_paths {
        assert!(index.first_overlapping_segment(route, 0.0).is_none());
        index.insert_route(route);
        for &position in &layout.node_positions {
            let rect = egui::Rect::from_center_size(position, GRAPH_NODE_SIZE);
            assert!(
                route
                    .windows(2)
                    .all(|pair| { !segment_crosses_rect_interior(pair[0], pair[1], rect) })
            );
        }
    }
}

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
fn relationship_layout_packs_many_isolated_entities_into_a_compact_grid() {
    let input = format!(
        "[{}]",
        (0..100)
            .map(|index| format!(r#"{{"id":"isolated-{index}"}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    let graph = layout_graph(&input);
    let positions = graph_node_positions(&graph);
    let row_count = positions
        .iter()
        .map(|position| position.y.to_bits())
        .collect::<std::collections::HashSet<_>>()
        .len();

    assert_eq!(graph.nodes.len(), 100);
    assert!(graph.edges.is_empty());
    assert_eq!(row_count, 10);
    assert_eq!(positions, graph_node_positions(&graph));
    for (index, &position) in positions.iter().enumerate() {
        let rect = egui::Rect::from_center_size(position, GRAPH_NODE_SIZE);
        for &other in &positions[..index] {
            assert!(!rect.intersects(egui::Rect::from_center_size(other, GRAPH_NODE_SIZE)));
        }
    }
}

#[test]
fn isolated_entities_do_not_spread_or_reroute_the_connected_graph() {
    let connected =
        layout_graph(r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"c"},{"id":"c"}]"#);
    let mut input = r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"c"},{"id":"c"}"#
        .trim_end_matches(']')
        .to_string();
    for index in 0..100 {
        input.push_str(&format!(r#",{{"id":"isolated-{index}"}}"#));
    }
    input.push(']');
    let with_isolated = layout_graph(&input);
    let connected_routing = build_graph_routing_layout(&connected);
    let isolated_routing = build_graph_routing_layout(&with_isolated);

    assert_eq!(
        &isolated_routing.node_positions[..connected.nodes.len()],
        connected_routing.node_positions
    );
    assert_eq!(isolated_routing.edge_paths, connected_routing.edge_paths);
    let mut routes_at_half_clearance =
        struct_view_routing::orthogonal::RouteIndex::new(&[], routing::GRAPH_EDGE_CLEARANCE / 2.0)
            .unwrap();
    for route in &connected_routing.edge_paths {
        let route = route
            .iter()
            .map(|point| struct_view_routing::orthogonal::Point::new(point.x, point.y))
            .collect::<Vec<_>>();
        assert!(
            !routes_at_half_clearance.parallel_conflicts_route(&route),
            "{route:?}"
        );
        routes_at_half_clearance.insert_route(&route).unwrap();
    }
    let connected_bottom = connected_routing
        .node_positions
        .iter()
        .map(|position| position.y)
        .fold(0.0, f32::max);
    assert!(
        isolated_routing.node_positions[connected.nodes.len()..]
            .iter()
            .all(|position| position.y < connected_bottom + GRAPH_STEP.y * 10.0)
    );
}

#[test]
fn relationship_labels_stay_next_to_the_connected_graph_when_isolated_nodes_are_added() {
    let mut nodes = vec![
        serde_json::json!({"id":"source"}),
        serde_json::json!({"id":"target"}),
    ];
    let edges = (0..12)
        .map(|index| {
            serde_json::json!({
                "source": "source",
                "target": "target",
                "label": format!("relationship label {index} with a long description")
            })
        })
        .collect::<Vec<_>>();
    let input = serde_json::json!({
        "graph": {"type": "directed_multigraph"},
        "nodes": nodes.clone(),
        "edges": edges.clone()
    });
    let connected = layout_graph(&input.to_string());
    nodes.extend((0..100).map(|index| serde_json::json!({"id":format!("isolated-{index}")})));
    let input = serde_json::json!({
        "graph": {"type": "directed_multigraph"},
        "nodes": nodes,
        "edges": edges
    });
    let graph = layout_graph(&input.to_string());
    let with_isolated = build_graph_routing_layout(&graph);
    let connected_routing = build_graph_routing_layout(&connected);

    for (before, after) in connected_routing
        .edge_labels
        .iter()
        .zip(&with_isolated.edge_labels)
    {
        let before = before.as_ref().unwrap();
        let after = after.as_ref().unwrap();
        assert_eq!(before.position, after.position);
        assert_eq!(before.background, after.background);
        assert!(
            after.background.right()
                < with_isolated.node_positions[connected.nodes.len()].x - GRAPH_STEP.x
        );
    }
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
    let result = build_graph_calculation(
        root,
        GraphRoutingWorkerSetting::Automatic,
        RoutingSearchBackend::Builtin,
        &progress,
    )
    .unwrap();
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
    let routes =
        route_graph_edges_with_progress(&grid, &endpoints, &ports, 4, Some(&progress)).unwrap();
    assert_eq!(routes, route_graph_edges(&grid, &endpoints, &ports, 4));
    let mut snapshot = progress.lock().unwrap();
    assert_eq!(snapshot.stage, Some(GraphStage::Conflicts));
    assert_eq!(snapshot.completed, endpoints.len());
    assert_eq!(snapshot.total, endpoints.len());
    assert_eq!(snapshot.workers, 1);
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
