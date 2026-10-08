use super::*;
use crate::RoutingError;

#[test]
fn dense_graphs_route_without_overlaps_or_worker_panics() {
    let positions = (0..9)
        .map(|index| {
            Point::new(
                128.0 + (index / 3) as f32 * 340.0,
                59.0 + (index % 3) as f32 * 150.0,
            )
        })
        .collect::<Vec<_>>();
    let endpoints = (0..positions.len())
        .flat_map(|source| (0..positions.len()).map(move |target| (source, target)))
        .collect::<Vec<_>>();
    let mut options = OrthogonalRouterOptions::new(Size::new(208.0, 70.0), Size::new(340.0, 150.0));
    let nodes = (0..positions.len()).collect::<Vec<_>>();
    let ports = assign_edge_ports(&positions, options.node_size, &endpoints).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
        let mut index = RouteIndex::new(&[], options.edge_clearance).unwrap();
        for (&(source, target), &ports) in endpoints.iter().zip(&ports) {
            let route = router
                .route_edge(source, target, ports, &index)
                .unwrap_or_else(|error| {
                    panic!("{backend:?} {source}->{target} {ports:?}: {error:?}")
                });
            assert!(
                index.first_overlapping_segment(&route, 0.0).is_none(),
                "{backend:?} {source}->{target}: {route:?}"
            );
            index.insert_route(&route).unwrap();
        }
    }
}

#[test]
fn reserved_port_leads_keep_later_edges_routable() {
    // Layered columns like the large UI fixture: earlier routes used to hug node boundaries
    // and run along the port leads of edges routed later.
    let positions = (0..24)
        .map(|index| {
            Point::new(
                128.0 + (index / 12) as f32 * 340.0,
                59.0 + (index % 12) as f32 * 150.0,
            )
        })
        .collect::<Vec<_>>();
    let endpoints = (0..12)
        .flat_map(|source| {
            [1, 3, 5, 8]
                .into_iter()
                .map(move |shift| (source, 12 + (source + shift) % 12))
                .chain([(source, (source + 2) % 12)])
        })
        .collect::<Vec<_>>();
    let mut options = OrthogonalRouterOptions::new(Size::new(208.0, 70.0), Size::new(340.0, 150.0));
    options.obstacle_clearance = 18.0;
    let nodes = (0..positions.len()).collect::<Vec<_>>();
    let ports = assign_edge_ports(&positions, options.node_size, &endpoints).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
    ] {
        options.search_backend = backend;
        let mut router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
        router.reserve_port_leads(&endpoints, &ports).unwrap();
        let mut index = RouteIndex::new(&[], options.edge_clearance).unwrap();
        for (&(source, target), &ports) in endpoints.iter().zip(&ports) {
            let route = router
                .route_edge(source, target, ports, &index)
                .unwrap_or_else(|error| panic!("{backend:?} {source}->{target}: {error:?}"));
            assert!(index.first_overlapping_segment(&route, 0.0).is_none());
            index.insert_route(&route).unwrap();
        }
    }
    let mut router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
    assert_eq!(
        router.reserve_port_leads(&endpoints[1..], &ports),
        Err(RoutingError::InvalidGeometry)
    );
}

#[test]
fn routes_keep_other_ports_straight_exits_free() {
    // Any vertical track between the Z-route bends is equally short; prefer one that is not
    // the straight exit of another node's top port.
    let positions = [
        Point::new(100.0, 100.0),
        Point::new(700.0, 300.0),
        Point::new(158.0, 420.0),
        Point::new(400.0, 420.0),
        Point::new(642.0, 420.0),
        Point::new(400.0, 200.0),
    ];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 100.0));
    let side_ports = EdgePorts {
        source_side: NodeSide::Right,
        target_side: NodeSide::Left,
        source_offset: 0.0,
        target_offset: 0.0,
    };
    let top_ports = EdgePorts {
        source_side: NodeSide::Top,
        target_side: NodeSide::Top,
        source_offset: 0.0,
        target_offset: 0.0,
    };
    let endpoints = [(0, 1), (2, 3), (3, 4), (4, 2)];
    let ports = [side_ports, top_ports, top_ports, top_ports];
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let mut router = OrthogonalRouter::new(&positions, &[0, 1, 2, 3, 4, 5], options).unwrap();
        router.reserve_port_leads(&endpoints, &ports).unwrap();
        let route = router.route_edge(0, 1, side_ports, &index).unwrap();
        assert_eq!(route.len(), 4, "{backend:?}: {route:?}");
        assert!(
            ![158.0, 642.0].contains(&route[1].x),
            "{backend:?}: {route:?}"
        );
    }
}

