use super::*;
use std::collections::HashSet;

type TestGraph = AdjacencyListGraph<&'static str, &'static str>;

fn graph_with_nodes(nodes: &[&'static str]) -> TestGraph {
    let mut graph = TestGraph::new();
    for &node in nodes {
        graph.add_node(node);
    }
    graph
}

fn add_edge(
    graph: &mut TestGraph,
    source: &'static str,
    target: &'static str,
    id: &'static str,
    weight: f64,
) {
    graph
        .add_edge(&source, &target, id, weight)
        .expect("test edge should be valid");
}

#[test]
fn adjacency_list_preserves_node_and_edge_order_and_validates_arc_ids() {
    let mut graph = TestGraph::new();
    assert_eq!(graph.add_node("first"), 0);
    assert_eq!(graph.add_node("second"), 1);
    assert_eq!(graph.add_node("first"), 0);
    assert_eq!(graph.node_ids(), ["first", "second"]);
    assert_eq!(graph.node_index(&"second"), Some(1));

    add_edge(&mut graph, "first", "second", "forward", 3.0);
    assert_eq!(
        graph.add_edge(&"second", &"first", "forward", 3.0),
        Err(GraphBuildError::DuplicateEdgeId)
    );
    assert_eq!(
        graph.add_edge(&"missing", &"second", "unknown-source", 1.0),
        Err(GraphBuildError::UnknownSource)
    );
    assert_eq!(
        graph.add_edge(&"first", &"missing", "unknown-target", 1.0),
        Err(GraphBuildError::UnknownTarget)
    );

    graph
        .add_undirected_edge(&"first", &"second", "back", "reverse", 2.0)
        .expect("distinct IDs should add both directed arcs");
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(graph.edges_from(&"first").unwrap().len(), 2);
    assert_eq!(graph.edges_from(&"second").unwrap().len(), 1);
}

#[test]
fn breadth_first_search_minimizes_hops_while_dijkstra_minimizes_weight() {
    let mut graph = graph_with_nodes(&["s", "a", "b", "t", "isolated"]);
    add_edge(&mut graph, "s", "a", "sa", 9.0);
    add_edge(&mut graph, "s", "b", "sb", 1.0);
    add_edge(&mut graph, "a", "t", "at", 1.0);
    add_edge(&mut graph, "b", "a", "ba", 1.0);
    add_edge(&mut graph, "b", "t", "bt", 20.0);

    let hops = breadth_first_search(&graph, "s", "t")
        .unwrap()
        .expect("a path exists");
    assert_eq!(hops.nodes, ["s", "a", "t"]);
    assert_eq!(hops.edges, ["sa", "at"]);
    assert_eq!(hops.edge_costs, [1.0, 1.0]);
    assert_eq!(hops.cost, 2.0);

    let weighted = dijkstra(&graph, "s", "t").unwrap().expect("a path exists");
    assert_eq!(weighted.nodes, ["s", "b", "a", "t"]);
    assert_eq!(weighted.edges, ["sb", "ba", "at"]);
    assert_eq!(weighted.cost, 3.0);
    assert_eq!(breadth_first_search(&graph, "s", "isolated").unwrap(), None);
    assert_eq!(
        breadth_first_search(&graph, "unknown", "t"),
        Err(RoutingError::UnknownNode)
    );
}

#[test]
fn dijkstra_and_a_star_find_the_same_minimum_cost_path() {
    let mut graph = graph_with_nodes(&["s", "a", "b", "t"]);
    add_edge(&mut graph, "s", "a", "sa", 9.0);
    add_edge(&mut graph, "s", "b", "sb", 1.0);
    add_edge(&mut graph, "a", "t", "at", 1.0);
    add_edge(&mut graph, "b", "a", "ba", 1.0);
    add_edge(&mut graph, "b", "t", "bt", 20.0);

    let expected = dijkstra(&graph, "s", "t").unwrap().unwrap();
    let actual = a_star(&graph, "s", "t", |node, _| match *node {
        "s" => 3.0,
        "a" => 1.0,
        "b" => 2.0,
        _ => 0.0,
    })
    .unwrap()
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        a_star(&graph, "s", "t", |_, _| f64::NAN),
        Err(RoutingError::InvalidHeuristic)
    );
}

