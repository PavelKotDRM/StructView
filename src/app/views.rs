use std::collections::HashSet;

use egui::{Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Vec2};
use serde_json::Value;

use crate::diff::format_value;
use crate::parser::{DataFormat, JsonValueType};
use crate::search::SearchState;

use super::i18n::{Locale, TextKey};
use super::state::ComparisonState;
use super::theme::{COLOR_ACTIVE_MATCH, COLOR_ERROR, COLOR_KEY, COLOR_MATCH, COLOR_SUCCESS};
use super::visualization::{
    RelationshipGraph, SchemaDiagram, SchemaSource, TableData, schema_visible_indices,
    table_to_csv, table_visible_indices,
};

const MIN_TABLE_WIDTH: f32 = 760.0;
const GRAPH_NODE_SIZE: Vec2 = Vec2::new(208.0, 70.0);
const GRAPH_STEP: Vec2 = Vec2::new(250.0, 116.0);

/// Отрисовать таблицу и вернуть `true`, если был запрошен экспорт CSV.
pub(super) fn show_table(
    ui: &mut egui::Ui,
    table: &TableData,
    search: &SearchState,
    locale: Locale,
) -> bool {
    let export_clicked = ui
        .horizontal(|ui| {
            ui.label(locale.text(TextKey::TableDescription));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.button(locale.text(TextKey::ExportCsv)).clicked()
            })
            .inner
        })
        .inner;

    let visible_indices = table_visible_indices(table, search);
    let visible_row_count = visible_indices.as_ref().map_or(table.rows.len(), Vec::len);
    if visible_row_count == 0 {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::NotFound));
        });
        return export_clicked;
    }

    let width = ui.available_width().max(MIN_TABLE_WIDTH);
    let path_width = width * 0.43;
    let value_width = width * 0.39;
    let type_width = width - path_width - value_width;
    let row_height = ui.spacing().interact_size.y;

    ui.horizontal(|ui| {
        table_header(
            ui,
            path_width,
            locale.text(TextKey::ComparisonPath),
            row_height,
        );
        table_header(ui, value_width, locale.text(TextKey::Value), row_height);
        table_header(ui, type_width, locale.text(TextKey::SchemaType), row_height);
    });

    egui::ScrollArea::both().auto_shrink([false; 2]).show_rows(
        ui,
        row_height,
        visible_row_count,
        |ui, row_range| {
            for visible_index in row_range {
                let row_index = visible_indices
                    .as_ref()
                    .map_or(visible_index, |indices| indices[visible_index]);
                let row = &table.rows[row_index];
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [path_width, row_height],
                        egui::Label::new(RichText::new(&row.path).monospace()).truncate(),
                    )
                    .on_hover_text(&row.path);
                    ui.add_sized(
                        [value_width, row_height],
                        egui::Label::new(
                            RichText::new(&row.value)
                                .color(value_color(&row.value_type))
                                .monospace(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&row.value);
                    ui.add_sized(
                        [type_width, row_height],
                        egui::Label::new(value_type_label(locale, &row.value_type)),
                    );
                });
            }
        },
    );

    export_clicked
}

fn table_header(ui: &mut egui::Ui, width: f32, label: &str, height: f32) {
    ui.add_sized(
        [width, height],
        egui::Label::new(RichText::new(label).strong()),
    );
}

