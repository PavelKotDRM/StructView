use super::*;
use std::collections::BTreeSet;

use crate::clipboard::ClipboardEntry;
use serde_json::Value;
use struct_view_core::parser::{
    DataFormat, JsonNode, JsonValueType, node_to_value, parse_data, parse_json,
};

fn leaf(value_type: JsonValueType, display_value: &str) -> JsonNode {
    JsonNode {
        key: None,
        value_type,
        display_value: display_value.to_string(),
        children: vec![],
        expanded: false,
        path: "root".to_string(),
    }
}

#[test]
fn roundtrip_preserves_structure() {
    let source = r#"{"a":[1,2],"b":"x","c":null,"d":true}"#;
    let node = parse_json(source).unwrap();
    let value = node_to_value(&node).unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), source);
}

#[test]
fn roundtrip_escapes_control_characters_in_strings() {
    let source = r#"{"text":"line\n\u0000"}"#;
    let node = parse_json(source).unwrap();
    let value = node_to_value(&node).unwrap();

    assert_eq!(serde_json::to_string(&value).unwrap(), source);
}

#[test]
fn edit_detects_literal_type() {
    let mut node = leaf(JsonValueType::Null, "null");

    apply_primitive_edit(&mut node, " 42 ").unwrap();
    assert_eq!(node.value_type, JsonValueType::Number);
    assert_eq!(node.display_value, "42");

    apply_primitive_edit(&mut node, "\"текст\"").unwrap();
    assert_eq!(node.value_type, JsonValueType::String);

    apply_primitive_edit(&mut node, "false").unwrap();
    assert_eq!(node.value_type, JsonValueType::Bool);
}

#[test]
fn edit_rejects_invalid_literal() {
    let mut node = leaf(JsonValueType::String, "\"x\"");
    assert!(apply_primitive_edit(&mut node, "").is_err());
    assert!(apply_primitive_edit(&mut node, "нет кавычек").is_err());
    assert_eq!(node.display_value, "\"x\"");
}

#[test]
fn typed_constructor_adds_values_without_format_literals() {
    for format in [
        DataFormat::Json,
        DataFormat::Yaml,
        DataFormat::Toml,
        DataFormat::Json5,
    ] {
        let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
        add_typed_child_at_path(&mut root, "", "name", &JsonValueType::String, "Ada", format)
            .unwrap();
        add_typed_child_at_path(&mut root, "", "age", &JsonValueType::Number, "37", format)
            .unwrap();
        add_typed_child_at_path(
            &mut root,
            "",
            "enabled",
            &JsonValueType::Bool,
            "false",
            format,
        )
        .unwrap();
        add_typed_child_at_path(&mut root, "", "profile", &JsonValueType::Object, "", format)
            .unwrap();
        add_typed_child_at_path(&mut root, "", "roles", &JsonValueType::Array, "", format).unwrap();

        let value = node_to_value(&root).unwrap();
        assert_eq!(value["name"], "Ada");
        assert_eq!(value["age"], 37);
        assert_eq!(value["enabled"], false);
        assert_eq!(value["profile"], serde_json::json!({}));
        assert_eq!(value["roles"], serde_json::json!([]));
    }

    let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
    add_typed_child_at_path(
        &mut root,
        "",
        "missing",
        &JsonValueType::Null,
        "",
        DataFormat::Json,
    )
    .unwrap();
    assert_eq!(node_to_value(&root).unwrap()["missing"], Value::Null);
}