#[test]
fn indexed_a_star_routes_dense_state_graphs_and_checks_inputs() {
    let path = a_star_indexed(
        4,
        0,
        |node| node == 3,
        |node, neighbors| match node {
            0 => neighbors.extend([
                IndexedNeighbor { node: 1, cost: 1.0 },
                IndexedNeighbor { node: 2, cost: 4.0 },
            ]),
            1 => neighbors.push(IndexedNeighbor { node: 3, cost: 1.0 }),
            2 => neighbors.push(IndexedNeighbor { node: 3, cost: 1.0 }),
            _ => {}
        },
        |node| if node == 0 { 2.0 } else { 0.0 },
    )
    .unwrap()
    .unwrap();
    assert_eq!(path.nodes, [0, 1, 3]);
    assert_eq!(path.cost, 2.0);

    assert_eq!(
        a_star_indexed(1, 1, |_| true, |_, _| {}, |_| 0.0),
        Err(RoutingError::InvalidNodeIndex)
    );
    assert_eq!(
        a_star_indexed(1, 0, |_| false, |_, _| {}, |_| f64::NAN),
        Err(RoutingError::InvalidHeuristic)
    );
    assert_eq!(
        a_star_indexed(
            1,
            0,
            |_| false,
            |_, neighbors| neighbors.push(IndexedNeighbor { node: 1, cost: 1.0 }),
            |_| 0.0
        ),
        Err(RoutingError::InvalidNodeIndex)
    );
    assert_eq!(
        a_star_indexed(
            2,
            0,
            |node| node == 1,
            |_, neighbors| neighbors.push(IndexedNeighbor {
                node: 1,
                cost: -1.0
            }),
            |_| 0.0
        ),
        Err(RoutingError::NegativeWeight)
    );
}

#[test]
fn bellman_ford_supports_negative_edges_and_rejects_only_reachable_negative_cycles() {
    let mut graph = graph_with_nodes(&["s", "a", "b", "t", "orphan"]);
    add_edge(&mut graph, "s", "a", "sa", 4.0);
    add_edge(&mut graph, "s", "b", "sb", 5.0);
    add_edge(&mut graph, "a", "b", "ab", -2.0);
    add_edge(&mut graph, "b", "t", "bt", 3.0);
    add_edge(&mut graph, "a", "t", "at", 10.0);

    let paths = bellman_ford(&graph, "s").unwrap();
    assert_eq!(paths.source(), &"s");
    assert_eq!(paths.distance_to(&"t"), Some(5.0));
    assert_eq!(paths.distance_to(&"orphan"), None);
    assert_eq!(
        paths.path_to(&"t").unwrap(),
        Path {
            nodes: vec!["s", "a", "b", "t"],
            edges: vec!["sa", "ab", "bt"],
            edge_costs: vec![4.0, -2.0, 3.0],
            cost: 5.0,
        }
    );
    assert_eq!(
        paths.path_to(&"s").unwrap(),
        Path {
            nodes: vec!["s"],
            edges: Vec::<&str>::new(),
            edge_costs: Vec::new(),
            cost: 0.0,
        }
    );

    let mut graph = graph_with_nodes(&["source", "a", "b", "orphan"]);
    add_edge(&mut graph, "source", "a", "sa", 1.0);
    add_edge(&mut graph, "a", "b", "ab", -2.0);
    add_edge(&mut graph, "b", "a", "ba", 0.0);
    assert_eq!(
        bellman_ford(&graph, "source"),
        Err(RoutingError::NegativeCycle)
    );
    assert_eq!(
        bellman_ford(&graph, "orphan")
            .unwrap()
            .path_to(&"orphan")
            .unwrap()
            .cost,
        0.0
    );
}