/// Отрисовать схему JSON Schema, OpenAPI либо выведенную схему примера.
pub(super) fn show_schema(
    ui: &mut egui::Ui,
    diagram: &SchemaDiagram,
    search: &SearchState,
    locale: Locale,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(schema_source_label(diagram.source, locale)).strong());
        if let Some(title) = &diagram.title {
            ui.label(title);
        }
        if diagram.source == SchemaSource::Inferred {
            ui.label(RichText::new(locale.text(TextKey::SchemaInferredNotice)).weak());
        }
    });

    if diagram.rows.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::SchemaNoDefinitions));
        });
        return;
    }

    let visible_indices = schema_visible_indices(diagram, search);
    let visible_row_count = visible_indices
        .as_ref()
        .map_or(diagram.rows.len(), Vec::len);
    if visible_row_count == 0 {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::NotFound));
        });
        return;
    }

    let width = ui.available_width().max(1_080.0);
    let path_width = width * 0.32;
    let type_width = width * 0.14;
    let required_width = width * 0.16;
    let constraints_width = width * 0.22;
    let reference_width = width - path_width - type_width - required_width - constraints_width;
    let row_height = ui.spacing().interact_size.y;

    ui.horizontal(|ui| {
        table_header(
            ui,
            path_width,
            locale.text(TextKey::ComparisonPath),
            row_height,
        );
        table_header(ui, type_width, locale.text(TextKey::SchemaType), row_height);
        table_header(
            ui,
            required_width,
            locale.text(TextKey::SchemaRequired),
            row_height,
        );
        table_header(
            ui,
            constraints_width,
            locale.text(TextKey::SchemaConstraints),
            row_height,
        );
        table_header(
            ui,
            reference_width,
            locale.text(TextKey::SchemaReference),
            row_height,
        );
    });

    egui::ScrollArea::both().auto_shrink([false; 2]).show_rows(
        ui,
        row_height,
        visible_row_count,
        |ui, row_range| {
            for visible_index in row_range {
                let row_index = visible_indices
                    .as_ref()
                    .map_or(visible_index, |indices| indices[visible_index]);
                let row = &diagram.rows[row_index];
                let required = match (diagram.source, row.required) {
                    (_, None) => "—",
                    (SchemaSource::JsonSchema | SchemaSource::OpenApi, Some(true)) => {
                        locale.text(TextKey::SchemaRequiredValue)
                    }
                    (SchemaSource::JsonSchema | SchemaSource::OpenApi, Some(false)) => {
                        locale.text(TextKey::SchemaOptionalValue)
                    }
                    (SchemaSource::Inferred, Some(true)) => {
                        locale.text(TextKey::SchemaPresentInAllSamples)
                    }
                    (SchemaSource::Inferred, Some(false)) => {
                        locale.text(TextKey::SchemaPresentInSomeSamples)
                    }
                };
                let required_color = match (diagram.source, row.required) {
                    (SchemaSource::JsonSchema | SchemaSource::OpenApi, Some(true))
                    | (SchemaSource::Inferred, Some(true)) => COLOR_SUCCESS,
                    (_, Some(false)) => COLOR_MATCH,
                    (_, None) => Color32::GRAY,
                };
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [path_width, row_height],
                        egui::Label::new(RichText::new(&row.path).monospace()).truncate(),
                    )
                    .on_hover_text(&row.path);
                    ui.add_sized(
                        [type_width, row_height],
                        egui::Label::new(schema_type_label(&row.type_name, locale)),
                    );
                    ui.add_sized(
                        [required_width, row_height],
                        egui::Label::new(RichText::new(required).color(required_color)),
                    );
                    let constraints = if row.constraints.is_empty() {
                        "—"
                    } else {
                        &row.constraints
                    };
                    ui.add_sized(
                        [constraints_width, row_height],
                        egui::Label::new(RichText::new(constraints).monospace()).truncate(),
                    )
                    .on_hover_text(&row.constraints);
                    let reference = row.reference.as_deref().unwrap_or("—");
                    ui.add_sized(
                        [reference_width, row_height],
                        egui::Label::new(RichText::new(reference).color(COLOR_MATCH).monospace())
                            .truncate(),
                    )
                    .on_hover_text(reference);
                });
            }
        },
    );
}

/// Отрисовать граф идентификаторов и зависимостей.
pub(super) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    search: &SearchState,
    locale: Locale,
) {
    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphNodes),
            graph.nodes.len()
        ));
        ui.separator();
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphEdges),
            graph.edges.len()
        ));
    });

    if graph.nodes.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::GraphNoEntities));
        });
        return;
    }
    if graph.edges.is_empty() {
        ui.label(RichText::new(locale.text(TextKey::GraphNoRelationships)).weak());
    }
    let matching_paths = search
        .matches
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let active_path = search.current_match_path();

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let node_count = graph.nodes.len();
            let columns = (node_count as f32).sqrt().ceil() as usize;
            let columns = columns.max(1);
            let rows = node_count.div_ceil(columns);
            let viewport = ui.available_size_before_wrap();
            let canvas_size = Vec2::new(
                viewport.x.max(columns as f32 * GRAPH_STEP.x + 48.0),
                viewport.y.max(rows as f32 * GRAPH_STEP.y + 48.0),
            );
            let (response, painter) = ui.allocate_painter(canvas_size, Sense::hover());
            let canvas = response.rect;
            let positions = (0..node_count)
                .map(|index| {
                    let column = index % columns;
                    let row = index / columns;
                    Pos2::new(
                        canvas.left()
                            + 24.0
                            + column as f32 * GRAPH_STEP.x
                            + GRAPH_NODE_SIZE.x / 2.0,
                        canvas.top() + 24.0 + row as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                    )
                })
                .collect::<Vec<_>>();

            for edge in &graph.edges {
                let start = positions[edge.source];
                let end = positions[edge.target];
                let direction = (end - start).normalized();
                let start_offset = box_border_offset(direction);
                let end_offset = box_border_offset(direction);
                let line_start = start + direction * start_offset;
                let line_end = end - direction * end_offset;
                let stroke = Stroke::new(1.5, COLOR_KEY);
                painter.line_segment([line_start, line_end], stroke);
                draw_arrow_head(&painter, line_end, direction, stroke);
                let label_position = Pos2::new(
                    (line_start.x + line_end.x) / 2.0,
                    (line_start.y + line_end.y) / 2.0 - 8.0,
                );
                painter.text(
                    label_position,
                    Align2::CENTER_CENTER,
                    shorten(&edge.label, 18),
                    FontId::proportional(12.0),
                    COLOR_KEY,
                );
            }

            for (index, node) in graph.nodes.iter().enumerate() {
                let center = positions[index];
                let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE);
                let active_match = node
                    .search_paths
                    .iter()
                    .any(|path| active_path == Some(path.as_str()));
                let is_match = active_match
                    || node
                        .search_paths
                        .iter()
                        .any(|path| matching_paths.contains(path.as_str()));
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::same(6),
                    ui.visuals().faint_bg_color,
                );
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(6),
                    if active_match {
                        Stroke::new(2.5, COLOR_ACTIVE_MATCH)
                    } else if is_match {
                        Stroke::new(2.0, COLOR_MATCH)
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke
                    },
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    Pos2::new(center.x, center.y - 9.0),
                    Align2::CENTER_CENTER,
                    shorten(&node.label, 24),
                    FontId::proportional(15.0),
                    COLOR_KEY,
                );
                painter.text(
                    Pos2::new(center.x, center.y + 13.0),
                    Align2::CENTER_CENTER,
                    shorten(&node.id, 26),
                    FontId::monospace(11.0),
                    ui.visuals().weak_text_color(),
                );
                ui.interact(
                    rect,
                    ui.make_persistent_id(("graph-node", &node.path)),
                    Sense::hover(),
                )
                .on_hover_text(format!("{}\n{}\n{}", node.label, node.id, node.path));
            }
        });
}