#[test]
fn shortest_length_takes_priority_over_fewer_bends() {
    let positions = [
        Point::new(100.0, 300.0),
        Point::new(700.0, 300.0),
        Point::new(300.0, 270.0),
        Point::new(500.0, 330.0),
    ];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 100.0));
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &[0, 1, 2, 3], options).unwrap();
        let route = router.route_edge(0, 1, ports, &index).unwrap();
        let length: f32 = route.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        assert_eq!(length, 572.0, "{backend:?}: {route:?}");
        assert_eq!(route.len(), 8, "{backend:?}: {route:?}");
        for position in &positions[2..] {
            let obstacle = Rect::from_center_size(*position, options.node_size)
                .expand(options.obstacle_clearance);
            assert!(
                route
                    .windows(2)
                    .all(|pair| { !segment_intersects_rect(pair[0], pair[1], obstacle) }),
                "{backend:?}: {route:?}"
            );
        }
    }
}

#[test]
fn routed_tracks_never_share_even_short_sections() {
    let positions = [
        Point::new(100.0, 300.0),
        Point::new(700.0, 300.0),
        Point::new(400.0, 300.0),
    ];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 100.0));
    let endpoints = vec![(0, 1); 8];
    let ports = assign_edge_ports(&positions, options.node_size, &endpoints).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &[0, 1, 2], options).unwrap();
        let mut index = RouteIndex::new(&[], options.edge_clearance).unwrap();
        for &ports in &ports {
            let route = router.route_edge(0, 1, ports, &index).unwrap();
            assert!(
                index.first_overlapping_segment(&route, 0.0).is_none(),
                "{backend:?}: {route:?}"
            );
            index.insert_route(&route).unwrap();
        }
    }
}

#[test]
fn conflicting_fixed_ports_report_no_path_instead_of_returning_overlap() {
    let positions = [Point::new(100.0, 100.0), Point::new(700.0, 100.0)];
    let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 100.0));
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let router = OrthogonalRouter::new(&positions, &[0, 1], options).unwrap();
    let index = RouteIndex::new(
        &[vec![Point::new(130.0, 100.0), Point::new(131.0, 100.0)]],
        options.edge_clearance,
    )
    .unwrap();
    assert_eq!(
        router.route_edge(0, 1, ports, &index),
        Err(RoutingError::NoOrthogonalPath)
    );
}

#[test]
fn lane_conflicts_allow_crossings_but_reject_parallel_overlap() {
    let index = RouteIndex::new(
        &[vec![Point::new(100.0, 100.0), Point::new(200.0, 100.0)]],
        DEFAULT_EDGE_CLEARANCE,
    )
    .unwrap();
    for reverse in [false, true] {
        for (start, end, clearance, expected) in [
            (Point::new(150.0, 0.0), Point::new(150.0, 200.0), 8.0, false),
            (
                Point::new(200.0, 100.0),
                Point::new(250.0, 100.0),
                8.0,
                false,
            ),
            (
                Point::new(150.0, 100.0),
                Point::new(151.0, 100.0),
                0.0,
                true,
            ),
            (
                Point::new(150.0, 106.0),
                Point::new(190.0, 106.0),
                8.0,
                true,
            ),
            (
                Point::new(150.0, 110.0),
                Point::new(190.0, 110.0),
                8.0,
                false,
            ),
        ] {
            let (start, end) = if reverse { (end, start) } else { (start, end) };
            assert_eq!(
                index.parallel_conflicts_segment(start, end, clearance),
                expected
            );
        }
    }
}

#[test]
fn crossings_do_not_bend_or_lengthen_direct_routes() {
    let positions = [Point::new(100.0, 100.0), Point::new(700.0, 100.0)];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(200.0, 200.0));
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let index = RouteIndex::new(
        &[vec![Point::new(400.0, 0.0), Point::new(400.0, 1_000.0)]],
        options.edge_clearance,
    )
    .unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &[0, 1], options).unwrap();
        let route = router.route_edge(0, 1, ports, &index).unwrap();
        assert_eq!(
            route,
            [Point::new(130.0, 100.0), Point::new(670.0, 100.0)],
            "{backend:?}"
        );
        assert!(index.intersects_route(&route));
    }
}

