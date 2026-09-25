use super::*;

/// Отрисовать схему JSON Schema, OpenAPI либо выведенную схему примера.
pub(in crate::app) fn show_schema(
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
