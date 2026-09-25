use super::{VisibleRows, focus_match_path, visible_row_index};
use struct_view_core::parser::{parse_json, set_expanded_all};

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