#[test]
fn crossings_are_allowed_regardless_of_the_other_route_length() {
    let positions = [Point::new(100.0, 100.0), Point::new(700.0, 100.0)];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(200.0, 200.0));
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &[0, 1], options).unwrap();
        for wall_end in [110.0, 552.0, 553.0, 10_000.0] {
            let index = RouteIndex::new(
                &[vec![Point::new(400.0, 0.0), Point::new(400.0, wall_end)]],
                options.edge_clearance,
            )
            .unwrap();
            let route = router.route_edge(0, 1, ports, &index).unwrap();
            assert!(index.intersects_route(&route), "{backend:?}: {route:?}");
            assert_eq!(route.len(), 2, "{backend:?}: {route:?}");
        }
    }
}

#[test]
fn shorter_routes_never_relax_node_obstacles() {
    let mut positions = vec![Point::new(100.0, 100.0), Point::new(700.0, 100.0)];
    positions.extend((0..17).map(|index| Point::new(400.0, 10.0 + index as f32 * 60.0)));
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(200.0, 200.0));
    let nodes = (0..positions.len()).collect::<Vec<_>>();
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
        let route = router.route_edge(0, 1, ports, &index).unwrap();
        assert_eq!(route.len(), 6, "{backend:?}: {route:?}");
        let length: f32 = route.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        assert!((length - 2356.0).abs() < 0.001, "{backend:?}: {route:?}");
        for position in positions.iter().skip(2) {
            let obstacle = Rect::from_center_size(*position, options.node_size)
                .expand(options.obstacle_clearance);
            assert!(
                route
                    .windows(2)
                    .all(|pair| { !segment_intersects_rect(pair[0], pair[1], obstacle) }),
                "{backend:?}: {route:?}"
            );
        }
    }
}

#[test]
fn detour_side_cost_is_independent_of_grid_subdivision() {
    let obstacle = Rect::from_min_max(Point::new(100.0, 100.0), Point::new(200.0, 200.0));
    for (start, end, direction) in [
        (Point::new(80.0, 220.0), Point::new(220.0, 220.0), Vector::X),
        (Point::new(220.0, 80.0), Point::new(220.0, 220.0), Vector::Y),
    ] {
        let expected = detour_side_preference_penalty(start, end, direction, &[obstacle]);
        assert_eq!(expected, GRAPH_ROUTE_SIDE_PREFERENCE_PENALTY);
        for subdivisions in [2, 7, 28] {
            let points = (0..=subdivisions)
                .map(|step| start + (end - start) * (step as f32 / subdivisions as f32))
                .collect::<Vec<_>>();
            for reverse in [false, true] {
                let cost: f32 = points
                    .windows(2)
                    .map(|segment| {
                        let [start, end] = if reverse {
                            [segment[1], segment[0]]
                        } else {
                            [segment[0], segment[1]]
                        };
                        detour_side_preference_penalty(start, end, direction, &[obstacle])
                    })
                    .sum();
                assert!((cost - expected).abs() < 0.001, "{cost} != {expected}");
            }
        }
        assert_eq!(
            detour_side_preference_penalty(start, start, direction, &[obstacle]),
            0.0
        );
    }
}

