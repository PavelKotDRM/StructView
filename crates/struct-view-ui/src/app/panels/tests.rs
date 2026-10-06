use std::thread;
use std::time::{Duration, Instant};

use super::super::state::{ComparisonDocument, ComparisonState};
use super::VisualizationMode;
use super::{AppMode, StructViewApp};
use struct_view_core::parser::parse_json;

#[test]
fn status_bar_uses_separate_fields_and_gui_dividers_in_all_document_modes() {
    use super::super::i18n::Locale;
    use struct_view_core::parser::DataFormat;
    for locale in Locale::ALL {
        for mode in [
            VisualizationMode::Tree,
            VisualizationMode::Graph,
            VisualizationMode::Table,
            VisualizationMode::Schema,
            VisualizationMode::Comparison,
            VisualizationMode::Diff,
        ] {
            let mut app = StructViewApp::default();
            app.locale = locale;
            app.visualization = mode;
            app.file_state.path = Some("sample.json".into());
            app.file_state.format = Some(DataFormat::Json);
            app.file_state.size_bytes = 2048;
            app.file_state.load_time_ms = 12;
            let comparison = matches!(
                mode,
                VisualizationMode::Comparison | VisualizationMode::Diff
            );
            if comparison {
                app.comparison = Some(ComparisonState {
                    documents: ["first.json", "second.json"]
                        .into_iter()
                        .map(|path| ComparisonDocument {
                            path: path.into(),
                            size_bytes: 0,
                            load_time_ms: 0,
                            format: DataFormat::Json,
                        })
                        .collect(),
                    differences: Vec::new(),
                    left_index: 0,
                    right_index: 1,
                    pair_cache: None,
                    previous_document: None,
                });
            }
            let context = egui::Context::default();
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| app.show_bottom_panel(ui),
            );
            output.textures_delta.clear();
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = if comparison {
                locale.comparison_status(2, 0)
            } else {
                locale.loaded_file_status("sample.json", "JSON", 2.0, 12)
            };
            for field in expected {
                assert!(
                    labels.contains(&field.as_str()),
                    "Missing field {field} in {mode:?}"
                );
            }
            assert!(labels.iter().all(|text| !text.contains('|')));
            let dividers = output
                .shapes
                .iter()
                .filter(|shape| {
                    matches!(
                        &shape.shape, egui::epaint::Shape::LineSegment { points, .. }
                            if (points[0].x - points[1].x).abs() < 0.01
                    )
                })
                .count();
            assert!(
                dividers >= if comparison { 1 } else { 3 },
                "Missing GUI dividers in {mode:?}"
            );
            output.drop_without_applying_deltas();
        }
    }
}

fn shape_contains_text(shape: &egui::epaint::Shape, expected: &str) -> bool {
    match shape {
        egui::epaint::Shape::Text(text) => text.galley.job.text == expected,
        egui::epaint::Shape::Vec(shapes) => shapes
            .iter()
            .any(|shape| shape_contains_text(shape, expected)),
        _ => false,
    }
}

