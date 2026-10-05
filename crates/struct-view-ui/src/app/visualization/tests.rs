use super::*;

use struct_view_core::parser::{DataFormat, parse_data};
use struct_view_core::search::{SearchOptions, SearchState};

#[test]
fn graph_node_hover_information_preserves_all_attributes_in_all_formats() {
    for source in [
        r#"{"graph":{"type":"directed","nodes":[{"id":"a","label":"Alpha","role":"Gateway","active":false,"count":0,"empty":"","nullable":null,"config":{"region":"West","ports":[80,443]},"tags":[],"custom.key":"custom"}],"edges":[]}}"#,
        r#"{"directed":true,"nodes":{"a":{"label":"Alpha","role":"Gateway","active":false,"count":0,"empty":"","nullable":null,"config":{"region":"West","ports":[80,443]},"tags":[],"custom.key":"custom"}},"edges":[]}"#,
        r#"{"items":[{"id":"a","label":"Alpha","role":"Gateway","active":false,"count":0,"empty":"","nullable":null,"config":{"region":"West","ports":[80,443]},"tags":[],"custom.key":"custom"}]}"#,
        r#"{"elements":[{"data":{"id":"a","label":"Alpha","role":"Gateway","active":false,"count":0,"empty":"","nullable":null,"config":{"region":"West","ports":[80,443]},"tags":[],"custom.key":"custom"},"position":{"x":12,"y":34}}]}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        for format in DataFormat::ALL {
            // TOML has no null: test it separately from formats which retain null.
            let mut serializable = root.clone();
            fn remove_null(node: &mut struct_view_core::parser::JsonNode) {
                node.children.retain(|child| {
                    child.value_type != struct_view_core::parser::JsonValueType::Null
                });
                for child in &mut node.children {
                    remove_null(child);
                }
            }
            if format == DataFormat::Toml {
                remove_null(&mut serializable);
            }
            let text =
                struct_view_core::parser::serialize_node(&serializable, format, false).unwrap();
            let parsed = parse_data(&text, Some(format)).unwrap().0;
            let graph = try_build_relationship_graph(&parsed).unwrap();
            let node = &graph.nodes[0];
            let prefix = if source.contains("elements") {
                "data."
            } else {
                ""
            };
            for (key, value) in [
                ("role", "\"Gateway\""),
                ("active", "false"),
                ("count", "0"),
                ("empty", "\"\""),
                ("config.region", "\"West\""),
                ("config.ports[0]", "80"),
                ("config.ports[1]", "443"),
                ("tags", "[0]"),
            ] {
                assert!(
                    node.attributes
                        .contains(&(format!("{prefix}{key}"), value.to_string())),
                    "{format:?}: {:?}",
                    node.attributes
                );
            }
            if format != DataFormat::Toml {
                assert!(
                    node.attributes
                        .contains(&(format!("{prefix}nullable"), "null".to_string()))
                );
            }
            assert!(
                node.attributes
                    .iter()
                    .any(|(key, value)| key.contains("custom.key") && value == "\"custom\"")
            );
            let tooltip = node.hover_text();
            assert!(tooltip.contains("role: \"Gateway\""));
            assert!(tooltip.contains(&node.path));
            assert_eq!(graph.nodes.len(), 1);
            assert!(graph.edges.is_empty());
            if !prefix.is_empty() {
                assert!(
                    node.attributes
                        .contains(&("position.x".to_string(), "12".to_string()))
                );
            }
        }
    }
}

#[test]
fn graph_node_attributes_keep_native_dates_and_yaml_tags() {
    for (format, source, expected) in [
        (
            DataFormat::Toml,
            "directed = true\nedges = []\n[[nodes]]\nid = 'a'\ncreated = 2026-10-05T11:21:00\n",
            "2026-10-05T11:21:00",
        ),
        (
            DataFormat::Yaml,
            "directed: true\nnodes:\n  - id: a\n    role: !Role Gateway\nedges: []\n",
            "Gateway",
        ),
    ] {
        let root = parse_data(source, Some(format)).unwrap().0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert!(graph.nodes[0].hover_text().contains(expected));
    }
    let root = parse_data(
        r#"{"graph":{"type":"directed"},"adjacency":{"a":{"b":2}}}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    assert!(
        try_build_relationship_graph(&root)
            .unwrap()
            .nodes
            .iter()
            .all(|node| node.attributes.is_empty())
    );
}

