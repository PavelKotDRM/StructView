use super::{
    PairChange, comparison_column_width, comparison_value_color, document_label_for_path,
    pair_change,
};

use super::super::theme::SyntaxColors;
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
    let decimal_one: serde_json::Value = serde_json::from_str("1.00").unwrap();
    let decimal_two: serde_json::Value = serde_json::from_str("1.0").unwrap();
    assert_eq!(pair_change(Some(&decimal_one), Some(&decimal_two)), None);
}

#[test]
fn comparison_colors_reflect_added_removed_and_changed_values() {
    let original = json!(1);
    let updated = json!(2);
    let unchanged_color = Color32::WHITE;
    let colors = SyntaxColors::new(&egui::Visuals::dark());

    assert_eq!(
        comparison_value_color(None, Some(&updated), unchanged_color, colors),
        colors.success
    );
    assert_eq!(
        comparison_value_color(Some(&original), None, unchanged_color, colors),
        colors.error
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&updated), unchanged_color, colors),
        colors.matched
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&original), unchanged_color, colors),
        unchanged_color
    );
}

#[test]
fn comparison_column_width_tracks_the_available_window_width() {
    assert_eq!(comparison_column_width(612.0, 4), 144.0);
    assert_eq!(comparison_column_width(200.0, 4), 96.0);
}

fn window_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(320.0, 240.0),
        )),
        ..Default::default()
    }
}

fn text_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::TextShape> {
    fn collect<'a>(shape: &'a egui::epaint::Shape, output: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::epaint::Shape::Text(text) => output.push(text),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, output);
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for shape in shapes {
        collect(&shape.shape, &mut texts);
    }
    texts
}

fn text_origin(output: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
    text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == text)
        .map(|shape| shape.pos)
}

#[test]
fn rendering_table_headers_scroll_with_their_columns() {
    let root = struct_view_core::parser::parse_json(r#"{"value":42}"#).unwrap();
    let table = super::super::visualization::build_table(&root);
    let context = egui::Context::default();
    let search = struct_view_core::search::SearchState::default();
    let mut scroll_id = egui::Id::NULL;
    let render = |ui: &mut egui::Ui| {
        super::show_table(ui, &table, &search, super::super::i18n::Locale::English);
    };
    let first = context.run_ui(window_input(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            scroll_id = ui.make_persistent_id(egui::IdSalt::new("table_horizontal"));
            let mut state = egui::scroll_area::State::default();
            state.offset.x = 250.0;
            state.store(ui.ctx(), scroll_id);
            render(ui);
        });
    });
    let mut state = egui::scroll_area::State::load(&context, scroll_id).unwrap_or_default();
    state.offset.x = 300.0;
    state.store(&context, scroll_id);
    let second = context.run_ui(window_input(), |ui| {
        egui::CentralPanel::default().show(ui, render);
    });
    let header_delta = text_origin(&second, "Value")
        .zip(text_origin(&first, "Value"))
        .map(|(second, first)| second.x - first.x);
    let row_delta = text_origin(&second, "42")
        .zip(text_origin(&first, "42"))
        .map(|(second, first)| second.x - first.x);
    first.drop_without_applying_deltas();
    second.drop_without_applying_deltas();
    let header_delta = header_delta.expect("Second column header must remain visible");
    let row_delta = row_delta.expect("Second column value must remain visible");
    assert!(
        (header_delta + 50.0).abs() < 1.0,
        "Header moved by {header_delta}"
    );
    assert!(
        (header_delta - row_delta).abs() < 1.0,
        "Header/body deltas: {header_delta}/{row_delta}"
    );
}

#[test]
fn rendering_schema_union_types_do_not_expand_virtualized_rows() {
    let root = struct_view_core::parser::parse_json(
        r#"{"type":["string","number","boolean","object","array","null"]}"#,
    )
    .unwrap();
    let diagram = super::super::visualization::build_schema_diagram(&root).unwrap();
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 240.0),
    ));
    let output = context.run_ui(input, |ui| {
        super::show_schema(
            ui,
            &diagram,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let lines = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text.contains(" | "))
        .map(|shape| shape.galley.rows.len());
    output.drop_without_applying_deltas();
    assert_eq!(lines, Some(1));
}

#[test]
fn rendering_graph_labels_fit_inside_their_cards() {
    let root = struct_view_core::parser::parse_json(
        &serde_json::json!({"id":"entity","name":"W".repeat(120)}).to_string(),
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let output = context.run_ui(window_input(), |ui| {
        super::show_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let measured = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text.starts_with("WWW"))
        .map(|shape| (shape.galley.rect.width(), shape.galley.rows.len()));
    output.drop_without_applying_deltas();
    let (width, lines) = measured.expect("Graph label must be visible");
    assert!(width <= super::GRAPH_NODE_SIZE.x - 16.0 + 1.0, "{width}");
    assert_eq!(lines, 1);
}

#[test]
fn rendering_graph_labels_use_the_light_theme_palette() {
    let root =
        struct_view_core::parser::parse_json(r#"{"id":"entity","name":"Visible entity"}"#).unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let visuals = egui::Visuals::light();
    context.set_visuals(visuals.clone());
    let output = context.run_ui(window_input(), |ui| {
        super::show_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let label_color = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == "Visible entity")
        .map(|shape| shape.fallback_color);
    output.drop_without_applying_deltas();
    assert_eq!(
        label_color,
        Some(SyntaxColors::new(&visuals).key),
        "Graph labels should remain readable in the light theme"
    );
}

#[test]
fn rendering_graph_keeps_relationship_labels_readable_between_nodes() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 240.0),
    ));
    let output = context.run_ui(input, |ui| {
        super::show_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let relationship_label = text_shapes(&output.shapes)
        .into_iter()
        .any(|shape| shape.galley.job.text == "depends_on");
    output.drop_without_applying_deltas();
    assert!(
        relationship_label,
        "Relationship label should not be truncated"
    );
}