#[test]
fn dense_tracks_do_not_force_a_longer_or_more_bent_detour() {
    for transpose in [false, true] {
        let transform = |point: Point| {
            if transpose {
                Point::new(point.y, point.x)
            } else {
                point
            }
        };
        let mut positions = vec![
            Point::new(100.0, 300.0),
            Point::new(700.0, 300.0),
            Point::new(400.0, 300.0),
            Point::new(400.0, 250.0),
            Point::new(400.0, 190.0),
            Point::new(400.0, 130.0),
        ];
        // Distant nodes add tracks but do not obstruct either detour.
        positions.extend((0..20).map(|index| Point::new(360.0 + index as f32 * 4.0, 900.0)));
        let positions = positions.into_iter().map(transform).collect::<Vec<_>>();
        let node_size = if transpose {
            Size::new(40.0, 60.0)
        } else {
            Size::new(60.0, 40.0)
        };
        let mut options = OrthogonalRouterOptions::new(node_size, Size::new(250.0, 250.0));
        let nodes = (0..positions.len()).collect::<Vec<_>>();
        let ports = assign_edge_ports(&positions, node_size, &[(0, 1)]).unwrap()[0];
        let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
        for backend in [
            RoutingSearchBackend::Builtin,
            RoutingSearchBackend::Pathfinding,
            RoutingSearchBackend::Petgraph,
        ] {
            options.search_backend = backend;
            let router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
            let route = router.route_edge(0, 1, ports, &index).unwrap();
            assert_eq!(route.len(), 6, "{backend:?}: {route:?}");
            let length: f32 = route.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
            assert!(
                (length - 616.0).abs() < 0.001,
                "{backend:?}: {length}, {route:?}"
            );
            assert!(
                route.iter().any(|&point| {
                    let point = transform(point);
                    point.y == 338.0
                }),
                "{backend:?}: {route:?}"
            );
            for obstacle in positions.iter().skip(2) {
                let obstacle =
                    Rect::from_center_size(*obstacle, node_size).expand(options.obstacle_clearance);
                assert!(
                    route
                        .windows(2)
                        .all(|pair| { !segment_intersects_rect(pair[0], pair[1], obstacle) }),
                    "{backend:?}: {route:?}"
                );
            }
        }
    }
}

#[test]
fn dense_tracks_do_not_add_bends_between_obstacles() {
    let mut positions = vec![
        Point::new(100.0, 300.0),
        Point::new(700.0, 300.0),
        Point::new(300.0, 300.0),
        Point::new(500.0, 300.0),
    ];
    positions.extend([250.0, 190.0, 130.0, 70.0, 10.0].map(|y| Point::new(300.0, y)));
    positions.extend((0..20).map(|index| Point::new(460.0 + index as f32 * 4.0, 900.0)));
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(250.0, 250.0));
    let nodes = (0..positions.len()).collect::<Vec<_>>();
    let ports = assign_edge_ports(&positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &nodes, options).unwrap();
        let route = router.route_edge(0, 1, ports, &index).unwrap();
        assert_eq!(route.len(), 6, "{backend:?}: {route:?}");
        let length: f32 = route.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        assert!(
            (length - 616.0).abs() < 0.001,
            "{backend:?}: {length}, {route:?}"
        );
        for position in positions.iter().skip(2) {
            let obstacle = Rect::from_center_size(*position, options.node_size)
                .expand(options.obstacle_clearance);
            assert!(
                route
                    .windows(2)
                    .all(|pair| { !segment_intersects_rect(pair[0], pair[1], obstacle) }),
                "{backend:?}: {route:?}"
            );
        }
    }
}

#[test]
fn orthogonal_router_routes_around_obstacles_with_port_endpoints() {
    let node_positions = [
        Point::new(40.0, 80.0),
        Point::new(260.0, 80.0),
        Point::new(480.0, 80.0),
    ];
    let mut options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
    assert_eq!(options.search_backend, RoutingSearchBackend::Builtin);
    let edge_ports = assign_edge_ports(&node_positions, options.node_size, &[(0, 2)])
        .unwrap()
        .remove(0);
    let obstacle = Rect::from_center_size(node_positions[1], options.node_size)
        .expand(options.obstacle_clearance);

    for search_backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = search_backend;
        let router = OrthogonalRouter::new(&node_positions, &[0, 1, 2], options).unwrap();
        let routes = RouteIndex::new(&[], options.edge_clearance).unwrap();
        let route = router.route_edge(0, 2, edge_ports, &routes).unwrap();

        assert_eq!(route.first(), Some(&Point::new(70.0, 80.0)));
        assert_eq!(route.last(), Some(&Point::new(450.0, 80.0)));
        assert!(route.len() > 2);
        assert!(
            route
                .windows(2)
                .all(|segment| { segment[0].x == segment[1].x || segment[0].y == segment[1].y }),
            "{search_backend:?} route must remain orthogonal: {route:?}"
        );
        let bends = route
            .windows(3)
            .filter(|points| (points[0].x == points[1].x) != (points[1].x == points[2].x))
            .count();
        assert_eq!(
            bends, 4,
            "{search_backend:?} route should use the minimum four bends around this obstacle: {route:?}"
        );
        assert!(
            route
                .windows(2)
                .all(|segment| !segment_intersects_rect(segment[0], segment[1], obstacle)),
            "{search_backend:?} route must clear the middle node: {route:?}"
        );
    }
}