#[test]
fn graph_edge_hover_information_preserves_custom_fields_in_all_formats() {
    for source in [
        r#"{"graph":{"type":"directed"},"nodes":["a","b"],"edges":[{"source":"a","target":"b","status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]}]}"#,
        r#"{"graph":{"type":"directed","nodes":["a","b"],"links":{"ab":{"source":"a","target":"b","status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]}}}}"#,
        r#"{"directed":true,"multigraph":true,"graph":{},"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]}],[]]}"#,
        r#"{"directed":true,"nodes":["a","b"],"adjacency":{"a":[{"target":"b","status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]}],"b":[]}}"#,
        r#"{"directed":true,"nodes":["a","b"],"edges":[["a","b",{"status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]}]]}"#,
        r#"{"elements":{"nodes":[{"data":{"id":"a"}},{"data":{"id":"b"}}],"edges":[{"data":{"source":"a","target":"b","status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]},"classes":"channel"}]}}"#,
        r#"{"elements":[{"data":{"id":"a"}},{"data":{"id":"b"}},{"data":{"source":"a","target":"b","status":"online","active":false,"empty":"","config":{"latency":12,"ports":[80,443]},"tags":[]},"classes":"channel"}]}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        for format in DataFormat::ALL {
            let text = struct_view_core::parser::serialize_node(&root, format, false).unwrap();
            let parsed = parse_data(&text, Some(format)).unwrap().0;
            let graph = try_build_relationship_graph(&parsed).unwrap();
            assert_eq!(graph.edges.len(), 1);
            let edge = &graph.edges[0];
            let prefix = if source.contains("elements") {
                "data."
            } else {
                ""
            };
            for (key, value) in [
                ("status", "\"online\""),
                ("active", "false"),
                ("empty", "\"\""),
                ("config.latency", "12"),
                ("config.ports[0]", "80"),
                ("config.ports[1]", "443"),
                ("tags", "[0]"),
            ] {
                assert!(
                    edge.attributes
                        .contains(&(format!("{prefix}{key}"), value.to_string())),
                    "{format:?}: {:?}",
                    edge.attributes
                );
            }
            assert!(edge.hover_text().contains("status: \"online\""));
            if !prefix.is_empty() {
                assert!(
                    edge.attributes
                        .contains(&("classes".to_string(), "\"channel\"".to_string()))
                );
            }
        }
    }
}

#[test]
fn graph_parallel_and_mirrored_edges_do_not_lose_custom_fields() {
    for (source, expected_count) in [
        (
            r#"{"directed":true,"multigraph":true,"nodes":["a","b"],"edges":[{"source":"a","target":"b","status":"primary"},{"source":"a","target":"b","status":"backup"}]}"#,
            2,
        ),
        (
            r#"{"directed":true,"nodes":["a","b"],"edges":[{"source":"a","target":"b","status":"primary"},{"source":"a","target":"b","status":"backup"}]}"#,
            1,
        ),
        (
            r#"{"directed":false,"multigraph":true,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"status":"primary"}],[{"id":"a","key":0,"status":"backup"}]]}"#,
            1,
        ),
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert_eq!(graph.edges.len(), expected_count);
        let attributes = graph
            .edges
            .iter()
            .flat_map(|edge| &edge.attributes)
            .collect::<Vec<_>>();
        assert!(
            attributes
                .iter()
                .any(|(key, value)| key == "status" && value == "\"primary\"")
        );
        assert!(
            attributes
                .iter()
                .any(|(key, value)| key == "status" && value == "\"backup\"")
        );
    }
}

#[test]
fn imported_graph_nodes_and_edges_keep_custom_attributes_for_hover() {
    for source in [
        r#"digraph { a [role="Gateway"]; b; a -> b [status="online"]; }"#,
        r#"<graphml><key id="role" for="node" attr.name="role" attr.type="string"/><key id="status" for="edge" attr.name="status" attr.type="string"/><graph edgedefault="directed"><node id="a"><data key="role">Gateway</data></node><node id="b"/><edge source="a" target="b"><data key="status">online</data></edge></graph></graphml>"#,
        r#"<gexf><graph defaultedgetype="directed"><attributes class="node"><attribute id="role" title="role" type="string"/></attributes><attributes class="edge"><attribute id="status" title="status" type="string"/></attributes><nodes><node id="a"><attvalues><attvalue for="role" value="Gateway"/></attvalues></node><node id="b"/></nodes><edges><edge source="a" target="b"><attvalues><attvalue for="status" value="online"/></attvalues></edge></edges></graph></gexf>"#,
    ] {
        let root = parse_data(source, None).unwrap().0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert!(graph.nodes[0].hover_text().contains("Gateway"));
        assert!(graph.edges[0].hover_text().contains("online"));
    }
}

#[test]
fn table_paths_and_csv_preserve_nested_fields() {
    let root = parse_data(
        r#"["line, one",{"quoted\"key":true}]"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let table = build_table(&root);

    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| row.path.as_str())
            .collect::<Vec<_>>(),
        ["$", "$[0]", "$[1]", "$[1][\"quoted\\\"key\"]"]
    );
    let csv = table_to_csv(&table, &SearchState::default());
    assert!(csv.starts_with("path,value,type\r\n"));
    assert!(csv.contains("\"line, one\""));
}

