use super::*;

/// Отрисовать схему JSON Schema, OpenAPI либо выведенную схему примера.
pub(in crate::app) fn show_schema(
    ui: &mut egui::Ui,
    diagram: &SchemaDiagram,
    search: &SearchState,
    locale: Locale,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(schema_source_label(diagram.source, locale)).strong());
        if let Some(title) = &diagram.title {
            ui.add(egui::Label::new(single_line_text(title)).truncate())
                .on_hover_text(title);
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

    let colors = SyntaxColors::new(ui.visuals());
    let width = (ui.available_width().max(1_080.0) - 4.0 * ui.spacing().item_spacing.x).max(1.0);
    let path_width = width * 0.32;
    let type_width = width * 0.14;
    let required_width = width * 0.16;
    let constraints_width = width * 0.22;
    let reference_width = width - path_width - type_width - required_width - constraints_width;
    let columns = [
        Column {
            width: path_width,
            header: locale.text(TextKey::ComparisonPath),
        },
        Column {
            width: type_width,
            header: locale.text(TextKey::SchemaType),
        },
        Column {
            width: required_width,
            header: locale.text(TextKey::SchemaRequired),
        },
        Column {
            width: constraints_width,
            header: locale.text(TextKey::SchemaConstraints),
        },
        Column {
            width: reference_width,
            header: locale.text(TextKey::SchemaReference),
        },
    ];
    show_virtualized_columns(
        ui,
        "schema_horizontal",
        &columns,
        visible_row_count,
        |ui, visible_index| {
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
                | (SchemaSource::Inferred, Some(true)) => colors.success,
                (_, Some(false)) => colors.matched,
                (_, None) => colors.null,
            };
            let type_label = schema_type_label(&row.type_name, locale);
            ui.horizontal(|ui| {
                column_label(
                    ui,
                    path_width,
                    RichText::new(single_line_text(&row.path)).monospace(),
                )
                .on_hover_text(&row.path);
                column_label(ui, type_width, RichText::new(single_line_text(&type_label)))
                    .on_hover_text(&type_label);
                column_label(
                    ui,
                    required_width,
                    RichText::new(required).color(required_color),
                )
                .on_hover_text(required);
                let constraints = if row.constraints.is_empty() {
                    "—"
                } else {
                    &row.constraints
                };
                column_label(
                    ui,
                    constraints_width,
                    RichText::new(single_line_text(constraints)).monospace(),
                )
                .on_hover_text(&row.constraints);
                let reference = row.reference.as_deref().unwrap_or("—");
                column_label(
                    ui,
                    reference_width,
                    RichText::new(single_line_text(reference))
                        .color(colors.matched)
                        .monospace(),
                )
                .on_hover_text(reference);
            });
        },
    );
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