#[test]
fn edge_ports_use_neighbor_order_and_separate_parallel_lanes() {
    let positions = [Point::new(0.0, 0.0), Point::new(300.0, 0.0)];
    let endpoints = [(0, 1), (0, 1), (0, 1)];
    let ports = assign_edge_ports(&positions, Size::new(120.0, 80.0), &endpoints).unwrap();

    assert_eq!(
        ports
            .iter()
            .map(|port| port.source_offset)
            .collect::<Vec<_>>(),
        [-16.0, 0.0, 16.0]
    );
    assert!(
        ports
            .iter()
            .all(|port| port.source_side == NodeSide::Right && port.target_side == NodeSide::Left)
    );
}

#[test]
fn horizontal_axis_uses_side_facing_the_other_node_and_falls_back_within_a_column() {
    let positions = [
        Point::new(0.0, 0.0),
        Point::new(300.0, 150.0),
        Point::new(0.0, 300.0),
    ];
    let node_size = Size::new(120.0, 80.0);
    let horizontal = EdgePortAxes {
        source: PortAxis::Horizontal,
        target: PortAxis::Horizontal,
    };
    let ports = assign_edge_ports_on_axes(
        &positions,
        node_size,
        &[(0, 1), (0, 2), (1, 0)],
        &[horizontal; 3],
    )
    .unwrap();

    assert_eq!(
        (ports[0].source_side, ports[0].target_side),
        (NodeSide::Right, NodeSide::Left)
    );
    // Nodes 0 and 2 share a column, so their ports stay on the top and bottom sides.
    assert_eq!(
        (ports[1].source_side, ports[1].target_side),
        (NodeSide::Bottom, NodeSide::Top)
    );
    assert_eq!(
        (ports[2].source_side, ports[2].target_side),
        (NodeSide::Left, NodeSide::Right)
    );
    assert_eq!(
        assign_edge_ports_on_axes(&positions, node_size, &[(0, 1)], &[]).unwrap_err(),
        RoutingError::InvalidGeometry
    );
}

#[test]
fn unaligned_ports_connect_with_orthogonal_bends_centered_between_the_nodes() {
    let positions = [Point::new(0.0, 0.0), Point::new(300.0, 200.0)];
    let mut options = OrthogonalRouterOptions::new(Size::new(120.0, 80.0), Size::new(300.0, 200.0));
    let axes = [EdgePortAxes {
        source: PortAxis::Horizontal,
        target: PortAxis::Horizontal,
    }];
    let ports =
        assign_edge_ports_on_axes(&positions, options.node_size, &[(0, 1)], &axes).unwrap()[0];
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    for backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = backend;
        let router = OrthogonalRouter::new(&positions, &[0, 1], options).unwrap();
        let route = router.route_edge(0, 1, ports, &index).unwrap();
        assert_eq!(
            route,
            [
                Point::new(60.0, 0.0),
                Point::new(150.0, 0.0),
                Point::new(150.0, 200.0),
                Point::new(240.0, 200.0),
            ],
            "{backend:?}"
        );
    }
}

#[test]
fn parallel_routes_between_the_same_columns_bundle_around_the_midline() {
    let positions = [Point::new(200.0, 200.0), Point::new(500.0, 400.0)];
    let options = OrthogonalRouterOptions::new(Size::new(120.0, 80.0), Size::new(300.0, 200.0));
    let axes = [EdgePortAxes {
        source: PortAxis::Horizontal,
        target: PortAxis::Horizontal,
    }; 2];
    let ports =
        assign_edge_ports_on_axes(&positions, options.node_size, &[(0, 1); 2], &axes).unwrap();
    let router = OrthogonalRouter::new(&positions, &[0, 1], options).unwrap();
    let mut index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    let mut routes = Vec::new();
    for port in ports {
        let route = router.route_edge(0, 1, port, &index).unwrap();
        index.insert_route(&route).unwrap();
        routes.push(route);
    }

    let vertical_x = |route: &[Point]| {
        route
            .windows(2)
            .find(|segment| segment[0].x == segment[1].x)
            .map(|segment| segment[0].x)
            .unwrap()
    };
    // The midline between the two exits is x = 350; the first track sits on it and the second
    // track takes a neighbouring lane, so the pair stays within half a track of the midline.
    let track_spacing = options.edge_clearance + 2.0;
    let center = (vertical_x(&routes[0]) + vertical_x(&routes[1])) / 2.0;
    assert!((center - 350.0).abs() <= track_spacing / 2.0, "{routes:?}");
    assert!(
        (vertical_x(&routes[0]) - vertical_x(&routes[1])).abs() >= options.edge_clearance,
        "{routes:?}"
    );
}

