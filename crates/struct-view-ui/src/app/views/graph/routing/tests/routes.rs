use super::*;

#[test]
fn self_loops_route_outside_cards_with_distinct_ports_and_parallel_tracks() {
    let positions = [Pos2::new(160.0, 100.0), Pos2::new(500.0, 100.0)];
    let grid = GraphRoutingGrid::new(&positions);
    let endpoints = [(0, 0), (0, 0), (1, 1), (0, 1)];
    let ports = graph_edge_ports(&positions, &endpoints);
    let serial = route_graph_edges(&grid, &endpoints, &ports, 1);
    let parallel = route_graph_edges(&grid, &endpoints, &ports, 4);
    for routes in [&serial, &parallel] {
        for (index, route) in routes.iter().enumerate() {
            assert!(
                route
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
            assert!(route.windows(2).all(|segment| segment[0] != segment[1]));
            if endpoints[index].0 == endpoints[index].1 {
                assert!(route.len() >= 4, "{route:?}");
                assert_ne!(route.first(), route.last());
                for rect in &grid.node_rects {
                    assert!(
                        route
                            .windows(2)
                            .all(|segment| !segment_crosses_rect_interior(
                                segment[0], segment[1], *rect
                            )),
                        "{route:?}"
                    );
                }
            }
        }
        assert_ne!(routes[0], routes[1]);
    }
    assert_eq!(serial[0].first(), parallel[0].first());
    assert_eq!(serial[0].last(), parallel[0].last());
}

#[test]
fn reverse_edges_change_layout_and_direction_changes_invalidate_cache() {
    let root = struct_view_core::parser::parse_json(r#"{"graph":{"type":"directed"},"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b"}]}"#).unwrap();
    let mut graph = build_relationship_graph(&root);
    let forward = graph_node_positions(&graph);
    let fingerprint = relationship_graph_fingerprint(&graph);
    graph.edges[0].direction = EdgeDirection::Reverse;
    let reverse = graph_node_positions(&graph);
    assert!(forward[0].x < forward[1].x);
    assert!(reverse[1].x < reverse[0].x);
    assert_ne!(fingerprint, relationship_graph_fingerprint(&graph));
    graph.edges[0].direction = EdgeDirection::Bidirectional;
    let mutual = graph_node_positions(&graph);
    assert_eq!(mutual[0].x, mutual[1].x);
}

#[test]
fn indexed_graph_penalties_match_exhaustive_segment_checks() {
    let routes = vec![
        vec![Pos2::new(-300.0, -20.0), Pos2::new(700.0, -20.0)],
        vec![Pos2::new(128.0, -400.0), Pos2::new(128.0, 800.0)],
        vec![Pos2::new(-300.0, 700.0), Pos2::new(700.0, -300.0)],
        vec![Pos2::ZERO, Pos2::ZERO],
        vec![
            Pos2::new(256.0, 0.0),
            Pos2::new(256.0, 128.0),
            Pos2::new(384.0, 128.0),
        ],
    ];
    let index = GraphRouteSegmentIndex::new(&routes);
    for x in [-308.0, -128.0, 0.0, 120.0, 128.0, 136.0, 256.0, 708.0] {
        for y in [-400.0, -28.0, -20.0, -12.0, 0.0, 128.0, 800.0] {
            let start = Pos2::new(x, y);
            for delta in [
                Vec2::ZERO,
                Vec2::new(50.0, 0.0),
                Vec2::new(0.0, 300.0),
                Vec2::new(-700.0, 400.0),
            ] {
                let end = start + delta;
                let expected: f32 = routes
                    .iter()
                    .flat_map(|route| route.windows(2))
                    .map(|segment| segment_pair_penalty(start, end, [segment[0], segment[1]]))
                    .sum();
                assert_eq!(index.penalty(start, end), expected, "{start:?} -> {end:?}");
                assert_eq!(index.penalty(end, start), expected);
            }
        }
    }
}

#[test]
fn indexed_graph_conflicts_match_exhaustive_checks_when_routes_are_added_incrementally() {
    let routes = vec![
        vec![Pos2::new(-300.0, -20.0), Pos2::new(700.0, -20.0)],
        vec![Pos2::new(128.0, -400.0), Pos2::new(128.0, 800.0)],
        vec![Pos2::new(-300.0, 700.0), Pos2::new(700.0, -300.0)],
        vec![
            Pos2::new(256.0, 0.0),
            Pos2::new(256.0, 128.0),
            Pos2::new(384.0, 128.0),
        ],
    ];
    let mut index = GraphRouteSegmentIndex::new(&[]);
    let mut accepted = Vec::new();
    for route in &routes {
        for x in [-308.0, -128.0, 0.0, 120.0, 128.0, 136.0, 256.0, 708.0] {
            for y in [-400.0, -28.0, -20.0, -12.0, 0.0, 128.0, 800.0] {
                let candidate = [
                    Pos2::new(x, y),
                    Pos2::new(x + 50.0, y),
                    Pos2::new(x + 50.0, y + 30.0),
                ];
                assert_eq!(
                    index.conflicts_route(&candidate),
                    graph_route_conflicts(&candidate, &accepted),
                    "{candidate:?}",
                );
            }
        }
        index.insert_route(route);
        accepted.push(route.clone());
    }
}

#[test]
fn dense_parallel_conflict_resolution_is_deterministic_and_preserves_ports() {
    let (grid, _, _) = routing_fixture(4);
    let endpoints = vec![(0, 2); 12];
    let ports = graph_edge_ports(&grid.node_positions, &endpoints);
    let expected = route_graph_edges(&grid, &endpoints, &ports, 2);
    for workers in [3, 4, 8] {
        assert_eq!(
            route_graph_edges(&grid, &endpoints, &ports, workers),
            expected
        );
    }
    let serial = route_graph_edges(&grid, &endpoints, &ports, 1);
    for (index, route) in expected.iter().enumerate() {
        assert_eq!(route.first(), serial[index].first());
        assert_eq!(route.last(), serial[index].last());
        for rect in &grid.node_rects {
            assert!(
                route.windows(2).all(|segment| {
                    !segment_crosses_rect_interior(segment[0], segment[1], *rect)
                })
            );
        }
        let conflicts = |route: &[Pos2], previous: &[Vec<Pos2>]| {
            previous
                .iter()
                .filter(|other| graph_route_conflicts(route, std::slice::from_ref(*other)))
                .count()
        };
        assert!(
            conflicts(route, &expected[..index]) <= conflicts(&serial[index], &serial[..index])
        );
    }
}

#[test]
fn parallel_graph_routes_are_deterministic_and_keep_separate_tracks() {
    let (grid, endpoints, ports) = routing_fixture(16);
    let expected = route_graph_edges(&grid, &endpoints, &ports, 2);
    for workers in [3, 4, 8] {
        assert_eq!(
            route_graph_edges(&grid, &endpoints, &ports, workers),
            expected
        );
    }
    assert_eq!(expected.len(), endpoints.len());
    for (index, route) in expected.iter().enumerate() {
        assert!(route.len() > 2);
        assert!(!graph_route_conflicts(route, &expected[..index]));
        for rect in &grid.node_rects {
            assert!(
                route.windows(2).all(|segment| {
                    !segment_crosses_rect_interior(segment[0], segment[1], *rect)
                })
            );
        }
    }
}

#[test]
fn parallel_graph_routing_resolves_crossing_parallel_and_reciprocal_edges() {
    let (grid, _, _) = routing_fixture(3);
    for endpoints in [vec![(0, 2), (0, 2), (2, 0)], vec![(0, 8), (2, 6)]] {
        let ports = graph_edge_ports(&grid.node_positions, &endpoints);
        let serial = route_graph_edges(&grid, &endpoints, &ports, 1);
        let parallel = route_graph_edges(&grid, &endpoints, &ports, 4);
        assert_eq!(parallel, serial);
        assert_eq!(route_graph_edges(&grid, &endpoints, &ports, 2), parallel);
        let preliminary = endpoints
            .iter()
            .zip(&ports)
            .map(|(&(source, target), &ports)| {
                grid.route_edge_with_ports(source, target, ports, &[])
            })
            .collect::<Vec<_>>();
        assert_ne!(
            parallel, preliminary,
            "Conflicting candidates must be rerouted"
        );
        for (index, route) in parallel.iter().enumerate() {
            assert_eq!(
                graph_route_conflicts(route, &parallel[..index]),
                graph_route_conflicts(&serial[index], &serial[..index]),
            );
            assert!(route.windows(2).all(|segment| segment[0] != segment[1]));
        }
    }
}

#[test]
fn routes_crossing_the_same_node_prefer_the_same_detour_side() {
    fn assert_top_detour(route: &[Pos2], grid: &GraphRoutingGrid, obstacle_index: usize) {
        let obstacle = grid.obstacles[obstacle_index];
        assert!(
            route.windows(2).any(|segment| {
                segment[0].y == segment[1].y
                    && segment[0].y <= obstacle.top()
                    && segment[0].x.max(segment[1].x) >= obstacle.left()
                    && segment[0].x.min(segment[1].x) <= obstacle.right()
            }),
            "Route should pass over the shared obstacle on its preferred side: {route:?}"
        );
        assert!(
            route.windows(2).all(|segment| {
                segment[0].y != segment[1].y
                    || segment[0].y < obstacle.bottom()
                    || segment[0].x.max(segment[1].x) < obstacle.left()
                    || segment[0].x.min(segment[1].x) > obstacle.right()
            }),
            "Route should not form the opposite side of a square around the obstacle: {route:?}"
        );
        assert!(
            route.windows(2).all(|segment| {
                !segment_crosses_rect_interior(
                    segment[0],
                    segment[1],
                    grid.node_rects[obstacle_index],
                )
            }),
            "Route must not cross the obstacle node: {route:?}"
        );
    }

    fn assert_left_detour(route: &[Pos2], grid: &GraphRoutingGrid, obstacle_index: usize) {
        let obstacle = grid.obstacles[obstacle_index];
        assert!(
            route.windows(2).any(|segment| {
                segment[0].x == segment[1].x
                    && segment[0].x <= obstacle.left()
                    && segment[0].y.max(segment[1].y) >= obstacle.top()
                    && segment[0].y.min(segment[1].y) <= obstacle.bottom()
            }),
            "Vertical routes should use the left side as the shared bypass: {route:?}"
        );
        assert!(
            route.windows(2).all(|segment| {
                segment[0].x != segment[1].x
                    || segment[0].x < obstacle.right()
                    || segment[0].y.max(segment[1].y) < obstacle.top()
                    || segment[0].y.min(segment[1].y) > obstacle.bottom()
            }),
            "Route should not use the opposite side of the obstacle: {route:?}"
        );
        assert!(
            route.windows(2).all(|segment| {
                !segment_crosses_rect_interior(
                    segment[0],
                    segment[1],
                    grid.node_rects[obstacle_index],
                )
            }),
            "Route must not cross the obstacle node: {route:?}"
        );
    }

    let positions = [
        Pos2::new(100.0, 400.0),
        Pos2::new(400.0, 400.0),
        Pos2::new(700.0, 400.0),
        Pos2::new(200.0, 200.0),
        Pos2::new(600.0, 600.0),
    ];
    let grid = GraphRoutingGrid::new(&positions);
    let endpoints = [(0, 2), (3, 4)];
    let ports = graph_edge_ports(&positions, &endpoints);
    let routes = route_graph_edges(&grid, &endpoints, &ports, 1);
    for route in routes {
        assert_top_detour(&route, &grid, 1);
    }

    let positions = [
        Pos2::new(400.0, 100.0),
        Pos2::new(400.0, 400.0),
        Pos2::new(400.0, 700.0),
    ];
    let grid = GraphRoutingGrid::new(&positions);
    let endpoints = [(0, 2)];
    let ports = graph_edge_ports(&positions, &endpoints);
    let route = route_graph_edges(&grid, &endpoints, &ports, 1).remove(0);
    assert_left_detour(&route, &grid, 1);
}

#[test]
fn graph_routing_handles_empty_and_small_graphs_without_parallel_workers() {
    assert_eq!(
        graph_routing_worker_count(0, GraphRoutingWorkerSetting::Automatic),
        1
    );
    let available = available_graph_routing_workers();
    assert_eq!(
        graph_routing_worker_count(usize::MAX, GraphRoutingWorkerSetting::Automatic),
        available
    );
    assert_eq!(
        graph_routing_worker_count(3, GraphRoutingWorkerSetting::Automatic),
        available.min(3)
    );
    assert_eq!(
        graph_routing_worker_count(usize::MAX, GraphRoutingWorkerSetting::Manual(usize::MAX)),
        available
    );
    assert_eq!(
        graph_routing_worker_count(3, GraphRoutingWorkerSetting::Manual(8)),
        3
    );
    assert_eq!(
        graph_routing_worker_count(3, GraphRoutingWorkerSetting::Manual(2)),
        2
    );
    let (grid, _, _) = routing_fixture(1);
    assert!(route_graph_edges(&grid, &[], &[], 8).is_empty());
    let endpoints = [(0, 2)];
    let ports = graph_edge_ports(&grid.node_positions, &endpoints);
    assert_eq!(
        route_graph_edges(&grid, &endpoints, &ports, 1),
        route_graph_edges(&grid, &endpoints, &ports, 8),
    );
}

#[test]
#[ignore = "Manual routing timing comparison; run with --release --ignored --nocapture"]
fn graph_routing_parallel_timing() {
    let (grid, endpoints, ports) = routing_fixture(96);
    for workers in [1, 2, 4, 8] {
        let started = std::time::Instant::now();
        let routes = route_graph_edges(&grid, &endpoints, &ports, workers);
        println!(
            "Graph routing: {workers} workers, {:?}, {} edges",
            started.elapsed(),
            routes.len()
        );
        for (index, route) in routes.iter().enumerate() {
            assert!(!graph_route_conflicts(route, &routes[..index]));
        }
    }
    let positions = (0..36)
        .map(|index| {
            Pos2::new(
                128.0 + (index % 6) as f32 * GRAPH_STEP.x,
                59.0 + (index / 6) as f32 * GRAPH_STEP.y,
            )
        })
        .collect::<Vec<_>>();
    let endpoints = (0..36)
        .flat_map(|index| [(index, (index + 1) % 36), (index, (index + 7) % 36)])
        .collect::<Vec<_>>();
    let grid = GraphRoutingGrid::new(&positions);
    let ports = graph_edge_ports(&positions, &endpoints);
    for workers in [1, 2, 4, 8] {
        let started = std::time::Instant::now();
        let routes = route_graph_edges(&grid, &endpoints, &ports, workers);
        println!(
            "Dense graph routing: {workers} workers, {:?}, {} edges",
            started.elapsed(),
            routes.len(),
        );
        assert_eq!(routes.len(), endpoints.len());
    }
    let positions = (0..64)
        .flat_map(|group| {
            let top = 59.0 + group as f32 * GRAPH_STEP.y * 3.0;
            [
                Pos2::new(128.0, top),
                Pos2::new(468.0, top),
                Pos2::new(128.0, top + GRAPH_STEP.y),
                Pos2::new(468.0, top + GRAPH_STEP.y),
            ]
        })
        .collect::<Vec<_>>();
    let endpoints = (0..64)
        .map(|group| (group * 4, group * 4 + 3))
        .chain((0..64).map(|group| (group * 4 + 1, group * 4 + 2)))
        .collect::<Vec<_>>();
    let grid = GraphRoutingGrid::new(&positions);
    let ports = graph_edge_ports(&positions, &endpoints);
    for workers in [2, 4, 8] {
        let progress = Arc::new(Mutex::new(GraphProgress::default()));
        let routes =
            route_graph_edges_with_progress(&grid, &endpoints, &ports, workers, Some(&progress));
        let mut snapshot = progress.lock().unwrap();
        snapshot.finish();
        let conflict_time = snapshot
            .timings
            .iter()
            .find(|(stage, _)| *stage == GraphStage::Conflicts)
            .unwrap()
            .1;
        println!(
            "Independent conflict groups: {workers} workers, conflict stage {conflict_time:?}"
        );
        assert_eq!(routes.len(), endpoints.len());
        for (index, route) in routes.iter().enumerate() {
            assert!(!graph_route_conflicts(route, &routes[..index]));
        }
    }
}

#[test]
#[ignore = "Manual large-fixture routing benchmark; run with --release --ignored --nocapture"]
fn graph_routing_large_fixture_timing() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/large-routing-graph.json"
    ))
    .unwrap();
    let graph: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    let entities = graph["entities"].as_object().unwrap();
    let ids = entities.keys().collect::<Vec<_>>();
    let node_indices = ids
        .iter()
        .enumerate()
        .map(|(index, id)| ((*id).clone(), index))
        .collect::<std::collections::HashMap<_, _>>();
    let positions = (0..ids.len())
        .map(|index| {
            Pos2::new(
                128.0 + (index / 20) as f32 * GRAPH_STEP.x,
                59.0 + (index % 20) as f32 * GRAPH_STEP.y,
            )
        })
        .collect::<Vec<_>>();
    let edges = graph["relations"]["edges"].as_array().unwrap();
    let endpoints = edges
        .iter()
        .map(|edge| {
            (
                node_indices[edge[0].as_str().unwrap()],
                node_indices[edge[1].as_str().unwrap()],
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 240);
    assert_eq!(endpoints.len(), 1320);
    let ports = graph_edge_ports(&positions, &endpoints);
    let grid = GraphRoutingGrid::new(&positions);
    let workers = 4;
    let started = std::time::Instant::now();
    let routes = route_graph_edges(&grid, &endpoints, &ports, workers);
    println!(
        "Large fixture routing: {workers} workers, {:?}, {} nodes, {} edges",
        started.elapsed(),
        positions.len(),
        routes.len()
    );
    assert_eq!(routes.len(), endpoints.len());
    for (index, route) in routes.iter().enumerate() {
        assert!(
            route
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        assert!(route.windows(2).all(|segment| segment[0] != segment[1]));
        for (obstacle, rect) in grid.obstacles.iter().enumerate() {
            if obstacle != endpoints[index].0 && obstacle != endpoints[index].1 {
                assert!(
                    route
                        .windows(2)
                        .all(|segment| { !segment_intersects_rect(segment[0], segment[1], *rect) }),
                    "Route {index} crosses obstacle {obstacle}"
                );
            }
        }
    }
}

#[test]
fn graph_labels_remain_visible_without_covering_routes_or_arrowheads() {
    for input in [
        r#"[{"id":"source","depends_on":"target","parent_id":"target","ref":"target"},{"id":"target"}]"#,
        r#"[{"id":"tl","depends_on":"br"},{"id":"tr","parent_id":"bl"},{"id":"bl"},{"id":"br"}]"#,
        r#"[{"id":"left","depends_on":"right"},{"id":"right","depends_on":"left"}]"#,
    ] {
        let root = struct_view_core::parser::parse_json(input).unwrap();
        let graph = build_relationship_graph(&root);
        let layout = build_graph_routing_layout(&graph);
        assert_eq!(
            layout.edge_labels.iter().flatten().count(),
            graph.edges.len(),
            "Every relationship must retain a label: {input}",
        );
        for label in layout.edge_labels.iter().flatten() {
            for route in &layout.edge_paths {
                assert!(
                    route.windows(2).all(|segment| {
                        !segment_intersects_rect(
                            segment[0],
                            segment[1],
                            label.background.expand(2.0),
                        )
                    }),
                    "Label {} obscures a route",
                    label.text
                );
                assert!(
                    !label.background.intersects(egui::Rect::from_center_size(
                        *route.last().unwrap(),
                        Vec2::splat(20.0),
                    )),
                    "Label {} obscures an arrowhead",
                    label.text
                );
            }
            for position in &layout.node_positions {
                assert!(!label.background.intersects(
                    egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE).expand(2.0),
                ));
            }
        }
    }
}

#[test]
fn displaced_graph_labels_point_to_their_own_route() {
    let points = [egui::pos2(40.0, 100.0), egui::pos2(240.0, 100.0)];
    let blocking_label = egui::Rect::from_min_max(egui::pos2(0.0, 70.0), egui::pos2(300.0, 130.0));
    let label = layout_graph_edge_label(
        "depends_on",
        &points,
        egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0)),
        &[],
        &[blocking_label],
        &[points.to_vec()],
    )
    .expect("A displaced relationship label must still be visible");
    let [anchor, end] = label
        .leader
        .expect("Displaced labels must identify their route");
    assert_eq!(anchor.y, 100.0);
    assert!((40.0..=240.0).contains(&anchor.x));
    assert!(label.background.expand(0.001).contains(end));
    assert!(!label.background.shrink(0.001).contains(end));
}