#[test]
fn table_filter_uses_tree_search_matches() {
    let root = parse_data(r#"{"keep":1,"drop":2}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let table = build_table(&root);
    let mut search = SearchState::default();
    search.search(&root, "keep");

    let visible = table_visible_indices(&table, &search);
    assert_eq!(
        visible
            .unwrap()
            .iter()
            .map(|index| table.rows[*index].path.as_str())
            .collect::<Vec<_>>(),
        ["$.keep"]
    );
}

#[test]
fn csv_escapes_quotes_and_line_breaks() {
    assert_eq!(csv_field("a,\"b\"\nline"), "\"a,\"\"b\"\"\nline\"");
}

#[test]
fn graph_resolves_entity_ids_and_dependency_fields() {
    let root = parse_data(
            r#"{"services":[{"id":"api","name":"API","depends_on":"db"},{"id":"db","name":"Database"}]}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
    let graph = build_relationship_graph(&root);

    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(
        graph.edges,
        [GraphEdge {
            source: 0,
            target: 1,
            label: "depends_on".to_string(),
            direction: struct_view_core::graph::EdgeDirection::Directed,
            attributes: Vec::new(),
        }]
    );
}

#[test]
fn graph_reads_explicit_json_nodes_and_edges() {
    let root = parse_data(
        r#"{
            "graph": {"name": "Order processing", "type": "directed"},
            "nodes": [
                {"id": "created", "label": "Created"},
                {"id": "paid", "label": "Paid"},
                {"id": "shipped", "label": "Shipped"},
                {"id": "cancelled", "label": "Cancelled"}
            ],
            "edges": [
                {"source": "created", "target": "paid"},
                {"source": "paid", "target": "shipped"},
                {"source": "created", "target": "cancelled"}
            ]
        }"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);

    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        ["Created", "Paid", "Shipped", "Cancelled"]
    );
    assert_eq!(
        graph.edges,
        [
            GraphEdge {
                source: 0,
                target: 1,
                label: String::new(),
                direction: struct_view_core::graph::EdgeDirection::Directed,
                attributes: vec![
                    ("source".to_string(), "\"created\"".to_string()),
                    ("target".to_string(), "\"paid\"".to_string())
                ],
            },
            GraphEdge {
                source: 0,
                target: 3,
                label: String::new(),
                direction: struct_view_core::graph::EdgeDirection::Directed,
                attributes: vec![
                    ("source".to_string(), "\"created\"".to_string()),
                    ("target".to_string(), "\"cancelled\"".to_string())
                ],
            },
            GraphEdge {
                source: 1,
                target: 2,
                label: String::new(),
                direction: struct_view_core::graph::EdgeDirection::Directed,
                attributes: vec![
                    ("source".to_string(), "\"paid\"".to_string()),
                    ("target".to_string(), "\"shipped\"".to_string())
                ],
            },
        ]
    );
}

#[test]
fn graph_preserves_per_edge_directions_and_loops_in_all_data_formats() {
    use struct_view_core::graph::EdgeDirection;
    let source = r#"{
        "graph":{"type":"mixed_multigraph"},
        "nodes":[{"id":"a"},{"id":"b"}],
        "edges":[
            {"source":"a","target":"b","direction":"directed"},
            {"source":"a","target":"b","direction":"bidirectional"},
            {"source":"a","target":"b","direction":"reverse"},
            {"source":"a","target":"b","direction":"undirected"},
            {"source":"a","target":"a","direction":"bidirectional"}
        ]}"#;
    let (root, _) = parse_data(source, Some(DataFormat::Json)).unwrap();
    for format in DataFormat::ALL {
        let text = struct_view_core::parser::serialize_node(&root, format, false).unwrap();
        let (root, _) = parse_data(&text, Some(format)).unwrap();
        let graph = try_build_relationship_graph(&root).unwrap();
        assert_eq!(graph.direction_name(), "mixed");
        assert_eq!(graph.edges.len(), 5);
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.direction)
                .collect::<Vec<_>>(),
            [
                EdgeDirection::Directed,
                EdgeDirection::Bidirectional,
                EdgeDirection::Reverse,
                EdgeDirection::Undirected,
                EdgeDirection::Bidirectional
            ]
        );
        assert_eq!(graph.edges[4].source, graph.edges[4].target);
    }
    for direction in ["bidirectional", "mutual", "both"] {
        let (root, _) = parse_data(&format!(r#"{{"graph":{{"type":"bidirectional"}},"nodes":[{{"id":"a"}},{{"id":"b"}}],"edges":[{{"source":"a","target":"b","direction":"{direction}"}}]}}"#), Some(DataFormat::Json)).unwrap();
        assert_eq!(
            build_relationship_graph(&root).edges[0].direction,
            EdgeDirection::Bidirectional
        );
    }
}

#[test]
fn graph_direction_errors_are_not_silently_rendered_as_forward_arrows() {
    for field in [
        r#""direction":"invalid""#,
        r#""direction":{}"#,
        r#""directed":"invalid""#,
    ] {
        let root = parse_data(&format!(r#"{{"graph":{{"type":"directed"}},"nodes":[{{"id":"a"}},{{"id":"b"}}],"edges":[{{"source":"a","target":"b",{field}}}]}}"#), Some(DataFormat::Json)).unwrap().0;
        assert!(try_build_relationship_graph(&root).is_err());
    }
}
#[test]
fn nested_mesh_graph_keeps_exact_nodes_edges_weights_directions_and_paths() {
    use struct_view_core::graph::EdgeDirection;
    let source = r#"{"graph":{"id":"mesh_network","type":"undirected","nodes":[
            {"id":"A","label":"Node A","role":"Gateway"},{"id":"B","label":"Node B","role":"Router"},
            {"id":"C","label":"Node C","role":"Core"},{"id":"D","label":"Node D","role":"Router"},
            {"id":"E","label":"Node E","role":"Gateway"}],"edges":[
            {"source":"A","target":"B","weight":1.5,"bidirectional":true},
            {"source":"A","target":"C","weight":2.0,"bidirectional":true},
            {"source":"B","target":"C","weight":1.0,"bidirectional":true},
            {"source":"B","target":"D","weight":3.2,"bidirectional":true},
            {"source":"C","target":"D","weight":1.8,"bidirectional":true},
            {"source":"C","target":"E","weight":2.5,"bidirectional":true},
            {"source":"D","target":"E","weight":1.2,"bidirectional":true}]}}"#;
    let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
    for format in DataFormat::ALL {
        let text = struct_view_core::parser::serialize_node(&root, format, false).unwrap();
        let parsed = parse_data(&text, Some(format)).unwrap().0;
        let graph = try_build_relationship_graph(&parsed).unwrap();
        assert_eq!(graph.nodes.len(), 5);
        assert_eq!(graph.edges.len(), 7);
        assert_eq!(graph.direction_name(), "bidirectional");
        assert_eq!(graph.nodes[0].label, "Node A");
        assert_eq!(graph.nodes[0].path, "graph.nodes[0]");
        assert!(graph.nodes.iter().all(|node| node.id != "mesh_network"));
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| (edge.source, edge.target, edge.label.as_str()))
                .collect::<Vec<_>>(),
            [
                (0, 1, "1.5"),
                (0, 2, "2.0"),
                (1, 2, "1.0"),
                (1, 3, "3.2"),
                (2, 3, "1.8"),
                (2, 4, "2.5"),
                (3, 4, "1.2")
            ]
        );
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.direction == EdgeDirection::Bidirectional)
        );
    }
}

