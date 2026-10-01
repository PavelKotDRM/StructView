use super::*;

use struct_view_core::parser::{DataFormat, parse_data};
use struct_view_core::search::SearchState;

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
