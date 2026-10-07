use super::*;
use crate::RoutingError;

#[test]
fn orthogonal_router_routes_around_obstacles_with_port_endpoints() {
    let node_positions = [
        Point::new(40.0, 80.0),
        Point::new(260.0, 80.0),
        Point::new(480.0, 80.0),
    ];
    let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
    let router = OrthogonalRouter::new(&node_positions, &[0, 1, 2], options).unwrap();
    let edge_ports = assign_edge_ports(&node_positions, options.node_size, &[(0, 2)])
        .unwrap()
        .remove(0);
    let routes = RouteIndex::new(&[], options.edge_clearance).unwrap();
    let route = router.route_edge(0, 2, edge_ports, &routes).unwrap();
    let obstacle = Rect::from_center_size(node_positions[1], options.node_size)
        .expand(options.obstacle_clearance);

    assert_eq!(route.first(), Some(&Point::new(70.0, 80.0)));
    assert_eq!(route.last(), Some(&Point::new(450.0, 80.0)));
    assert!(route.len() > 2);
    assert!(
        route
            .windows(2)
            .all(|segment| { segment[0].x == segment[1].x || segment[0].y == segment[1].y }),
        "Orthogonal routes must contain only horizontal and vertical segments: {route:?}"
    );
    assert!(
        route
            .windows(2)
            .all(|segment| !segment_intersects_rect(segment[0], segment[1], obstacle)),
        "The path must clear the middle node: {route:?}"
    );
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