#[test]
fn bidirectional_boolean_and_nested_graph_validation_are_explicit() {
    use struct_view_core::graph::EdgeDirection;
    let root = parse_data(
        r#"{"graph":{"type":"directed","nodes":{"a":"A","b":"B"},"edges":[
            {"source":"a","target":"b","bidirectional":false},
            {"source":"a","target":"b","bidirectional":true}]}}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let graph = try_build_relationship_graph(&root).unwrap();
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.edges[0].direction, EdgeDirection::Directed);
    assert_eq!(graph.edges[1].direction, EdgeDirection::Bidirectional);
    for source in [
        r#"{"graph":{"type":"directed","nodes":[]}}"#,
        r#"{"graph":{"type":"unknown","nodes":[],"edges":[]}}"#,
        r#"{"graph":{"type":"directed","nodes":[],"edges":"invalid"}}"#,
        r#"{"graph":{"type":"directed","nodes":[],"edges":[]},"nodes":[]}"#,
        r#"{"graph":{"type":"directed"},"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b","bidirectional":"true"}]}"#,
        r#"{"graph":{"type":"directed"},"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b","direction":"reverse","bidirectional":true}]}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        assert!(try_build_relationship_graph(&root).is_err(), "{source}");
    }
}

#[test]
fn graph_tuple_formats_and_adjacency_preserve_loop_and_direction_semantics() {
    use struct_view_core::graph::EdgeDirection;
    for (source, expected) in [
        (
            r#"{"graph":{"type":"undirected"},"adjacency":{"a":["a","b"],"b":["a"]}}"#,
            vec![EdgeDirection::Undirected; 2],
        ),
        (
            r#"{"graph":{"type":"weighted_undirected","node_order":["a","b"],"adjacency_matrix":[[2,3],[3,0]]}}"#,
            vec![EdgeDirection::Undirected; 2],
        ),
        (
            r#"{"graph":{"name":"entities","directed":true,"entity_count":2,"relation_count":2},"entities":{"a":"A","b":"B"},"relations":{"edges":[["a","a","loop","","bidirectional"],["a","b","back","","reverse"]]}}"#,
            vec![EdgeDirection::Bidirectional, EdgeDirection::Reverse],
        ),
        (
            r#"{"graph":{"type":"bipartite","direction":"bidirectional"},"partitions":{"left":["a"],"right":["b"]},"relations":{"pairs":[["a","b","both"],["a","a","loop","reverse"]]}}"#,
            vec![EdgeDirection::Reverse, EdgeDirection::Bidirectional],
        ),
        (
            r#"{"graph":{"type":"mixed_multigraph"},"nodes":{"a":"A","b":"B"},"edges":[{"source":"a","target":"a","direction":"both"},{"source":"a","target":"b","directed":false}]}"#,
            vec![EdgeDirection::Bidirectional, EdgeDirection::Undirected],
        ),
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.direction)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(graph.edges.iter().any(|edge| edge.source == edge.target));
    }
    let root = parse_data(r#"[{"id":"a","ref":"a"}]"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    assert_eq!(build_relationship_graph(&root).edges.len(), 1);
}

