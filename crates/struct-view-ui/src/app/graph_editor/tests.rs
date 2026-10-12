use super::*;
use serde_json::json;
use struct_view_core::graph::EdgeDirection;
use struct_view_core::parser::serialize_node;

fn document() -> GraphDocument {
    let mut document = GraphDocument::empty();
    document.nodes = vec![
        json!({"id":"a","label":"A","attributes":{"count":2}}),
        json!({"id":"b","label":"B","attributes":{}}),
    ];
    document.edges = vec![
        json!({"id":"e1","source":"a","target":"b","label":"first","direction":"directed","weight":-1.25}),
        json!({"id":"e2","source":"a","target":"b","label":"second","direction":"bidirectional"}),
        json!({"id":"loop","source":"a","target":"a","direction":"reverse"}),
    ];
    document
}

#[test]
fn graph_editor_round_trip_preserves_typed_attributes_and_routes_all_edge_kinds() {
    let mut document = document();
    document.partitions = vec!["left".into(), "right".into()];
    document.nodes[0]["partition"] = json!("left");
    document.nodes[1]["partition"] = json!("right");
    let root = document.to_root().unwrap();
    let loaded = GraphDocument::read(&root).unwrap().unwrap();
    assert_eq!(loaded.nodes, document.nodes);
    assert_eq!(loaded.edges, document.edges);
    let graph = try_build_relationship_graph(&root).unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(graph.nodes[0].partition, Some(0));
    assert_eq!(graph.nodes[1].partition, Some(1));
    assert_eq!(graph.edges[0].direction, EdgeDirection::Directed);
    assert_eq!(graph.edges[1].direction, EdgeDirection::Bidirectional);
    assert_eq!(graph.edges[2].source, graph.edges[2].target);
    assert_eq!(graph.edges[2].direction, EdgeDirection::Reverse);
    let svg = crate::app::views::graph_image_for_headless(&root, false, false).unwrap();
    let svg = String::from_utf8(svg).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("first"));
    assert!(svg.contains("second"));
}

#[test]
fn graph_editor_rename_updates_every_endpoint_and_delete_cascades() {
    let mut document = document();
    let mut renamed = document.nodes[0].clone();
    renamed["id"] = json!("renamed");
    document.put_node(Some(0), renamed).unwrap();
    assert!(
        document
            .edges
            .iter()
            .all(|edge| edge["source"] == "renamed")
    );
    assert_eq!(document.edges[2]["target"], "renamed");
    document.delete_node(0).unwrap();
    assert_eq!(document.nodes.len(), 1);
    assert!(document.edges.is_empty());
    document.validate().unwrap();
}

#[test]
fn graph_editor_rejects_invalid_changes_atomically() {
    let mut document = document();
    let before = document.to_root().unwrap();
    assert!(document.put_node(None, json!({"id":"a"})).is_err());
    assert!(
        document
            .put_edge(
                None,
                json!({
                    "id":"invalid","source":"missing","target":"b","direction":"directed"
                })
            )
            .is_err()
    );
    assert!(
        document
            .put_edge(
                None,
                json!({
                    "id":"invalid","source":"a","target":"b","direction":"unknown"
                })
            )
            .is_err()
    );
    assert!(
        document
            .put_edge(
                None,
                json!({
                    "id":"invalid","source":"a","target":"b","direction":"directed","weight":"NaN"
                })
            )
            .is_err()
    );
    assert_eq!(
        node_to_value(&before).unwrap(),
        node_to_value(&document.to_root().unwrap()).unwrap()
    );
}

#[test]
fn graph_editor_conversion_keeps_source_and_duplicate_identity_topology() {
    let (root, _) = parse_data(
        r#"{
        "items":[{"id":"same","name":"One","value":42},{"id":"same","name":"Two"}]
    }"#,
        Some(DataFormat::Json),
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    let copy = GraphDocument::from_graph(&root, &graph).unwrap();
    assert_eq!(copy.nodes.len(), 2);
    assert_ne!(copy.nodes[0]["id"], copy.nodes[1]["id"]);
    assert_eq!(
        copy.metadata["source_document"],
        node_to_value(&root).unwrap()
    );
    assert_eq!(copy.nodes[0]["attributes"]["value"], 42);
}

