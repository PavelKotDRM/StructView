use super::*;

use struct_view_core::parser::{DataFormat, parse_data};
use struct_view_core::search::{SearchOptions, SearchState};

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
            },
            GraphEdge {
                source: 0,
                target: 3,
                label: String::new(),
            },
            GraphEdge {
                source: 1,
                target: 2,
                label: String::new(),
            },
        ]
    );
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
            },
            GraphEdge {
                source: 0,
                target: 2,
                label: "0 км".to_string(),
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