#[test]
fn per_node_sizes_place_ports_on_each_card_border() {
    let positions = [Point::new(0.0, 0.0), Point::new(400.0, 0.0)];
    let sizes = [Size::new(240.0, 80.0), Size::new(120.0, 60.0)];
    let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(300.0, 200.0));
    let axes = [EdgePortAxes {
        source: PortAxis::Horizontal,
        target: PortAxis::Horizontal,
    }];
    let ports = assign_edge_ports_for_sizes(&positions, &sizes, &[(0, 1)], &axes).unwrap()[0];
    let router = OrthogonalRouter::with_node_sizes(&positions, &sizes, &[0, 1], options).unwrap();
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    let route = router.route_edge(0, 1, ports, &index).unwrap();

    assert_eq!(route.first(), Some(&Point::new(120.0, 0.0)));
    assert_eq!(route.last(), Some(&Point::new(340.0, 0.0)));
    assert!(
        assign_edge_ports_for_sizes(&positions, &sizes[..1], &[(0, 1)], &axes).is_err(),
        "one size for two nodes must be rejected"
    );
}

#[test]
fn route_index_detects_overlap_and_scales_the_penalty() {
    let index = RouteIndex::new(
        &[vec![Point::ZERO, Point::new(1_000.0, 0.0)]],
        DEFAULT_EDGE_CLEARANCE,
    )
    .unwrap();

    assert!(
        index
            .first_overlapping_segment(
                &[Point::new(20.0, 0.0), Point::new(980.0, 0.0)],
                GRAPH_ROUTE_SHARED_SEGMENT_VISIBLE_THRESHOLD,
            )
            .is_some()
    );
    assert_eq!(
        index.penalty(Point::ZERO, Point::new(1_000.0, 0.0),),
        GRAPH_EDGE_SHARED_SEGMENT_PENALTY_LIMIT
    );
}

#[test]
fn simplify_route_removes_collinear_backtracking() {
    let backtracking = [
        Point::ZERO,
        Point::new(20.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 20.0),
    ];
    assert_eq!(
        simplify_route(backtracking.to_vec()),
        [Point::ZERO, Point::new(10.0, 0.0), Point::new(10.0, 20.0)]
    );

    let looped = [
        Point::ZERO,
        Point::new(0.0, 10.0),
        Point::ZERO,
        Point::new(10.0, 0.0),
    ];
    assert_eq!(
        simplify_route(looped.to_vec()),
        [Point::ZERO, Point::new(10.0, 0.0)]
    );
}

#[test]
fn simplify_route_removes_closed_detours_but_preserves_closed_routes() {
    let route_with_detour = [
        Point::ZERO,
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(10.0, 0.0),
        Point::ZERO,
        Point::new(20.0, 0.0),
    ];
    assert_eq!(
        simplify_route(route_with_detour.to_vec()),
        [Point::ZERO, Point::new(20.0, 0.0)]
    );

    let closed_route = [
        Point::ZERO,
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(10.0, 0.0),
        Point::ZERO,
    ];
    assert_eq!(simplify_route(closed_route.to_vec()), closed_route);
}

