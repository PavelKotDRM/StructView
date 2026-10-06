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
                let (constraints, constraints_help) =
                    format_schema_constraints(&row.constraints, locale);
                column_label(
                    ui,
                    constraints_width,
                    RichText::new(single_line_text(&constraints)),
                )
                .on_hover_text(constraints_help);
                let reference = row.reference.as_deref().unwrap_or("—");
                let reference_summary = schema_reference_summary(reference);
                column_label(
                    ui,
                    reference_width,
                    RichText::new(single_line_text(&reference_summary))
                        .color(colors.matched)
                        .monospace(),
                )
                .on_hover_text(reference);
            });
        },
    );
}

pub(super) fn schema_reference_summary(reference: &str) -> String {
    const MAX_CHARS: usize = 28;
    let reference = reference.trim();
    if reference.chars().count() <= MAX_CHARS {
        return reference.to_string();
    }

    let target = reference.rsplit('/').next().unwrap_or(reference);
    let target_chars = target.chars().count();
    if target_chars + 2 <= MAX_CHARS {
        return format!("…/{target}");
    }

    let suffix = target
        .chars()
        .rev()
        .take(MAX_CHARS - 2)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("…/{suffix}")
}

pub(super) fn format_schema_constraints(constraints: &str, locale: Locale) -> (String, String) {
    if constraints.is_empty() {
        return ("—".to_string(), String::new());
    }
    let Ok(serde_json::Value::Object(object)) =
        serde_json::from_str::<serde_json::Value>(constraints)
    else {
        if let Some(counts) = constraints
            .strip_prefix("observed in ")
            .and_then(|value| value.strip_suffix(" sample object(s)"))
            .and_then(|value| value.split_once('/'))
        {
            let (seen, total) = counts;
            let total = total.trim_end_matches(|character: char| !character.is_ascii_digit());
            let summary = match locale {
                Locale::Russian => format!(
                    "{}: {seen} из {total} {}",
                    locale.text(TextKey::SchemaConstraintObserved),
                    struct_view_core::parser::plural_ru(
                        total.parse().unwrap_or_default(),
                        "объект",
                        "объекта",
                        "объектов"
                    )
                ),
                Locale::English => format!(
                    "{}: {seen} of {total} samples",
                    locale.text(TextKey::SchemaConstraintObserved)
                ),
            };
            return (summary, constraints.to_string());
        }
        return (constraints.to_string(), constraints.to_string());
    };

    let summary_items = object
        .iter()
        .map(|(key, value)| {
            format!(
                "{}: {}",
                constraint_label(key, locale),
                constraint_value(key, value, locale, false)
            )
        })
        .collect::<Vec<_>>();
    let details = object
        .iter()
        .map(|(key, value)| {
            format!(
                "{}: {}",
                constraint_label(key, locale),
                constraint_value(key, value, locale, true)
            )
        })
        .collect::<Vec<_>>();
    let summary = summary_items
        .iter()
        .take(2)
        .cloned()
        .chain((details.len() > 2).then(|| {
            format!(
                "+{} {}",
                details.len() - 2,
                locale.text(TextKey::SchemaConstraintsMore)
            )
        }))
        .collect::<Vec<_>>()
        .join(" · ");
    let help = details.join("\n");
    (summary, help)
}