#[test]
fn floyd_warshall_reconstructs_paths_and_rejects_any_negative_cycle() {
    let mut graph = graph_with_nodes(&["a", "b", "c", "d", "isolated"]);
    add_edge(&mut graph, "a", "b", "ab", 5.0);
    add_edge(&mut graph, "a", "c", "ac", 10.0);
    add_edge(&mut graph, "b", "c", "bc", -4.0);
    add_edge(&mut graph, "c", "d", "cd", 3.0);
    add_edge(&mut graph, "b", "d", "bd", 10.0);

    let paths = floyd_warshall(&graph).unwrap();
    assert_eq!(paths.distance(&"a", &"d"), Some(4.0));
    assert_eq!(
        paths.path(&"a", &"d").unwrap(),
        Path {
            nodes: vec!["a", "b", "c", "d"],
            edges: vec!["ab", "bc", "cd"],
            edge_costs: vec![5.0, -4.0, 3.0],
            cost: 4.0,
        }
    );
    assert_eq!(paths.distance(&"d", &"a"), None);
    assert_eq!(paths.distance(&"isolated", &"isolated"), Some(0.0));

    let mut graph = graph_with_nodes(&["x", "y", "isolated"]);
    add_edge(&mut graph, "x", "y", "xy", 1.0);
    add_edge(&mut graph, "y", "x", "yx", -2.0);
    assert_eq!(floyd_warshall(&graph), Err(RoutingError::NegativeCycle));
}

#[test]
fn yen_returns_unique_loopless_paths_in_cost_order() {
    let mut graph = graph_with_nodes(&["s", "a", "b", "c", "t"]);
    add_edge(&mut graph, "s", "a", "sa", 1.0);
    add_edge(&mut graph, "a", "t", "at", 1.0);
    add_edge(&mut graph, "s", "b", "sb", 2.0);
    add_edge(&mut graph, "b", "t", "bt", 1.0);
    add_edge(&mut graph, "s", "c", "sc", 2.0);
    add_edge(&mut graph, "c", "t", "ct", 2.0);
    add_edge(&mut graph, "a", "b", "ab", 1.0);
    add_edge(&mut graph, "b", "a", "ba", 1.0);

    let paths = yen_k_shortest_paths(&graph, "s", "t", 10).unwrap();
    assert_eq!(
        paths.iter().map(|path| path.cost).collect::<Vec<_>>(),
        [2.0, 3.0, 3.0, 4.0, 4.0]
    );
    for (index, path) in paths.iter().enumerate() {
        assert_eq!(path.nodes.first(), Some(&"s"));
        assert_eq!(path.nodes.last(), Some(&"t"));
        assert_eq!(path.edges.len() + 1, path.nodes.len());
        assert_eq!(path.edge_costs.len(), path.edges.len());
        assert_eq!(
            path.nodes.iter().collect::<HashSet<_>>().len(),
            path.nodes.len(),
            "Yen paths must be loopless"
        );
        assert!(
            paths[..index]
                .iter()
                .all(|previous| previous.nodes != path.nodes || previous.edges != path.edges)
        );
    }
    assert!(
        yen_k_shortest_paths(&graph, "s", "t", 0)
            .unwrap()
            .is_empty()
    );

    let empty = graph_with_nodes(&["s", "t"]);
    assert!(
        yen_k_shortest_paths(&empty, "s", "t", 3)
            .unwrap()
            .is_empty()
    );
    let source_path = yen_k_shortest_paths(&empty, "s", "s", 3).unwrap();
    assert_eq!(source_path.len(), 1);
    assert_eq!(source_path[0].cost, 0.0);
}

#[test]
fn shortest_path_algorithms_validate_weight_assumptions() {
    let mut negative = graph_with_nodes(&["s", "t"]);
    add_edge(&mut negative, "s", "t", "negative", -1.0);
    assert_eq!(
        dijkstra(&negative, "s", "t"),
        Err(RoutingError::NegativeWeight)
    );
    assert_eq!(
        yen_k_shortest_paths(&negative, "s", "t", 1),
        Err(RoutingError::NegativeWeight)
    );
    assert_eq!(
        breadth_first_search(&negative, "s", "t")
            .unwrap()
            .unwrap()
            .cost,
        1.0
    );

    let mut non_finite = graph_with_nodes(&["s", "t"]);
    add_edge(&mut non_finite, "s", "t", "nan", f64::NAN);
    assert_eq!(
        bellman_ford(&non_finite, "s"),
        Err(RoutingError::NonFiniteWeight)
    );
}