#[test]
fn graph_editor_changes_share_document_undo_redo_and_read_only_guards() {
    let mut app = StructViewApp::default();
    app.open_graph_document(document().to_root().unwrap());
    let original = node_to_value(app.root.as_ref().unwrap()).unwrap();
    let mut changed = GraphDocument::read(app.root.as_ref().unwrap())
        .unwrap()
        .unwrap();
    changed.delete_node(0).unwrap();
    app.apply_graph_document(changed.clone()).unwrap();
    assert!(app.can_undo());
    app.undo();
    assert_eq!(node_to_value(app.root.as_ref().unwrap()).unwrap(), original);
    app.redo();
    assert_eq!(
        GraphDocument::read(app.root.as_ref().unwrap())
            .unwrap()
            .unwrap()
            .nodes
            .len(),
        1
    );
    app.mode = AppMode::View;
    assert!(app.apply_graph_document(changed).is_err());
}

#[test]
fn graph_editor_saves_and_reopens_in_serializable_formats() {
    let root = document().to_root().unwrap();
    for format in [
        DataFormat::Json,
        DataFormat::Json5,
        DataFormat::Yaml,
        DataFormat::Toml,
    ] {
        let serialized = serialize_node(&root, format, true).unwrap();
        let (reopened, _) = parse_data(&serialized, Some(format)).unwrap();
        let loaded = GraphDocument::read(&reopened).unwrap().unwrap();
        assert_eq!(loaded.nodes, document().nodes);
        assert_eq!(loaded.edges, document().edges);
    }
}

#[test]
fn graph_editor_new_and_copy_respect_unsaved_document_confirmation() {
    let mut app = StructViewApp::default();
    app.request_new_graph();
    assert_eq!(app.visualization, VisualizationMode::Graph);
    assert_eq!(app.mode, AppMode::Edit);
    assert!(app.file_state.path.is_none());
    app.request_new_graph();
    assert!(app.close_file_confirmation_open);
    app.cancel_close_file_confirmation();
    app.request_close_file();
    assert!(app.close_file_confirmation_open);
}

#[test]
fn graph_editor_metadata_errors_are_not_hidden_by_other_adapters() {
    let mut document = document();
    document.partitions = vec!["same".into(), "same".into()];
    assert!(document.to_root().is_err());
    let (root, _) = parse_data(
        r#"{
        "graph":{"editor_version":99,"type":"mixed_multigraph"},
        "nodes":[],"edges":[],"partitions":[]
    }"#,
        Some(DataFormat::Json),
    )
    .unwrap();
    assert!(
        try_build_relationship_graph(&root)
            .unwrap_err()
            .contains("editor_version")
    );
}

#[test]
fn graph_editor_copy_preserves_imported_weights_and_ungrouped_layout() {
    let (root, _) = parse_data(
        r#"{
        "graph":{"type":"mixed_multigraph","weight_unit":"ms"},
        "nodes":[{"id":"a"},{"id":"b"}],
        "edges":[{"id":"original","source":"a","target":"b","label":"time","weight":2.5}]
    }"#,
        Some(DataFormat::Json),
    )
    .unwrap();
    let graph = try_build_relationship_graph(&root).unwrap();
    let mut copy = GraphDocument::from_graph(&root, &graph).unwrap();
    assert_eq!(copy.edges[0]["weight"], 2.5);
    assert_eq!(copy.metadata["weight_unit"], "ms");
    assert!(!copy.edges[0]["label"].as_str().unwrap().contains("2.5"));
    copy.partitions = vec!["first".into()];
    copy.nodes[0]["partition"] = json!("first");
    let copied_graph = try_build_relationship_graph(&copy.to_root().unwrap()).unwrap();
    assert_eq!(copied_graph.nodes[0].partition, Some(0));
    assert_eq!(copied_graph.nodes[1].partition, Some(1));
    assert_eq!(copied_graph.partition_names.unwrap(), vec!["first", ""]);
    assert_eq!(copied_graph.edges.len(), 1);
    assert_eq!(copied_graph.edges[0].label.matches("2.5 ms").count(), 1);
}