#[test]
fn graph_adjacency_and_matrix_support_direction_defaults_without_losing_tiny_loops() {
    use struct_view_core::graph::EdgeDirection;
    for (kind, direction) in [
        ("directed", EdgeDirection::Directed),
        ("bidirectional", EdgeDirection::Bidirectional),
        ("undirected", EdgeDirection::Undirected),
    ] {
        let root = parse_data(
            &format!(r#"{{"graph":{{"type":"{kind}"}},"adjacency":{{"a":["a","b"],"b":[]}}}}"#),
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert_eq!(graph.edges.len(), 2);
        assert!(graph.edges.iter().all(|edge| edge.direction == direction));
        let root = parse_data(&format!(r#"{{"graph":{{"type":"weighted_{kind}","node_order":["a","b"],"adjacency_matrix":[[1e-1000,2],[null,0]]}}}}"#), Some(DataFormat::Json)).unwrap().0;
        let graph = try_build_relationship_graph(&root).unwrap();
        assert_eq!(
            graph.edges.len(),
            2,
            "tiny nonzero diagonal must not disappear"
        );
        assert!(graph.edges.iter().all(|edge| edge.direction == direction));
    }
    let root = parse_data(r#"{"graph":{"type":"directed","direction":"reverse","node_order":["a","b"],"adjacency_matrix":[[0,null],[3,0]]}}"#, Some(DataFormat::Json)).unwrap().0;
    let graph = try_build_relationship_graph(&root).unwrap();
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].direction, EdgeDirection::Reverse);
    assert_eq!((graph.edges[0].source, graph.edges[0].target), (1, 0));
}

#[test]
fn graph_reads_all_explicit_json_graph_types() {
    for (graph_type, directed, multigraph) in [
        ("directed", true, false),
        ("weighted_directed", true, false),
        ("undirected", false, false),
        ("weighted_undirected", false, false),
        ("directed_multigraph", true, true),
        ("undirected_multigraph", false, true),
    ] {
        let input = serde_json::json!({
            "graph": {
                "type": graph_type,
                "weight_unit": "kg"
            },
            "nodes": [
                {"id": "a"},
                {"id": "b"}
            ],
            "edges": [
                {
                    "source": "a",
                    "target": "b",
                    "label": "connects",
                    "weight": 0
                },
                {
                    "source": "a",
                    "target": "b",
                    "label": "connects",
                    "weight": 0
                }
            ]
        })
        .to_string();
        let root = parse_data(&input, Some(DataFormat::Json)).unwrap().0;
        let graph = build_relationship_graph(&root);

        assert_eq!(graph.directed, directed, "{graph_type}");
        assert_eq!(
            graph.edges.len(),
            if multigraph { 2 } else { 1 },
            "{graph_type}"
        );
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.label == "connects · 0 kg"),
            "{graph_type}"
        );
    }
}

#[test]
fn graph_reads_weighted_undirected_yaml_adjacency_matrix() {
    let root = parse_data(
        r#"
            graph:
              name: "Расстояния между городами"
              type: "weighted_undirected"
              weight_unit: "км"
              node_order: ["Москва", "Тверь", "Тула"]
              adjacency_matrix:
                - [0, 180, 0]
                - [180, 0, null]
                - [0, null, 0]
        "#,
        Some(DataFormat::Yaml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);

    assert!(!graph.directed);
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["Москва", "Тверь", "Тула"]
    );
    assert_eq!(
        graph.edges,
        [
            GraphEdge {
                source: 0,
                target: 1,
                label: "180 км".to_string(),
                direction: struct_view_core::graph::EdgeDirection::Undirected,
                attributes: Vec::new(),
            },
            GraphEdge {
                source: 0,
                target: 2,
                label: "0 км".to_string(),
                direction: struct_view_core::graph::EdgeDirection::Undirected,
                attributes: Vec::new(),
            },
        ]
    );
}

#[test]
fn graph_reads_explicit_toml_entities_and_relation_tuples() {
    let root = parse_data(
        r#"
            [graph]
            name = "Store"
            directed = true
            entity_count = 3
            relation_count = 2

            [entities]
            customer = "Buyer"
            order = "Order"
            product = "Product"

            [relations]
            edges = [
                ["customer", "order", "places", "1:N"],
                ["order", "product", "contains", "1:N"],
            ]
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);

    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.nodes.iter().any(|node| {
        node.id == "customer" && node.label == "Buyer" && node.path == "entities.customer"
    }));
    assert!(graph.edges.iter().any(|edge| {
        graph.nodes[edge.source].id == "customer"
            && graph.nodes[edge.target].id == "order"
            && edge.label == "places (1:N)"
    }));
    assert!(graph.edges.iter().any(|edge| {
        graph.nodes[edge.source].id == "order"
            && graph.nodes[edge.target].id == "product"
            && edge.label == "contains (1:N)"
    }));
}

