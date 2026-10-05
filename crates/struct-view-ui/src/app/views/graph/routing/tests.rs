use super::*;

fn layout_graph(input: &str) -> RelationshipGraph {
    build_relationship_graph(&struct_view_core::parser::parse_json(input).unwrap())
}

fn routing_fixture(rows: usize) -> (GraphRoutingGrid, Vec<(usize, usize)>, Vec<GraphEdgePorts>) {
    let positions = (0..rows * 3)
        .map(|index| {
            Pos2::new(
                128.0 + (index % 3) as f32 * GRAPH_STEP.x,
                59.0 + (index / 3) as f32 * GRAPH_STEP.y,
            )
        })
        .collect::<Vec<_>>();
    let endpoints = (0..rows)
        .map(|row| (row * 3, row * 3 + 2))
        .collect::<Vec<_>>();
    let ports = graph_edge_ports(&positions, &endpoints);
    (GraphRoutingGrid::new(&positions), endpoints, ports)
}

mod layout;
mod routes;
mod view;
