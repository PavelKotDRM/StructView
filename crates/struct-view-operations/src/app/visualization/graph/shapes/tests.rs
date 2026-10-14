use super::*;
use struct_view_core::parser::{DataFormat, parse_data, serialize_node};

fn graph(source: &str) -> RelationshipGraph {
    let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
    super::super::try_build_relationship_graph(&root).unwrap()
}

#[test]
fn adapters_work_in_every_writable_format() {
    let cases = [
        (
            r#"{"directed":false,"multigraph":true,"graph":{},"nodes":[{"id":1,"label":"First"},{"id":2}],"links":[{"source":1,"target":2,"key":0,"weight":3},{"source":{"id":1},"target":{"id":2},"key":1,"direction":"bidirectional"},{"source":2,"target":2,"key":2,"direction":"reverse"}]}"#,
            "nodes[0]",
        ),
        (
            r#"{"graph":{"type":"mixed_multigraph","nodes":{"a":{"label":"First"},"b":{"name":"Second"}},"links":{"e1":{"from":"a","to":"b","weight":3},"e2":{"source":"a","target":"b","direction":"bidirectional"},"loop":{"source":"b","target":"b","direction":"reverse"}}}}"#,
            "graph.nodes.a",
        ),
        (
            r#"{"elements":{"nodes":[{"data":{"id":"a","label":"First"}},{"data":{"id":"b"}}],"edges":[{"data":{"id":"e1","source":"a","target":"b","weight":3}},{"data":{"id":"e2","source":"a","target":"b","direction":"bidirectional"}},{"data":{"id":"loop","source":"b","target":"b","direction":"reverse"}}]}}"#,
            "elements.nodes[0]",
        ),
        (
            r#"{"elements":[{"data":{"source":"a","target":"b","weight":3}},{"data":{"id":"a","label":"First"}},{"data":{"id":"b"}},{"data":{"source":"a","target":"b","direction":"bidirectional"}},{"data":{"source":"b","target":"b","direction":"reverse"}}]}"#,
            "elements[1]",
        ),
        (
            r#"{"directed":true,"multigraph":true,"nodes":["a","b"],"edges":[["a","b",3],["a","b","two","bidirectional"],["b","b","loop","reverse"]]}"#,
            "nodes[0]",
        ),
        (
            r#"{"directed":true,"multigraph":true,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","weight":3},{"id":"b","direction":"bidirectional"}],[{"id":"b","direction":"reverse"}]]}"#,
            "nodes[0]",
        ),
        (
            r#"{"graph":{"type":"mixed_multigraph"},"nodes":{"a":"First","b":"Second"},"adjacency":{"a":[{"target":"b","weight":3},{"target":"b","direction":"bidirectional"}],"b":[{"target":"b","direction":"reverse"}]}}"#,
            "nodes.a",
        ),
    ];
    for (source, path) in cases {
        let expected = graph(source);
        assert_eq!(expected.nodes.len(), 2, "{source}");
        assert_eq!(expected.edges.len(), 3, "{source}");
        assert!(
            expected
                .edges
                .iter()
                .any(|edge| edge.source == edge.target && edge.direction == EdgeDirection::Reverse)
        );
        assert!(
            expected
                .edges
                .iter()
                .any(|edge| edge.direction == EdgeDirection::Bidirectional)
        );
        assert!(expected.edges.iter().any(|edge| edge.label.contains('3')));
        assert_eq!(expected.nodes[0].path, path);
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        for format in DataFormat::ALL {
            let text = serialize_node(&root, format, false).unwrap();
            let root = parse_data(&text, Some(format)).unwrap().0;
            let actual = super::super::try_build_relationship_graph(&root).unwrap();
            assert_eq!(actual.edges, expected.edges, "{format:?}: {text}");
            assert_eq!(actual.nodes.len(), expected.nodes.len());
            for (actual, expected) in actual.nodes.iter().zip(&expected.nodes) {
                assert_eq!(actual.id, expected.id);
                assert_eq!(actual.label, expected.label);
                assert_eq!(actual.path, expected.path);
                assert!(actual.search_paths.contains(&actual.path));
            }
        }
    }
}

#[test]
fn native_yaml_and_toml_keep_keyed_edges_and_weights() {
    for (format, source) in [
        (
            DataFormat::Yaml,
            "graph:\n  type: mixed_multigraph\n  weight_unit: ms\nnodes:\n  a: Alpha\n  b: Beta\nlinks:\n  ab:\n    source: a\n    target: b\n    weight: 2\n    direction: bidirectional\n",
        ),
        (
            DataFormat::Toml,
            "[graph]\ntype = 'mixed_multigraph'\nweight_unit = 'ms'\n[nodes]\na = 'Alpha'\nb = 'Beta'\n[links.ab]\nsource = 'a'\ntarget = 'b'\nweight = 2\ndirection = 'bidirectional'\n",
        ),
    ] {
        let root = parse_data(source, Some(format)).unwrap().0;
        let actual = super::super::try_build_relationship_graph(&root).unwrap();
        assert_eq!(actual.nodes[0].label, "Alpha");
        assert_eq!(actual.edges[0].label, "2 ms · ab");
        assert_eq!(actual.edges[0].direction, EdgeDirection::Bidirectional);
    }
}