#[test]
fn graph_reads_bipartite_toml_partitions_labels_and_pairs() {
    let root = parse_data(
        r#"
            [graph]
            name = "Сотрудники и проекты"
            type = "bipartite"
            directed = false

            [partitions]
            employees = ["anna", "boris", "vera"]
            projects = ["shop", "analytics"]

            [labels]
            anna = "Анна"
            boris = "Борис"
            vera = "Вера"
            shop = "Магазин"
            analytics = "Аналитика"

            [relations]
            pairs = [
                ["anna", "shop"],
                ["boris", "shop"],
                ["boris", "analytics"],
                ["vera", "analytics"],
            ]
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);

    assert!(!graph.directed);
    assert_eq!(
        graph.partition_names,
        Some(vec!["employees".to_string(), "projects".to_string()])
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.label.as_str(), node.partition))
            .collect::<Vec<_>>(),
        [
            ("anna", "Анна", Some(0)),
            ("boris", "Борис", Some(0)),
            ("vera", "Вера", Some(0)),
            ("shop", "Магазин", Some(1)),
            ("analytics", "Аналитика", Some(1)),
        ]
    );
    assert_eq!(graph.edges.len(), 4);
    assert!(
        graph
            .edges
            .iter()
            .all(|edge| graph.nodes[edge.source].partition != graph.nodes[edge.target].partition)
    );
}

#[test]
fn graph_reads_toml_multipartite_partitions_and_cross_partition_pairs() {
    let root = parse_data(
        r#"
            [graph]
            name = "People, projects, and regions"
            type = "multipartite"
            directed = false

            [partitions]
            people = ["anna", "boris"]
            projects = ["shop"]
            regions = ["north", "south"]

            [labels]
            anna = "Анна"
            boris = "Борис"
            shop = "Магазин"
            north = "Север"
            south = "Юг"

            [relations]
            pairs = [
                ["anna", "shop", "работает над"],
                ["shop", "north", "расположен в"],
                ["boris", "south", "живёт в"],
                ["anna", "boris", "в той же доле"],
            ]
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);
    let mut relationships = graph
        .edges
        .iter()
        .map(|edge| {
            (
                graph.nodes[edge.source].id.clone(),
                graph.nodes[edge.target].id.clone(),
                edge.label.clone(),
            )
        })
        .collect::<Vec<_>>();
    relationships.sort();

    assert!(!graph.directed);
    assert_eq!(
        graph.partition_names,
        Some(vec![
            "people".to_string(),
            "projects".to_string(),
            "regions".to_string()
        ])
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.partition))
            .collect::<Vec<_>>(),
        [
            ("anna", Some(0)),
            ("boris", Some(0)),
            ("shop", Some(1)),
            ("north", Some(2)),
            ("south", Some(2)),
        ]
    );
    assert_eq!(
        relationships,
        [
            (
                "anna".to_string(),
                "shop".to_string(),
                "работает над".to_string()
            ),
            (
                "boris".to_string(),
                "south".to_string(),
                "живёт в".to_string()
            ),
            (
                "shop".to_string(),
                "north".to_string(),
                "расположен в".to_string()
            ),
        ]
    );
}

#[test]
fn graph_reads_toml_directed_multigraph_and_keeps_parallel_edges() {
    let root = parse_data(
        r#"
            [graph]
            name = "Связи между организациями"
            type = "directed_multigraph"

            [nodes]
            a = "Альфа"
            b = "Бета"
            c = "Гамма"

            [[edges]]
            id = "e1"
            source = "a"
            target = "b"
            relation = "поставляет товары"
            contract = "SUP-001"

            [[edges]]
            id = "e2"
            source = "a"
            target = "b"
            relation = "оказывает поддержку"
            contract = "SUPPORT-002"

            [[edges]]
            id = "e3"
            source = "b"
            target = "c"
            relation = "перевозит грузы"
            contract = "LOG-003"
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);
    let parallel_edges = graph
        .edges
        .iter()
        .filter(|edge| graph.nodes[edge.source].id == "a" && graph.nodes[edge.target].id == "b")
        .collect::<Vec<_>>();

    assert!(graph.directed);
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(parallel_edges.len(), 2);
    assert!(parallel_edges[0].label.contains("SUP-001"));
    assert!(parallel_edges[1].label.contains("SUPPORT-002"));
}