fn shorten(text: &str, max_chars: usize) -> String {
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn box_border_offset(direction: Vec2) -> f32 {
    if direction.x.abs() >= direction.y.abs() {
        GRAPH_NODE_SIZE.x / 2.0
    } else {
        GRAPH_NODE_SIZE.y / 2.0
    }
}

fn draw_arrow_head(painter: &egui::Painter, tip: Pos2, direction: Vec2, stroke: Stroke) {
    let arrow_length = 9.0;
    for angle_offset in [2.55, -2.55] {
        let wing = tip + Vec2::angled(direction.angle() + angle_offset) * arrow_length;
        painter.line_segment([tip, wing], stroke);
    }
}

/// Отрисовать парный diff с выбором сравниваемых файлов.
pub(super) fn show_diff(ui: &mut egui::Ui, comparison: &mut ComparisonState, locale: Locale) {
    let mut left_index = comparison.left_index;
    let mut right_index = comparison.right_index;
    ui.horizontal(|ui| {
        ui.label(locale.text(TextKey::DiffLeft));
        egui::ComboBox::from_id_salt("diff_left_document")
            .selected_text(document_label(comparison, left_index))
            .show_ui(ui, |ui| {
                for (index, document) in comparison.documents.iter().enumerate() {
                    ui.selectable_value(
                        &mut left_index,
                        index,
                        document_label_for_path(&document.path, document.format),
                    );
                }
            });
        ui.label(locale.text(TextKey::DiffRight));
        egui::ComboBox::from_id_salt("diff_right_document")
            .selected_text(document_label(comparison, right_index))
            .show_ui(ui, |ui| {
                for (index, document) in comparison.documents.iter().enumerate() {
                    ui.selectable_value(
                        &mut right_index,
                        index,
                        document_label_for_path(&document.path, document.format),
                    );
                }
            });
    });
    comparison.left_index = left_index;
    comparison.right_index = right_index;

    let pair_differences = comparison
        .differences
        .iter()
        .filter(|difference| {
            difference.values.get(left_index) != difference.values.get(right_index)
        })
        .collect::<Vec<_>>();
    if pair_differences.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(locale.text(TextKey::DiffNoDifferences))
                    .size(18.0)
                    .color(COLOR_SUCCESS),
            );
        });
        return;
    }

    egui::ScrollArea::both().show(ui, |ui| {
        egui::Grid::new("pair_diff_grid")
            .striped(true)
            .min_col_width(140.0)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new(locale.text(TextKey::ComparisonPath)).strong());
                ui.label(RichText::new(locale.text(TextKey::DiffChangeType)).strong());
                ui.label(
                    RichText::new(document_label(comparison, left_index))
                        .strong()
                        .monospace(),
                );
                ui.label(
                    RichText::new(document_label(comparison, right_index))
                        .strong()
                        .monospace(),
                );
                ui.end_row();

                for difference in pair_differences {
                    let left = difference.values.get(left_index).and_then(Option::as_ref);
                    let right = difference.values.get(right_index).and_then(Option::as_ref);
                    let (change_type, left_color, right_color) = match pair_change(left, right) {
                        Some(PairChange::Added) => (
                            locale.text(TextKey::DiffAdded),
                            Color32::GRAY,
                            COLOR_SUCCESS,
                        ),
                        Some(PairChange::Removed) => (
                            locale.text(TextKey::DiffRemoved),
                            COLOR_ERROR,
                            Color32::GRAY,
                        ),
                        Some(PairChange::Changed) => {
                            (locale.text(TextKey::DiffChanged), COLOR_MATCH, COLOR_MATCH)
                        }
                        None => continue,
                    };
                    ui.label(
                        RichText::new(&difference.path)
                            .color(COLOR_MATCH)
                            .monospace(),
                    );
                    ui.label(change_type);
                    ui.label(
                        RichText::new(
                            left.map(|value| format_value(Some(value)))
                                .unwrap_or_else(|| locale.text(TextKey::MissingValue).to_string()),
                        )
                        .color(left_color)
                        .monospace(),
                    );
                    ui.label(
                        RichText::new(
                            right
                                .map(|value| format_value(Some(value)))
                                .unwrap_or_else(|| locale.text(TextKey::MissingValue).to_string()),
                        )
                        .color(right_color)
                        .monospace(),
                    );
                    ui.end_row();
                }
            });
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairChange {
    Added,
    Removed,
    Changed,
}