#[test]
fn large_graph_view_shows_progress_without_blocking_the_ui() {
    let entities = (0..100)
        .map(|index| {
            let mut node = serde_json::Map::new();
            node.insert(
                "id".to_string(),
                serde_json::Value::String(format!("entity-{index}")),
            );
            node.insert(
                "depends_on".to_string(),
                serde_json::Value::String(format!("entity-{}", (index + 1) % 100)),
            );
            if index < 4 {
                node.insert(
                    "parent_id".to_string(),
                    serde_json::Value::String(format!("entity-{}", (index + 7) % 100)),
                );
            }
            serde_json::Value::Object(node)
        })
        .collect::<Vec<_>>();
    let mut app = StructViewApp::default();
    app.root = Some(parse_json(&serde_json::Value::Array(entities).to_string()).unwrap());
    app.visualization = VisualizationMode::Graph;
    let context = egui::Context::default();
    let started_at = Instant::now();
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 500.0),
            )),
            ..Default::default()
        },
        |ui| app.show_central_panel(ui),
    );
    let loading_label = app
        .locale
        .text(super::super::i18n::TextKey::GraphCalculating);
    let is_visible = output
        .shapes
        .iter()
        .any(|shape| shape_contains_text(&shape.shape, loading_label));
    output.drop_without_applying_deltas();

    assert!(is_visible, "Graph calculation progress should be visible");
    assert!(
        started_at.elapsed() < Duration::from_millis(250),
        "The UI frame must not wait for graph routing: {:?}",
        started_at.elapsed()
    );

    let deadline = Instant::now() + Duration::from_secs(10);
    while app.graph_calculation.result().is_none()
        && app.graph_calculation.error().is_none()
        && Instant::now() < deadline
    {
        app.graph_calculation.poll(&context);
        if app.graph_calculation.result().is_none() {
            thread::sleep(Duration::from_millis(1));
        }
    }
    let result = app
        .graph_calculation
        .result()
        .expect("The background graph calculation should finish");
    assert_eq!(result.graph.nodes.len(), 100);
    assert_eq!(result.graph.edges.len(), 104);
}

#[test]
fn delete_key_requests_deletion_of_selected_structure_in_edit_mode() {
    let mut app = StructViewApp::default();
    app.root = Some(parse_json(r#"{"value":1}"#).unwrap());
    app.mode = AppMode::Edit;
    app.selected_paths = std::collections::BTreeSet::from(["value".to_string()]);
    let context = egui::Context::default();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: Some(egui::Key::Delete),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..Default::default()
    };

    context
        .run_ui(input, |ui| app.handle_shortcuts(ui.ctx()))
        .drop_without_applying_deltas();

    assert!(app.delete_requested);
}

#[test]
fn comparison_and_diff_keep_long_values_in_single_line_columns() {
    let long_value = "V".repeat(300);
    let values = [
        serde_json::json!({"value": long_value}),
        serde_json::json!({"value": "changed"}),
    ];
    let mut app = StructViewApp::default();
    app.comparison = Some(ComparisonState {
        documents: ["first.json", "second.json"]
            .into_iter()
            .map(|path| ComparisonDocument {
                path: path.into(),
                size_bytes: 0,
                load_time_ms: 0,
                format: struct_view_core::parser::DataFormat::Json,
            })
            .collect(),
        differences: struct_view_core::diff::compare_values(&values),
        left_index: 0,
        right_index: 1,
        pair_cache: None,
        previous_document: None,
    });
    let context = egui::Context::default();

    fn collect_long_values(shape: &egui::epaint::Shape, values: &mut Vec<(f32, usize)>) {
        match shape {
            egui::epaint::Shape::Text(text)
                if text.galley.job.text.starts_with("\"VV")
                    || text.galley.job.text.starts_with("VV") =>
            {
                values.push((text.galley.rect.width(), text.galley.rows.len()));
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_long_values(shape, values);
                }
            }
            _ => {}
        }
    }

    for mode in [VisualizationMode::Comparison, VisualizationMode::Diff] {
        app.visualization = mode;
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(850.0, 250.0),
                )),
                ..Default::default()
            },
            |ui| app.show_central_panel(ui),
        );
        let mut values = Vec::new();
        for shape in &output.shapes {
            collect_long_values(&shape.shape, &mut values);
        }
        assert!(!values.is_empty(), "{mode:?}: long value was not rendered");
        let width = super::comparison_column_width(
            850.0,
            if mode == VisualizationMode::Diff {
                4
            } else {
                3
            },
        );
        assert!(
            values
                .iter()
                .all(|(actual_width, rows)| *rows == 1 && *actual_width <= width + 1.0),
            "{mode:?}: {values:?}, column width {width}"
        );
        output.drop_without_applying_deltas();
    }
}