#[test]
fn graph_reads_toml_undirected_multigraph_and_keeps_parallel_edges() {
    let root = parse_data(
        r#"
            [graph]
            type = "undirected_multigraph"

            [nodes]
            first = "First"
            second = "Second"

            [[edges]]
            source = "second"
            target = "first"
            relation = "first relation"

            [[edges]]
            source = "first"
            target = "second"
            relation = "second relation"
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);

    assert!(!graph.directed);
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.edges.iter().all(|edge| edge.source < edge.target));
    assert_eq!(
        graph
            .edges
            .iter()
            .map(|edge| edge.label.as_str())
            .collect::<Vec<_>>(),
        ["first relation", "second relation"]
    );
}

#[test]
fn graph_metadata_counts_are_not_inferred_as_relationships() {
    let root = parse_data(
        r#"
            [graph]
            name = "Store"
            directed = true
            entity_count = 100
            relation_count = 104
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;

    assert!(build_relationship_graph(&root).nodes.is_empty());
}

#[test]
fn graph_resolves_json_schema_references_and_skips_ambiguous_ids() {
    let root = parse_data(
        r##"{"$defs":{"User":{"type":"object"},"Pet":{"$ref":"#/$defs/User"}}}"##,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].label, "$ref");

    let duplicate_ids = parse_data(
        r#"{"items":[{"id":"same"},{"id":"same"},{"id":"consumer","ref":"same"}]}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    assert!(build_relationship_graph(&duplicate_ids).edges.is_empty());
}

#[test]
fn schema_view_reads_json_schema_requirements_and_constraints() {
    let root = parse_data(
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["name"],"properties":{"name":{"type":"string","minLength":1},"age":{"type":"integer"}}}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
    let diagram = build_schema_diagram(&root).unwrap();

    assert_eq!(diagram.source, SchemaSource::JsonSchema);
    let name = diagram
        .rows
        .iter()
        .find(|row| row.path == "$.name")
        .unwrap();
    assert_eq!(name.type_name, "string");
    assert_eq!(name.required, Some(true));
    assert!(name.constraints.contains("minLength=1"));
    let age = diagram.rows.iter().find(|row| row.path == "$.age").unwrap();
    assert_eq!(age.required, Some(false));
}

#[test]
fn schema_view_recognizes_scalar_json_schemas_and_toml_date_types() {
    let scalar_schema = parse_data(r#"{"type":"string"}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let schema = build_schema_diagram(&scalar_schema).unwrap();
    assert_eq!(schema.source, SchemaSource::JsonSchema);
    assert_eq!(schema.rows[0].type_name, "string");

    let toml_document = parse_data("created = 1979-05-27T07:32:00Z", Some(DataFormat::Toml))
        .unwrap()
        .0;
    let inferred = build_schema_diagram(&toml_document).unwrap();
    assert_eq!(inferred.source, SchemaSource::Inferred);
    let created = inferred
        .rows
        .iter()
        .find(|row| row.path == "$.created")
        .unwrap();
    assert_eq!(created.type_name, "date-time");
}

#[test]
fn schema_view_recognizes_openapi_components_and_infers_sample_presence() {
    let openapi = parse_data(
            r##"{"openapi":"3.0.0","components":{"schemas":{"Pet":{"type":"object","properties":{"owner":{"$ref":"#/components/schemas/User"}}},"User":{"type":"object"}}},"paths":{"/pets":{"post":{"requestBody":{"content":{"application/json":{"schema":{"type":"object","required":["name"],"properties":{"name":{"type":"string"}}}}}}}}}}"##,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
    let diagram = build_schema_diagram(&openapi).unwrap();
    assert_eq!(diagram.source, SchemaSource::OpenApi);
    assert!(diagram.rows.iter().any(|row| {
        row.path == "$.components.schemas.Pet.owner"
            && row.reference.as_deref() == Some("#/components/schemas/User")
    }));
    assert!(diagram.rows.iter().any(|row| {
        row.path.contains("requestBody")
            && row.path.ends_with(".name")
            && row.required == Some(true)
    }));

    let samples = parse_data(
        r#"{"users":[{"id":1,"name":"Ada"},{"id":2}]}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let inferred = build_schema_diagram(&samples).unwrap();
    assert_eq!(inferred.source, SchemaSource::Inferred);
    let name = inferred
        .rows
        .iter()
        .find(|row| row.path == "$.users[*].name")
        .unwrap();
    assert_eq!(name.required, Some(false));
}

#[test]
fn table_paths_keep_the_root_prefix_for_comments_and_yaml_tags() {
    let root = parse_data("# note\nsecret: !custom hello", Some(DataFormat::Yaml))
        .unwrap()
        .0;
    let table = build_table(&root);
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| row.path.as_str())
            .collect::<Vec<_>>(),
        ["$", "$::comment[0]", "$.secret", "$.secret::metadata-value"]
    );
}

#[test]
fn graph_uses_valid_identity_and_label_when_another_alias_is_null() {
    let root = parse_data(
        r#"{"$id":null,"id":"actual","name":null,"title":"Visible title"}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].id, "actual");
    assert_eq!(graph.nodes[0].label, "Visible title");
}

#[test]
fn graph_resolves_pointers_to_ordinary_entities() {
    let root = parse_data(
        r##"{"objects":[{"id":"source","$ref":"#/objects/1"},{"id":"target"}]}"##,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let graph = build_relationship_graph(&root);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].source, 0);
    assert_eq!(graph.edges[0].target, 1);
}

#[test]
fn swagger_definitions_and_inline_constraint_only_schemas_are_visible() {
    let root = parse_data(
        r#"{"swagger":"2.0","definitions":{"Pet":{"type":"object","properties":{"name":{"type":"string"}}}},"paths":{"/pets":{"get":{"responses":{"200":{"schema":{"minimum":0},"example":{"schema":{"type":"string"}}}}}}}}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let diagram = build_schema_diagram(&root).unwrap();
    assert_eq!(diagram.source, SchemaSource::OpenApi);
    assert!(
        diagram
            .rows
            .iter()
            .any(|row| row.path == "$.definitions.Pet.name")
    );
    assert!(
        diagram
            .rows
            .iter()
            .any(|row| row.constraints == "minimum=0")
    );
    assert!(!diagram.rows.iter().any(|row| row.path.contains(".example")));
}

#[test]
fn an_ordinary_openapi_named_field_is_not_an_api_schema() {
    let root = parse_data(r#"{"openapi":"notes","count":1}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    assert_eq!(
        build_schema_diagram(&root).unwrap().source,
        SchemaSource::Inferred
    );
}

#[test]
fn empty_or_boolean_properties_are_not_mistaken_for_json_schema() {
    for source in [
        r#"{"properties":{}}"#,
        r#"{"properties":{"name":{}}}"#,
        r#"{"properties":{"enabled":true}}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        assert_eq!(
            build_schema_diagram(&root).unwrap().source,
            SchemaSource::Inferred,
            "{source}"
        );
    }

    let schema = parse_data(
        r#"{"properties":{"name":{"type":"string"}}}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    assert_eq!(
        build_schema_diagram(&schema).unwrap().source,
        SchemaSource::JsonSchema
    );
}

#[test]
fn schema_key_search_matches_field_names_not_entire_paths() {
    let root = parse_data(
        r#"{"type":"object","properties":{"name":{"type":"string"},"profile":{"type":"object","properties":{"name":{"type":"string"}}}}}"#,
        Some(DataFormat::Json),
    )
    .unwrap()
    .0;
    let diagram = build_schema_diagram(&root).unwrap();
    let mut search = SearchState::default();
    search.search_with_options(
        &root,
        "name",
        struct_view_core::search::SearchOptions {
            search_keys: true,
            search_values: false,
            search_paths: false,
            exact_match: true,
            ..Default::default()
        },
    );
    let indices = schema_visible_indices(&diagram, &search).unwrap();
    assert_eq!(
        indices
            .iter()
            .map(|index| diagram.rows[*index].path.as_str())
            .collect::<Vec<_>>(),
        ["$.name", "$.profile.name"]
    );
}

#[test]
fn schema_key_value_search_matches_the_same_field_and_type() {
    let root = parse_data(r#"{"name":"Alice","role":1}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let diagram = build_schema_diagram(&root).unwrap();
    let mut search = SearchState::default();
    search.search(&root, "name: string");

    let indices = schema_visible_indices(&diagram, &search).unwrap();
    assert_eq!(
        indices
            .iter()
            .map(|index| diagram.rows[*index].path.as_str())
            .collect::<Vec<_>>(),
        ["$.name"]
    );
}

#[test]
fn inferred_schema_keeps_field_names_through_yaml_tags() {
    let root = parse_data(
        "secret: !custom value\nprofile: !custom {name: Ada}",
        Some(DataFormat::Yaml),
    )
    .unwrap()
    .0;
    let diagram = build_schema_diagram(&root).unwrap();
    let row = diagram
        .rows
        .iter()
        .find(|row| row.path == "$.secret")
        .unwrap();
    assert_eq!(row.key.as_deref(), Some("secret"));
    assert_eq!(row.required, Some(true));
    let mut search = SearchState::default();
    search.search_with_options(
        &root,
        "secret",
        SearchOptions {
            search_keys: true,
            search_values: false,
            search_paths: false,
            exact_match: true,
            ..Default::default()
        },
    );
    let indices = schema_visible_indices(&diagram, &search).unwrap();
    assert_eq!(indices.len(), 1);
    assert_eq!(diagram.rows[indices[0]].path, "$.secret");
    let profile = diagram
        .rows
        .iter()
        .find(|row| row.path == "$.profile")
        .unwrap();
    assert_eq!(profile.key.as_deref(), Some("profile"));
    let name = diagram
        .rows
        .iter()
        .find(|row| row.path == "$.profile.name")
        .unwrap();
    assert_eq!(name.key.as_deref(), Some("name"));
}