fn pair_change(left: Option<&Value>, right: Option<&Value>) -> Option<PairChange> {
    match (left, right) {
        (None, Some(_)) => Some(PairChange::Added),
        (Some(_), None) => Some(PairChange::Removed),
        (Some(left), Some(right)) if left != right => Some(PairChange::Changed),
        (None, None) | (Some(_), Some(_)) => None,
    }
}

fn document_label(comparison: &ComparisonState, index: usize) -> String {
    comparison
        .documents
        .get(index)
        .map(|document| document_label_for_path(&document.path, document.format))
        .unwrap_or_default()
}

fn document_label_for_path(path: &std::path::Path, format: DataFormat) -> String {
    format!("{} ({format})", path.display())
}

fn schema_source_label(source: SchemaSource, locale: Locale) -> &'static str {
    match source {
        SchemaSource::JsonSchema => locale.text(TextKey::SchemaJsonSchema),
        SchemaSource::OpenApi => locale.text(TextKey::SchemaOpenApi),
        SchemaSource::Inferred => locale.text(TextKey::SchemaInferred),
    }
}

fn schema_type_label(type_name: &str, locale: Locale) -> String {
    type_name
        .split(" | ")
        .map(|type_name| match type_name {
            "object" => locale.text(TextKey::TypeObject),
            "array" => locale.text(TextKey::TypeArray),
            "string" => locale.text(TextKey::TypeString),
            "date-time" => locale.text(TextKey::TypeDateTime),
            "number" => locale.text(TextKey::TypeNumber),
            "integer" => locale.text(TextKey::TypeInteger),
            "boolean" => locale.text(TextKey::TypeBoolean),
            "null" => locale.text(TextKey::TypeNull),
            "any" => locale.text(TextKey::SchemaAnyType),
            "never" => locale.text(TextKey::SchemaNeverType),
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn value_type_label(locale: Locale, value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::Object => locale.text(TextKey::TypeObject),
        JsonValueType::Array => locale.text(TextKey::TypeArray),
        JsonValueType::String => locale.text(TextKey::TypeString),
        JsonValueType::DateTime => locale.text(TextKey::TypeDateTime),
        JsonValueType::Number => locale.text(TextKey::TypeNumber),
        JsonValueType::Float => locale.text(TextKey::TypeFloat),
        JsonValueType::Bool => locale.text(TextKey::TypeBoolean),
        JsonValueType::Null => locale.text(TextKey::TypeNull),
    }
}

fn value_color(value_type: &JsonValueType) -> Color32 {
    match value_type {
        JsonValueType::String | JsonValueType::DateTime => super::theme::COLOR_STRING,
        JsonValueType::Number | JsonValueType::Float => super::theme::COLOR_NUMBER,
        JsonValueType::Bool => super::theme::COLOR_BOOL,
        JsonValueType::Null => super::theme::COLOR_NULL,
        JsonValueType::Object | JsonValueType::Array => COLOR_KEY,
    }
}

/// Формировать экспорт CSV после применения поиска, вызываемое из состояния.
pub(super) fn export_table_csv(table: &TableData, search: &SearchState) -> String {
    table_to_csv(table, search)
}

#[cfg(test)]
mod tests {
    use super::{PairChange, document_label_for_path, pair_change};

    use crate::parser::DataFormat;
    use serde_json::json;

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
}