#[test]
fn typed_constructor_adds_comments_and_yaml_metadata() {
    let mut json5_root = parse_data("{}", Some(DataFormat::Json)).unwrap().0;
    add_typed_child_at_path(
        &mut json5_root,
        "",
        "",
        &JsonValueType::Comment,
        "generated note",
        DataFormat::Json5,
    )
    .unwrap();
    assert_eq!(json5_root.children[0].value_type, JsonValueType::Comment);
    assert_eq!(json5_root.children[0].display_value, "// generated note");
    assert_eq!(node_to_value(&json5_root).unwrap(), serde_json::json!({}));
    assert!(
        struct_view_core::parser::serialize_node(&json5_root, DataFormat::Json5, false)
            .unwrap()
            .starts_with("// generated note")
    );

    let mut array_root = parse_data("[]", Some(DataFormat::Json5)).unwrap().0;
    add_typed_child_at_path(
        &mut array_root,
        "",
        "",
        &JsonValueType::Comment,
        "array note",
        DataFormat::Json5,
    )
    .unwrap();
    add_typed_child_at_path(
        &mut array_root,
        "",
        "",
        &JsonValueType::Number,
        "2",
        DataFormat::Json5,
    )
    .unwrap();
    assert_eq!(array_root.children[0].path, "::comment[0]");
    assert_eq!(array_root.children[1].path, "0");
    assert_eq!(node_to_value(&array_root).unwrap(), serde_json::json!([2]));

    for format in [DataFormat::Yaml, DataFormat::Toml] {
        let mut root = parse_data("{}", Some(DataFormat::Json)).unwrap().0;
        add_typed_child_at_path(
            &mut root,
            "",
            "",
            &JsonValueType::Comment,
            "generated note",
            format,
        )
        .unwrap();
        let output = struct_view_core::parser::serialize_node(&root, format, false).unwrap();
        assert!(output.starts_with("# generated note\n"));
        let reparsed = parse_data(&output, Some(format)).unwrap().0;
        assert_eq!(node_to_value(&reparsed).unwrap(), serde_json::json!({}));
    }

    let mut yaml_root = parse_data("{}", Some(DataFormat::Yaml)).unwrap().0;
    add_typed_child_at_path(
        &mut yaml_root,
        "",
        "secret",
        &JsonValueType::Metadata,
        "!custom value",
        DataFormat::Yaml,
    )
    .unwrap();
    assert_eq!(yaml_root.children[0].value_type, JsonValueType::Metadata);
    assert_eq!(node_to_value(&yaml_root).unwrap()["secret"], "value");
    edit_child_at_path(
        &mut yaml_root,
        "secret::metadata-value",
        None,
        &JsonValueType::String,
        "updated",
        DataFormat::Yaml,
    )
    .unwrap();
    assert_eq!(node_to_value(&yaml_root).unwrap()["secret"], "updated");
    assert!(
        struct_view_core::parser::serialize_node(&yaml_root, DataFormat::Yaml, false)
            .unwrap()
            .contains("!custom")
    );
    assert!(
        add_typed_child_at_path(
            &mut yaml_root,
            "",
            "invalid",
            &JsonValueType::Metadata,
            "plain value",
            DataFormat::Yaml,
        )
        .is_err()
    );
}

#[test]
fn toml_datetime_fields_can_be_edited_without_becoming_strings() {
    let (mut root, _) =
        parse_data("created = 1979-05-27T07:32:00Z", Some(DataFormat::Toml)).unwrap();

    edit_child_at_path(
        &mut root,
        "created",
        None,
        &JsonValueType::DateTime,
        "1980-01-02T03:04:05Z",
        DataFormat::Toml,
    )
    .unwrap();

    assert_eq!(root.children[0].value_type, JsonValueType::DateTime);
    assert_eq!(
        node_to_value(&root).unwrap()["created"],
        "1980-01-02T03:04:05Z"
    );
    let serialized =
        struct_view_core::parser::serialize_node(&root, DataFormat::Toml, false).unwrap();
    assert!(serialized.contains("created = 1980-01-02T03:04:05Z"));
}

#[test]
fn typed_toml_float_constructor_preserves_float_type() {
    let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
    add_typed_child_at_path(
        &mut root,
        "",
        "ratio",
        &JsonValueType::Float,
        "1",
        DataFormat::Toml,
    )
    .unwrap();

    assert_eq!(root.children[0].value_type, JsonValueType::Float);
    assert_eq!(node_to_value(&root).unwrap()["ratio"], 1.0);
    let serialized =
        struct_view_core::parser::serialize_node(&root, DataFormat::Toml, false).unwrap();
    assert!(serialized.contains("ratio = 1.0"));
}