fn constraint_label(key: &str, locale: Locale) -> String {
    locale
        .text(match key {
            "format" | "contentMediaType" => TextKey::SchemaConstraintFormat,
            "enum" => TextKey::SchemaConstraintAllowedValues,
            "const" => TextKey::SchemaConstraintConstant,
            "default" => TextKey::SchemaConstraintDefault,
            "minimum" => TextKey::SchemaConstraintMinimum,
            "maximum" => TextKey::SchemaConstraintMaximum,
            "exclusiveMinimum" => TextKey::SchemaConstraintExclusiveMinimum,
            "exclusiveMaximum" => TextKey::SchemaConstraintExclusiveMaximum,
            "multipleOf" => TextKey::SchemaConstraintMultipleOf,
            "minLength" => TextKey::SchemaConstraintMinLength,
            "maxLength" => TextKey::SchemaConstraintMaxLength,
            "pattern" => TextKey::SchemaConstraintPattern,
            "minItems" | "minContains" => TextKey::SchemaConstraintMinItems,
            "maxItems" | "maxContains" => TextKey::SchemaConstraintMaxItems,
            "uniqueItems" => TextKey::SchemaConstraintUniqueItems,
            "minProperties" => TextKey::SchemaConstraintMinProperties,
            "maxProperties" => TextKey::SchemaConstraintMaxProperties,
            "additionalProperties"
            | "unevaluatedProperties"
            | "unevaluatedItems"
            | "contentSchema" => TextKey::SchemaConstraintAdditionalProperties,
            "required" | "dependentRequired" => TextKey::SchemaConstraintRequiredProperties,
            "allOf" | "anyOf" | "oneOf" | "not" | "if" | "then" | "else" | "contains"
            | "propertyNames" | "patternProperties" | "dependentSchemas" | "prefixItems" => {
                TextKey::SchemaConstraintConditional
            }
            "deprecated" => TextKey::SchemaConstraintDeprecated,
            "readOnly" => TextKey::SchemaConstraintReadOnly,
            "writeOnly" => TextKey::SchemaConstraintWriteOnly,
            "nullable" => TextKey::SchemaConstraintNullable,
            "description" | "title" | "examples" | "externalDocs" | "discriminator" | "xml"
            | "contentEncoding" => TextKey::SchemaConstraintNote,
            _ => return key.to_string(),
        })
        .to_string()
}

fn constraint_value(key: &str, value: &serde_json::Value, locale: Locale, full: bool) -> String {
    use serde_json::Value;
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value)
            if matches!(
                key,
                "additionalProperties"
                    | "unevaluatedProperties"
                    | "unevaluatedItems"
                    | "deprecated"
                    | "readOnly"
                    | "writeOnly"
                    | "nullable"
                    | "uniqueItems"
            ) =>
        {
            match (locale, value) {
                (Locale::Russian, true) => "да".to_string(),
                (Locale::Russian, false) => "нет".to_string(),
                (Locale::English, true) => "yes".to_string(),
                (Locale::English, false) => "no".to_string(),
            }
        }
        Value::Array(values) if key == "enum" => {
            let total = values.len();
            let shown = if full { total } else { total.min(3) };
            let shown_values = values
                .iter()
                .take(shown)
                .map(constraint_scalar)
                .collect::<Vec<_>>();
            let suffix = if shown < total {
                format!(" … +{}", total - shown)
            } else {
                String::new()
            };
            format!("{}{}", shown_values.join(", "), suffix)
        }
        Value::Array(values) if key == "required" && !full => values.len().to_string(),
        Value::Array(values) if matches!(key, "required" | "dependentRequired") => {
            format_constraint_list(values, full)
        }
        Value::Array(values) if full => Value::Array(values.clone()).to_string(),
        Value::Array(values) => format!(
            "{} {}",
            values.len(),
            if locale == Locale::Russian {
                struct_view_core::parser::plural_ru(
                    values.len(),
                    "вариант",
                    "варианта",
                    "вариантов",
                )
            } else if values.len() == 1 {
                "option"
            } else {
                "options"
            }
        ),
        Value::Object(object) if full => Value::Object(object.clone()).to_string(),
        Value::Object(object) => format!(
            "{} {}",
            object.len(),
            if locale == Locale::Russian {
                struct_view_core::parser::plural_ru(object.len(), "условие", "условия", "условий")
            } else if object.len() == 1 {
                "condition"
            } else {
                "conditions"
            }
        ),
        _ => value.to_string(),
    }
}

fn format_constraint_list(values: &[serde_json::Value], full: bool) -> String {
    let shown = if full {
        values.len()
    } else {
        values.len().min(3)
    };
    let items = values
        .iter()
        .take(shown)
        .map(constraint_scalar)
        .collect::<Vec<_>>();
    let suffix = if shown < values.len() {
        format!(" … +{}", values.len() - shown)
    } else {
        String::new()
    };
    format!("{}{}", items.join(", "), suffix)
}

fn constraint_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
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
