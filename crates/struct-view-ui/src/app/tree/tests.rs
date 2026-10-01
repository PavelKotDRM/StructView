use super::super::i18n::TextKey;
use super::render::add_child_request;
use super::{VisibleRows, focus_match_path, visible_row_index};
use struct_view_core::parser::{parse_json, set_expanded_all};

#[test]
fn add_child_action_matches_container_type_and_path() {
    let root = parse_json(r#"{"object":{},"array":[],"value":1}"#).unwrap();
    let object = root
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("object"))
        .unwrap();
    let array = root
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("array"))
        .unwrap();
    let value = root
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("value"))
        .unwrap();

    let (label, object_request) = add_child_request(object).unwrap();
    assert!(matches!(label, TextKey::AddField));
    assert_eq!(object_request.parent_path, "object");
    assert!(object_request.is_object);

    let (label, array_request) = add_child_request(array).unwrap();
    assert!(matches!(label, TextKey::AddElement));
    assert_eq!(array_request.parent_path, "array");
    assert!(!array_request.is_object);

    assert!(add_child_request(value).is_none());
}

#[test]
fn focus_match_path_reveals_target_and_collapses_other_branches() {
    let mut root = parse_json(
        r#"{"outer":{"inner":{"value":"needle"},"other":{"value":1}},"second":{"value":2}}"#,
    )
    .unwrap();
    set_expanded_all(&mut root, true);

    assert!(focus_match_path(&mut root, "outer.inner.value"));
    assert!(root.expanded);
    let outer = root
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("outer"))
        .unwrap();
    assert!(outer.expanded);
    assert!(
        outer
            .children
            .iter()
            .find(|node| node.key.as_deref() == Some("inner"))
            .unwrap()
            .expanded
    );
    assert!(
        !outer
            .children
            .iter()
            .find(|node| node.key.as_deref() == Some("other"))
            .unwrap()
            .expanded
    );
    assert!(
        !root
            .children
            .iter()
            .find(|node| node.key.as_deref() == Some("second"))
            .unwrap()
            .expanded
    );
}

#[test]
fn focus_match_path_collapses_tree_for_missing_target() {
    let mut root = parse_json(r#"{"outer":{"value":1}}"#).unwrap();
    set_expanded_all(&mut root, true);

    assert!(!focus_match_path(&mut root, "missing"));
    assert!(!root.expanded);
    assert!(!root.children[0].expanded);
}

#[test]
fn visible_rows_index_respects_expanded_branches() {
    let mut root = parse_json(r#"{"outer":{"value":1},"array":[{"value":2},3]}"#).unwrap();

    assert_eq!(VisibleRows::from_root(&root).len(), 1);

    root.expanded = true;
    assert_eq!(VisibleRows::from_root(&root).len(), 3);

    set_expanded_all(&mut root, true);
    assert_eq!(VisibleRows::from_root(&root).len(), 7);
    assert_eq!(visible_row_index(&root, "array[0].value"), Some(3));
    assert_eq!(visible_row_index(&root, "missing"), None);
}

#[test]
fn rendering_virtualized_rows_have_a_fixed_single_line_height() {
    for (source, mode) in [
        (
            r#"{"line\nbreak":1,"other":2}"#,
            super::super::state::AppMode::View,
        ),
        (
            "{/* first line\nsecond line */ value: 1}",
            super::super::state::AppMode::View,
        ),
        (
            r#"{"object":{},"array":[]}"#,
            super::super::state::AppMode::Edit,
        ),
    ] {
        let mut root = struct_view_core::parser::parse_data(source, None)
            .unwrap()
            .0;
        set_expanded_all(&mut root, true);
        let rows = VisibleRows::from_root(&root);
        let context = egui::Context::default();
        let search = struct_view_core::search::SearchState::default();
        let selected_paths = std::collections::BTreeSet::new();
        let mut measured = None;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(240.0, 200.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let options = super::RenderOptions {
                        search: &search,
                        matching_paths: std::collections::HashSet::new(),
                        format: struct_view_core::parser::DataFormat::Json,
                        mode,
                        scroll_to_path: None,
                        selected_paths: &selected_paths,
                        locale: super::super::i18n::Locale::English,
                    };
                    let height = super::tree_row_height(ui);
                    let expected = rows.len() as f32 * height
                        + rows.len().saturating_sub(1) as f32 * ui.spacing().item_spacing.y;
                    let response = ui.scope(|ui| {
                        super::render_visible_rows(
                            ui,
                            &mut root,
                            &rows,
                            &options,
                            &mut super::TreeOutcome::default(),
                            0..rows.len(),
                        );
                    });
                    measured = Some((response.response.rect.height(), expected));
                },
            )
            .drop_without_applying_deltas();
        let (height, expected) = measured.unwrap();
        assert!(height <= expected + 0.5, "{height} > {expected}: {source}");
    }
}