#[test]
fn typed_constructor_edits_type_renames_field_and_preserves_containers() {
    let (mut root, _) = parse_data(
        r#"{"profile":{"name":"Ada"},"value":1,"items":[1]}"#,
        Some(DataFormat::Json),
    )
    .unwrap();

    edit_child_at_path(
        &mut root,
        "value",
        Some("count"),
        &JsonValueType::Number,
        "2",
        DataFormat::Json,
    )
    .unwrap();
    edit_child_at_path(
        &mut root,
        "profile",
        None,
        &JsonValueType::Object,
        "",
        DataFormat::Json,
    )
    .unwrap();
    edit_child_at_path(
        &mut root,
        "items",
        None,
        &JsonValueType::Array,
        "",
        DataFormat::Json,
    )
    .unwrap();

    assert_eq!(
        node_to_value(&root).unwrap(),
        serde_json::json!({
            "profile": {"name": "Ada"},
            "count": 2,
            "items": [1]
        })
    );
    assert!(is_object_child(&root, "count"));
    assert_eq!(find_node(&root, "count").unwrap().path, "count");
}

#[test]
fn typed_constructor_rejects_toml_null_values() {
    let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
    let error = add_typed_child_at_path(
        &mut root,
        "",
        "missing",
        &JsonValueType::Null,
        "",
        DataFormat::Toml,
    )
    .unwrap_err();
    assert!(!error.is_empty());
    assert!(root.children.is_empty());
}

#[test]
fn adds_object_fields_and_array_elements_for_all_formats() {
    let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
    add_child(&mut root, "enabled", "true", DataFormat::Json).unwrap();
    add_child(&mut root, "profile", "name: Ada", DataFormat::Yaml).unwrap();
    add_child(&mut root, "server", "{ port = 8080 }", DataFormat::Toml).unwrap();
    add_child(&mut root, "items", "[1, 2,]", DataFormat::Json5).unwrap();

    assert_eq!(root.children.len(), 4);
    assert_eq!(root.children[0].path, "enabled");
    assert_eq!(root.children[1].children[0].key.as_deref(), Some("name"));
    assert_eq!(root.children[2].children[0].key.as_deref(), Some("port"));
    assert_eq!(root.children[3].children.len(), 2);

    let mut array = parse_data("[]", Some(DataFormat::Json)).unwrap().0;
    add_child(&mut array, "", "\"first\"", DataFormat::Json).unwrap();
    assert_eq!(array.children[0].key.as_deref(), Some("0"));
    assert_eq!(array.children[0].path, "0");
}

