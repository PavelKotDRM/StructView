use std::collections::BTreeSet;

use super::{
    AppMode, HISTORY_LIMIT, InlineEditEvent, StructViewApp, VisualizationMode, edit_child_at_path,
    field_value_types, paste_structures_at_path, with_format_extension,
};
use crate::clipboard::ClipboardEntry;
use struct_view_core::parser::{DataFormat, JsonValueType, node_to_value, parse_data};

#[test]
fn constructor_offers_comments_and_metadata_only_for_supported_formats() {
    assert!(field_value_types(DataFormat::Json5, false, false).contains(&JsonValueType::Comment));
    assert!(field_value_types(DataFormat::Yaml, false, false).contains(&JsonValueType::Comment));
    assert!(field_value_types(DataFormat::Toml, false, false).contains(&JsonValueType::Comment));
    assert!(!field_value_types(DataFormat::Json, false, false).contains(&JsonValueType::Comment));
    assert!(field_value_types(DataFormat::Yaml, false, false).contains(&JsonValueType::Metadata));
    assert!(!field_value_types(DataFormat::Json5, false, false).contains(&JsonValueType::Metadata));
    assert_eq!(
        field_value_types(DataFormat::Json5, false, true),
        vec![JsonValueType::Comment]
    );
}

#[test]
fn undo_and_redo_restore_document_snapshots_and_new_changes_clear_redo() {
    let root = parse_data(r#"{"value":1}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let mut app = StructViewApp {
        root: Some(root),
        mode: AppMode::Edit,
        ..StructViewApp::default()
    };

    let before_edit = app.root.as_ref().unwrap().clone();
    app.root.as_mut().unwrap().children[0].display_value = "2".to_string();
    app.push_undo_snapshot(before_edit);
    assert!(app.can_undo());

    app.undo();
    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap()["value"],
        1
    );
    assert!(app.can_redo());

    app.redo();
    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap()["value"],
        2
    );

    app.undo();
    let before_new_edit = app.root.as_ref().unwrap().clone();
    app.root.as_mut().unwrap().children[0].display_value = "3".to_string();
    app.push_undo_snapshot(before_new_edit);
    assert!(!app.can_redo());
}

#[test]
fn deleting_selected_structure_is_undoable() {
    let root = parse_data("[1,2,3]", Some(DataFormat::Json)).unwrap().0;
    let mut app = StructViewApp {
        root: Some(root),
        mode: AppMode::Edit,
        selected_paths: BTreeSet::from(["1".to_string()]),
        ..StructViewApp::default()
    };

    assert!(app.can_delete_selected());
    app.delete_selected();

    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap(),
        serde_json::json!([1, 3])
    );
    assert!(app.selected_paths.is_empty());
    assert!(app.can_undo());

    app.undo();
    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap(),
        serde_json::json!([1, 2, 3])
    );
}

#[test]
fn inline_typing_is_grouped_into_one_history_step() {
    let root = parse_data(r#"{"value":1}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let mut app = StructViewApp {
        root: Some(root),
        mode: AppMode::Edit,
        ..StructViewApp::default()
    };

    app.root.as_mut().unwrap().children[0].display_value = "12".to_string();
    app.handle_inline_edit_events(vec![InlineEditEvent {
        path: "value".to_string(),
        before_value_type: JsonValueType::Number,
        before_display_value: "1".to_string(),
        changed: true,
        finished: false,
        valid: true,
    }]);

    app.root.as_mut().unwrap().children[0].display_value = "123".to_string();
    app.handle_inline_edit_events(vec![InlineEditEvent {
        path: "value".to_string(),
        before_value_type: JsonValueType::Number,
        before_display_value: "12".to_string(),
        changed: true,
        finished: false,
        valid: true,
    }]);

    app.handle_inline_edit_events(vec![InlineEditEvent {
        path: "value".to_string(),
        before_value_type: JsonValueType::Number,
        before_display_value: "123".to_string(),
        changed: false,
        finished: true,
        valid: true,
    }]);

    assert_eq!(app.undo_history.len(), 1);
    app.undo();
    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap()["value"],
        1
    );
}

