use super::{
    PairChange, comparison_column_width, comparison_value_color, document_label_for_path,
    pair_change,
};

use super::super::theme::{COLOR_ERROR, COLOR_MATCH, COLOR_SUCCESS};
use egui::Color32;
use serde_json::json;
use struct_view_core::parser::DataFormat;

#[test]
fn diff_file_labels_include_format() {
    assert_eq!(
        document_label_for_path(std::path::Path::new("before.json"), DataFormat::Json),
        "before.json (JSON)"
    );
}

#[test]
fn pair_diff_classifies_missing_changed_and_unchanged_values() {
    assert_eq!(pair_change(None, Some(&json!(1))), Some(PairChange::Added));
    assert_eq!(
        pair_change(Some(&json!(1)), None),
        Some(PairChange::Removed)
    );
    assert_eq!(
        pair_change(Some(&json!(1)), Some(&json!(2))),
        Some(PairChange::Changed)
    );
    assert_eq!(pair_change(Some(&json!(null)), Some(&json!(null))), None);
    assert_eq!(pair_change(None, None), None);
}

#[test]
fn comparison_colors_reflect_added_removed_and_changed_values() {
    let original = json!(1);
    let updated = json!(2);
    let unchanged_color = Color32::WHITE;

    assert_eq!(
        comparison_value_color(None, Some(&updated), unchanged_color),
        COLOR_SUCCESS
    );
    assert_eq!(
        comparison_value_color(Some(&original), None, unchanged_color),
        COLOR_ERROR
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&updated), unchanged_color),
        COLOR_MATCH
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&original), unchanged_color),
        unchanged_color
    );
}

#[test]
fn comparison_column_width_tracks_the_available_window_width() {
    assert_eq!(comparison_column_width(612.0, 4), 144.0);
    assert_eq!(comparison_column_width(200.0, 4), 96.0);
}