#[test]
fn all_search_backends_avoid_previously_routed_segments() {
    let node_positions = [Point::new(100.0, 100.0), Point::new(700.0, 100.0)];
    let mut options = OrthogonalRouterOptions::new(Size::new(120.0, 80.0), Size::new(100.0, 80.0));
    let ports = assign_edge_ports(&node_positions, options.node_size, &[(0, 1)]).unwrap()[0];
    let existing = vec![vec![Point::new(260.0, 106.0), Point::new(540.0, 106.0)]];
    let route_index = RouteIndex::new(&existing, options.edge_clearance).unwrap();

    for search_backend in [
        RoutingSearchBackend::Builtin,
        RoutingSearchBackend::Pathfinding,
        RoutingSearchBackend::Petgraph,
    ] {
        options.search_backend = search_backend;
        let router = OrthogonalRouter::new(&node_positions, &[0, 1], options).unwrap();
        let route = router.route_edge(0, 1, ports, &route_index).unwrap();

        assert!(
            route.len() > 2,
            "{search_backend:?} should detour: {route:?}"
        );
        assert!(
            !route_index.parallel_conflicts_route(&route),
            "{search_backend:?} route overlaps an accepted route: {route:?}"
        );
        assert!(
            route
                .windows(2)
                .all(|segment| { segment[0].x == segment[1].x || segment[0].y == segment[1].y }),
            "{search_backend:?} route must remain orthogonal: {route:?}"
        );
        assert!(
            route.windows(3).all(|points| {
                cross_product(points[1] - points[0], points[2] - points[1]) != 0.0
            }),
            "{search_backend:?} route contains a redundant collinear bump: {route:?}"
        );
    }
}

#[test]
fn orthogonal_router_rejects_invalid_geometry_and_indices() {
    let invalid_options =
        OrthogonalRouterOptions::new(Size::new(0.0, 40.0), Size::new(100.0, 50.0));
    assert_eq!(
        OrthogonalRouter::new(&[Point::ZERO], &[0], invalid_options).unwrap_err(),
        RoutingError::InvalidGeometry
    );

    let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
    let router = OrthogonalRouter::new(&[Point::ZERO], &[0], options).unwrap();
    let ports = EdgePorts {
        source_side: NodeSide::Right,
        target_side: NodeSide::Left,
        source_offset: 0.0,
        target_offset: 0.0,
    };
    let index = RouteIndex::new(&[], options.edge_clearance).unwrap();
    assert_eq!(
        router.route_edge(0, 1, ports, &index).unwrap_err(),
        RoutingError::InvalidNodeIndex
    );
    assert_eq!(
        assign_edge_ports(&[Point::ZERO], options.node_size, &[(0, 2)]).unwrap_err(),
        RoutingError::InvalidNodeIndex
    );
}

#[test]
fn router_handles_negative_zero_route_coordinates_near_ports() {
    // The source escape lands exactly on x = 0.0 while the route uses x = -0.0.
    let node_positions = [
        Point::new(58.0, 100.0),
        Point::new(258.0, 100.0),
        Point::new(458.0, 100.0),
    ];
    let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
    let router = OrthogonalRouter::new(&node_positions, &[0, 1, 2], options).unwrap();
    let ports = EdgePorts {
        source_side: NodeSide::Left,
        target_side: NodeSide::Left,
        source_offset: 0.0,
        target_offset: 0.0,
    };
    let existing = vec![vec![Point::new(-0.0, 0.0), Point::new(-0.0, 300.0)]];
    let index = RouteIndex::new(&existing, options.edge_clearance).unwrap();

    let route = router.route_edge(0, 2, ports, &index).unwrap();

    assert_eq!(route.first(), Some(&Point::new(28.0, 100.0)));
    assert_eq!(route.last(), Some(&Point::new(428.0, 100.0)));
}
#[test]
fn edge_ports_stay_on_small_node_sides_in_neighbor_order() {
    let node_size = Size::new(20.0, 4.0);
    let positions = [
        Point::new(0.0, 0.0),
        Point::new(300.0, -10.0),
        Point::new(300.0, 10.0),
    ];
    let ports = assign_edge_ports(&positions, node_size, &[(0, 1), (0, 2)]).unwrap();

    assert!(ports.iter().all(|port| port.source_side == NodeSide::Right));
    assert!(
        ports
            .iter()
            .all(|port| port.source_offset.abs() <= node_size.height / 2.0),
        "ports must stay on the node side: {ports:?}"
    );
    assert!(
        ports[0].source_offset <= ports[1].source_offset,
        "ports must follow neighbor order: {ports:?}"
    );
}

#[test]
fn route_index_penalizes_near_crossings_across_cells_with_zero_clearance() {
    let index = RouteIndex::new(
        &[vec![Point::new(128.5, 0.0), Point::new(128.5, 100.0)]],
        0.0,
    )
    .unwrap();

    assert_eq!(
        index.penalty(Point::new(127.8, 0.0), Point::new(127.8, 100.0)),
        GRAPH_EDGE_CROSSING_PENALTY
    );
}