#[test]
fn invalid_inline_edit_is_rolled_back_without_history() {
    let root = parse_data(r#"{"value":1}"#, Some(DataFormat::Json))
        .unwrap()
        .0;
    let mut app = StructViewApp {
        root: Some(root),
        mode: AppMode::Edit,
        ..StructViewApp::default()
    };
    app.root.as_mut().unwrap().children[0].display_value = "invalid".to_string();

    assert!(app.handle_inline_edit_events(vec![InlineEditEvent {
        path: "value".to_string(),
        before_value_type: JsonValueType::Number,
        before_display_value: "1".to_string(),
        changed: true,
        finished: true,
        valid: false,
    }]));

    assert_eq!(
        node_to_value(app.root.as_ref().unwrap()).unwrap()["value"],
        1
    );
    assert!(app.undo_history.is_empty());
}

#[test]
fn history_is_limited_to_one_hundred_snapshots() {
    let root = parse_data("{}", Some(DataFormat::Json)).unwrap().0;
    let mut app = StructViewApp {
        root: Some(root.clone()),
        mode: AppMode::Edit,
        ..StructViewApp::default()
    };

    for _ in 0..HISTORY_LIMIT + 1 {
        app.push_undo_snapshot(root.clone());
    }

    assert_eq!(app.undo_history.len(), HISTORY_LIMIT);
}

#[test]
fn selection_survives_paste_and_edits_when_paths_remain_valid() {
    let (root, _) = parse_data(
        r#"{"target":{"value":1},"other":true}"#,
        Some(DataFormat::Json),
    )
    .unwrap();
    let mut app = StructViewApp {
        root: Some(root),
        selected_paths: BTreeSet::from([
            "target".to_string(),
            "target.value".to_string(),
            "other".to_string(),
        ]),
        ..StructViewApp::default()
    };
    let selected_before_change = app.selected_paths.clone();

    paste_structures_at_path(
        app.root.as_mut().unwrap(),
        "target",
        &[ClipboardEntry {
            key: Some("added".to_string()),
            value: serde_json::json!(2),
        }],
    )
    .unwrap();
    app.retain_valid_selected_paths();
    assert_eq!(app.selected_paths, selected_before_change);

    edit_child_at_path(
        app.root.as_mut().unwrap(),
        "target.value",
        None,
        &JsonValueType::Number,
        "3",
        DataFormat::Json,
    )
    .unwrap();
    app.retain_valid_selected_paths();
    assert_eq!(app.selected_paths, selected_before_change);

    edit_child_at_path(
        app.root.as_mut().unwrap(),
        "target",
        None,
        &JsonValueType::Number,
        "4",
        DataFormat::Json,
    )
    .unwrap();
    app.retain_valid_selected_paths();
    assert_eq!(
        app.selected_paths,
        BTreeSet::from(["other".to_string(), "target".to_string()])
    );
}

#[test]
fn save_current_writes_updated_document_to_loaded_path() {
    let path =
        std::env::temp_dir().join(format!("struct_view-save-test-{}.json", std::process::id()));
    std::fs::write(&path, r#"{"value":1}"#).unwrap();

    let mut app = StructViewApp::default();
    app.load_file(path.clone());
    app.root
        .as_mut()
        .unwrap()
        .children
        .first_mut()
        .unwrap()
        .display_value = "2".to_string();

    app.save_current();

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"value\": 2"));
    assert_eq!(app.file_state.size_bytes, saved.len() as u64);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn open_document_can_be_converted_to_every_other_format() {
    let input = std::env::temp_dir().join(format!(
        "struct_view-convert-test-{}.json",
        std::process::id()
    ));
    std::fs::write(&input, r#"{"server":{"port":8080},"enabled":true}"#).unwrap();

    let mut app = StructViewApp::default();
    app.load_file(input.clone());

    for (index, format) in DataFormat::ALL.into_iter().enumerate() {
        if format == DataFormat::Json {
            continue;
        }

        let requested_path = std::env::temp_dir().join(format!(
            "struct_view-convert-test-{}-{}.output",
            std::process::id(),
            index
        ));
        let output = with_format_extension(requested_path, format);
        let size_bytes = app.write_root_to_path(&output, format).unwrap();
        let content = std::fs::read_to_string(&output).unwrap();
        let (_, parsed_format) = parse_data(&content, Some(format)).unwrap();

        assert_eq!(parsed_format, format);
        assert_eq!(size_bytes, content.len() as u64);
        std::fs::remove_file(output).unwrap();
    }

    assert_eq!(app.file_state.path, Some(input.clone()));
    assert_eq!(app.file_state.format, Some(DataFormat::Json));
    std::fs::remove_file(input).unwrap();
}

#[test]
fn close_file_clears_document_state() {
    let path = std::env::temp_dir().join(format!(
        "struct_view-close-test-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, r#"{"value":1}"#).unwrap();

    let mut app = StructViewApp::default();
    app.load_file(path.clone());
    let snapshot = app.root.as_ref().unwrap().clone();
    app.undo_history.push(snapshot.clone());
    app.redo_history.push(snapshot);
    app.search_query_buf = "value".to_string();
    app.save_requested = true;
    app.close_file();

    assert!(app.root.is_none());
    assert!(app.file_state.path.is_none());
    assert!(app.search_query_buf.is_empty());
    assert!(!app.save_requested);
    assert!(app.undo_history.is_empty());
    assert!(app.redo_history.is_empty());
    assert_eq!(app.mode, super::AppMode::View);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn comparison_loads_all_documents_and_changed_paths() {
    let prefix =
        std::env::temp_dir().join(format!("struct_view-compare-test-{}", std::process::id()));
    let first = prefix.with_extension("first.json");
    let second = prefix.with_extension("second.json");
    std::fs::write(&first, r#"{"value":1,"same":true}"#).unwrap();
    std::fs::write(&second, r#"{"value":2,"same":true}"#).unwrap();

    let mut app = StructViewApp::default();
    app.load_comparison(vec![first.clone(), second.clone()]);

    let comparison = app.comparison.as_ref().unwrap();
    assert_eq!(comparison.documents.len(), 2);
    assert_eq!(comparison.differences.len(), 1);
    assert_eq!(comparison.differences[0].path, "$.value");
    assert_eq!(comparison.left_index, 0);
    assert_eq!(comparison.right_index, 1);
    assert_eq!(app.visualization, VisualizationMode::Comparison);
    assert!(app.root.is_none());
    assert!(app.parse_error.is_none());

    std::fs::remove_file(first).unwrap();
    std::fs::remove_file(second).unwrap();
}

#[test]
fn comparing_one_selected_file_uses_the_open_document_as_the_first_version() {
    let prefix =
        std::env::temp_dir().join(format!("struct_view-open-compare-{}", std::process::id()));
    let open_path = prefix.with_extension("open.json");
    let selected_path = prefix.with_extension("selected.json");
    std::fs::write(&open_path, r#"{"value":1}"#).unwrap();
    std::fs::write(&selected_path, r#"{"value":2}"#).unwrap();

    let mut app = StructViewApp::default();
    app.load_file(open_path.clone());
    app.root.as_mut().unwrap().children[0].display_value = "3".to_string();
    app.load_comparison(vec![selected_path.clone()]);

    let comparison = app.comparison.as_ref().unwrap();
    assert_eq!(comparison.documents.len(), 2);
    assert_eq!(comparison.documents[0].path, open_path);
    assert_eq!(comparison.documents[1].path, selected_path);
    assert_eq!(
        comparison.differences[0].values,
        vec![Some(serde_json::json!(3)), Some(serde_json::json!(2))]
    );
    assert_eq!(app.visualization, VisualizationMode::Diff);

    std::fs::remove_file(open_path).unwrap();
    std::fs::remove_file(selected_path).unwrap();
}

#[test]
fn creating_new_file_initializes_editable_document_for_all_formats() {
    let formats = [
        DataFormat::Json,
        DataFormat::Yaml,
        DataFormat::Toml,
        DataFormat::Json5,
    ];

    for (index, format) in formats.into_iter().enumerate() {
        let path = std::env::temp_dir().join(format!(
            "struct_view-create-test-{}-{}.{}",
            std::process::id(),
            index,
            format.extension()
        ));
        let mut app = StructViewApp::default();
        app.create_new_file(path.clone(), format);

        assert_eq!(app.mode, AppMode::Edit);
        assert_eq!(app.file_state.path.as_deref(), Some(path.as_path()));
        assert_eq!(app.file_state.format, Some(format));
        assert_eq!(
            app.root.as_ref().map(|root| root.value_type.clone()),
            Some(JsonValueType::Object)
        );

        let content = std::fs::read_to_string(&path).unwrap();
        let (_, parsed_format) = parse_data(&content, Some(format)).unwrap();
        assert_eq!(parsed_format, format);
        assert_eq!(app.file_state.size_bytes, content.len() as u64);
        std::fs::remove_file(path).unwrap();
    }
}