#[test]
fn rejects_duplicate_object_field() {
    let (mut root, _) = parse_data(r#"{"name":"Ada"}"#, Some(DataFormat::Json)).unwrap();
    let error = add_child(&mut root, "name", "\"Grace\"", DataFormat::Json).unwrap_err();
    assert!(error.contains("уже существует"));
}

#[test]
fn selected_structures_keep_hierarchy_and_skip_nested_duplicates() {
    let root =
        parse_json(r#"{"profile":{"name":"Ada","roles":["admin"]},"enabled":true}"#).unwrap();
    let selected_paths = BTreeSet::from([
        "profile".to_string(),
        "profile.name".to_string(),
        "enabled".to_string(),
    ]);

    let entries = selected_structures(&root, &selected_paths).unwrap();

    assert_eq!(entries.len(), 2);
    let profile = entries
        .iter()
        .find(|entry| entry.key.as_deref() == Some("profile"))
        .unwrap();
    assert_eq!(
        profile.value,
        serde_json::json!({"name": "Ada", "roles": ["admin"]})
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.key.as_deref() == Some("enabled"))
    );
}

#[test]
fn deleting_selected_structures_skips_nested_duplicates() {
    let mut root = parse_json(r#"{"profile":{"name":"Ada"},"enabled":true}"#).unwrap();
    let selected_paths = BTreeSet::from([
        "profile".to_string(),
        "profile.name".to_string(),
        "enabled".to_string(),
    ]);

    assert_eq!(
        delete_selected_structures(&mut root, &selected_paths).unwrap(),
        2
    );
    assert_eq!(node_to_value(&root).unwrap(), serde_json::json!({}));
    assert!(root.children.is_empty());
}

#[test]
fn deleting_array_items_reindexes_remaining_nodes_and_comments() {
    let mut root = parse_data("[]", Some(DataFormat::Json5)).unwrap().0;
    add_typed_child_at_path(
        &mut root,
        "",
        "",
        &JsonValueType::Comment,
        "first note",
        DataFormat::Json5,
    )
    .unwrap();
    add_typed_child_at_path(
        &mut root,
        "",
        "",
        &JsonValueType::Comment,
        "second note",
        DataFormat::Json5,
    )
    .unwrap();
    add_typed_child_at_path(
        &mut root,
        "",
        "",
        &JsonValueType::Number,
        "1",
        DataFormat::Json5,
    )
    .unwrap();
    add_typed_child_at_path(
        &mut root,
        "",
        "",
        &JsonValueType::Number,
        "2",
        DataFormat::Json5,
    )
    .unwrap();
    let selected_paths = BTreeSet::from(["::comment[0]".to_string(), "0".to_string()]);

    assert_eq!(
        delete_selected_structures(&mut root, &selected_paths).unwrap(),
        2
    );
    assert_eq!(node_to_value(&root).unwrap(), serde_json::json!([2]));
    assert_eq!(root.children[0].path, "::comment[0]");
    assert_eq!(root.children[0].display_value, "// second note");
    assert_eq!(root.children[1].key.as_deref(), Some("0"));
    assert_eq!(root.children[1].path, "0");
}

#[test]
fn deleting_root_or_missing_selection_is_rejected_without_changes() {
    let mut root = parse_json(r#"{"value":1}"#).unwrap();
    let original = node_to_value(&root).unwrap();

    assert_eq!(
        delete_selected_structures(&mut root, &BTreeSet::from(["".to_string()])),
        Err(DeleteError::RootSelected)
    );
    assert_eq!(
        delete_selected_structures(&mut root, &BTreeSet::from(["missing".to_string()])),
        Err(DeleteError::SelectionNotFound)
    );
    assert_eq!(node_to_value(&root).unwrap(), original);
}

#[test]
fn pasting_object_structures_preserves_nested_values_and_paths() {
    let source = parse_json(r#"{"profile":{"name":"Ada","roles":["admin"]}}"#).unwrap();
    let selected_paths = BTreeSet::from(["profile".to_string()]);
    let entries = selected_structures(&source, &selected_paths).unwrap();
    let mut target = parse_json(r#"{"existing":true}"#).unwrap();

    let inserted = paste_structures_at_path(&mut target, "", &entries).unwrap();

    assert_eq!(inserted, 1);
    assert_eq!(
        node_to_value(&target).unwrap(),
        serde_json::json!({
            "existing": true,
            "profile": {"name": "Ada", "roles": ["admin"]}
        })
    );
    assert_eq!(target.children[1].path, "profile");
    assert_eq!(target.children[1].children[0].path, "profile.name");
}

#[test]
fn pasting_root_array_into_array_appends_its_elements() {
    let source = parse_json(r#"[{"id":1},{"id":2}]"#).unwrap();
    let selected_paths = BTreeSet::from(["".to_string()]);
    let entries = selected_structures(&source, &selected_paths).unwrap();
    let mut target = parse_json(r#"[{"id":0}]"#).unwrap();

    let inserted = paste_structures_at_path(&mut target, "", &entries).unwrap();

    assert_eq!(inserted, 2);
    assert_eq!(
        node_to_value(&target).unwrap(),
        serde_json::json!([{"id": 0}, {"id": 1}, {"id": 2}])
    );
    assert_eq!(target.children[2].path, "2");
}

#[test]
fn duplicate_paste_does_not_partially_modify_object() {
    let entry = ClipboardEntry {
        key: Some("name".to_string()),
        value: Value::String("Grace".to_string()),
    };
    let mut target = parse_json(r#"{"name":"Ada"}"#).unwrap();

    let error = paste_structures_at_path(&mut target, "", &[entry]).unwrap_err();

    assert!(error.contains("уже существует"));
    assert_eq!(
        node_to_value(&target).unwrap(),
        serde_json::json!({"name": "Ada"})
    );
}