#[test]
fn tuples_and_weighted_adjacency_keep_isolated_and_incoming_nodes() {
    let actual = graph(
        r#"{"directed":true,"nodes":["a","b","isolated"],"edges":[["a","b"],["b","b",{"weight":2,"direction":"reverse"}]]}"#,
    );
    assert_eq!(actual.nodes.len(), 3);
    assert_eq!(actual.edges[1].label, "2");
    assert_eq!(actual.edges[1].direction, EdgeDirection::Reverse);
    let actual = graph(
        r#"{"graph":{"type":"weighted_undirected"},"adjacency":{"a":{"b":2,"a":3},"isolated":[]}}"#,
    );
    assert_eq!(actual.nodes.len(), 3);
    assert_eq!(actual.edges.len(), 2);
    assert!(actual.nodes.iter().any(|node| node.id == "b"));
}

#[test]
fn networkx_undirected_adjacency_deduplicates_mirrors_not_parallel_keys() {
    let actual = graph(
        r#"{"directed":false,"multigraph":true,"graph":{},"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"weight":2},{"id":"b","key":1,"weight":2},{"id":"a","key":0}],[{"id":"a","key":0,"weight":2},{"id":"a","key":1,"weight":2}]]}"#,
    );
    assert_eq!(actual.edges.len(), 3);
    assert_eq!(actual.edges[0].label, "2 · 0");
    assert_eq!(actual.edges[1].label, "2 · 1");
    let actual = graph(
        r#"{"directed":false,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b"}],[{"id":"a"}]]}"#,
    );
    assert_eq!(actual.edges.len(), 1);
}

#[test]
fn malformed_graphs_report_errors_instead_of_losing_edges() {
    for source in [
        r#"{"directed":true,"nodes":["a","a"],"edges":[]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[["a","missing"]]}"#,
        r#"{"directed":true,"nodes":{"a":{"id":"b"}},"edges":[]}"#,
        r#"{"directed":true,"nodes":{"a":[]},"edges":[]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[["a"]]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[["a","a",1,"invalid"]]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","from":"a","target":"a"}]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[],"links":[]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[],"adjacency":[[]]}"#,
        r#"{"directed":"true","nodes":["a"],"edges":[]}"#,
        r#"{"directed":true,"multigraph":"true","nodes":["a"],"edges":[]}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":[]}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":[[{"id":"missing"}]]}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":{"a":["missing"]}}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":{"a":[{"target":"a","source":"other"}]}}"#,
        r#"{"directed":true,"nodes":["a"]}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":false}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","target":"a","directed":"true"}]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","target":"a","weight":{}}]}"#,
        r#"{"directed":true,"nodes":["a"],"edges":{"e1":{"id":"e2","source":"a","target":"a"}}}"#,
        r#"{"directed":true,"nodes":["a"],"edges":[["a","a",{"direction":"reverse"},"bidirectional"]]}"#,
        r#"{"directed":false,"multigraph":true,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"weight":2}],[{"id":"a","key":0,"weight":3}]]}"#,
        r#"{"graph":{"type":"directed"},"adjacency":{"a":false}}"#,
        r#"{"elements":[{"data":{"id":"a"}},{"data":{"source":"a","target":"missing"}}]}"#,
        r#"{"elements":[{"data":{"id":"a"}},{"data":"invalid"}]}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        assert!(
            super::super::try_build_relationship_graph(&root).is_err(),
            "{source}"
        );
    }
}

#[test]
fn ordinary_documents_are_not_claimed_by_the_adapter() {
    for source in [
        r#"{"nodes":[{"name":"chapter"}],"edges":["margin"],"links":["website"]}"#,
        r#"{"elements":["water","air"]}"#,
        r#"{"elements":{"nodes":[],"edges":[]}}"#,
        r#"{"nodes":[],"edges":[]}"#,
        r#"{"nodes":["chapter"],"links":["https://example.test"]}"#,
    ] {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        assert!(build(&root).unwrap().is_none(), "{source}");
        assert!(
            super::super::try_build_relationship_graph(&root).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn networkx_graph_attributes_are_not_nested_graph_config() {
    let actual = graph(
        r#"{"directed":false,"multigraph":false,"graph":{"type":"domain-specific","nodes":"attribute","direction":"north"},"nodes":[{"id":"a"},{"id":"b"}],"links":[{"source":"a","target":"b"}]}"#,
    );
    assert_eq!(actual.nodes.len(), 2);
    assert_eq!(actual.edges[0].direction, EdgeDirection::Undirected);
}
