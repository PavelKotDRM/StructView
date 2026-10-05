use std::collections::HashSet;

use serde_json::Value;
use struct_view_core::parser::build_path;

use super::SchemaRow;

pub(super) fn collect_schema_rows(
    schema: &Value,
    path: String,
    required: Option<bool>,
    rows: &mut Vec<SchemaRow>,
    key: Option<&str>,
) {
    let object = schema.as_object();
    let type_name = if schema == &Value::Bool(false) {
        "never".to_string()
    } else {
        object
            .and_then(|object| object.get("type"))
            .map(schema_type_name)
            .unwrap_or_else(|| {
                if object.is_some_and(|object| object.contains_key("properties")) {
                    "object".to_string()
                } else if object.is_some_and(|object| object.contains_key("items")) {
                    "array".to_string()
                } else {
                    "any".to_string()
                }
            })
    };
    let constraints = object
        .map(|object| {
            [
                "required",
                "additionalProperties",
                "patternProperties",
                "prefixItems",
                "format",
                "enum",
                "const",
                "default",
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "multipleOf",
                "minLength",
                "maxLength",
                "pattern",
                "minItems",
                "maxItems",
                "uniqueItems",
                "minProperties",
                "maxProperties",
                "contains",
                "minContains",
                "maxContains",
                "propertyNames",
                "unevaluatedItems",
                "unevaluatedProperties",
                "dependentRequired",
                "dependentSchemas",
                "contentEncoding",
                "contentMediaType",
                "contentSchema",
                "allOf",
                "anyOf",
                "oneOf",
                "not",
                "if",
                "then",
                "else",
                "readOnly",
                "writeOnly",
                "deprecated",
                "discriminator",
                "nullable",
                "xml",
                "externalDocs",
                "description",
            ]
            .iter()
            .filter_map(|key| object.get(*key).map(|value| format!("{key}={value}")))
            .collect::<Vec<_>>()
            .join("; ")
        })
        .unwrap_or_default();
    let reference = object
        .and_then(|object| object.get("$ref"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    rows.push(SchemaRow {
        key: key.map(ToOwned::to_owned),
        path: path.clone(),
        type_name,
        required,
        constraints,
        reference,
    });

    let Some(object) = object else {
        return;
    };
    let required_fields = object
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();

    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, property_schema) in properties {
            let property_path = build_path(&path, &Some(name.clone()), false);
            collect_schema_rows(
                property_schema,
                property_path,
                Some(required_fields.contains(name.as_str())),
                rows,
                Some(name),
            );
        }
    }

    if let Some(items) = object.get("items") {
        if let Some(tuple_items) = items.as_array() {
            for (index, item_schema) in tuple_items.iter().enumerate() {
                collect_schema_rows(item_schema, format!("{path}[{index}]"), None, rows, None);
            }
        } else {
            collect_schema_rows(items, format!("{path}[*]"), None, rows, None);
        }
    }

    if let Some(prefix_items) = object.get("prefixItems").and_then(Value::as_array) {
        for (index, item_schema) in prefix_items.iter().enumerate() {
            collect_schema_rows(item_schema, format!("{path}[{index}]"), None, rows, None);
        }
    }

    if let Some(additional) = object
        .get("additionalProperties")
        .filter(|value| value.is_object())
    {
        collect_schema_rows(additional, format!("{path}{{*}}"), None, rows, None);
    }

    if let Some(property_names) = object.get("propertyNames") {
        collect_schema_rows(property_names, format!("{path}{{key}}"), None, rows, None);
    }

    if let Some(patterns) = object.get("patternProperties").and_then(Value::as_object) {
        for (pattern, pattern_schema) in patterns {
            collect_schema_rows(
                pattern_schema,
                format!("{path}[pattern={pattern}]"),
                None,
                rows,
                Some(pattern),
            );
        }
    }

    if let Some(contains) = object.get("contains") {
        collect_schema_rows(contains, format!("{path}.contains[*]"), None, rows, None);
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(schemas) = object.get(keyword).and_then(Value::as_array) {
            for (index, child_schema) in schemas.iter().enumerate() {
                collect_schema_rows(
                    child_schema,
                    format!("{path}.{keyword}[{index}]"),
                    None,
                    rows,
                    None,
                );
            }
        }
    }

    for keyword in ["not", "if", "then", "else"] {
        if let Some(child_schema) = object.get(keyword) {
            collect_schema_rows(
                child_schema,
                format!("{path}.{keyword}"),
                None,
                rows,
                Some(keyword),
            );
        }
    }

    if let Some(dependent_schemas) = object.get("dependentSchemas").and_then(Value::as_object) {
        for (name, child_schema) in dependent_schemas {
            let child_path = build_path(
                &build_path(&path, &Some("dependentSchemas".to_string()), false),
                &Some(name.clone()),
                false,
            );
            collect_schema_rows(child_schema, child_path, None, rows, Some(name));
        }
    }

    for keyword in ["definitions", "$defs"] {
        if let Some(definitions) = object.get(keyword).and_then(Value::as_object) {
            for (name, definition) in definitions {
                let definition_path = build_path(
                    &build_path(&path, &Some(keyword.to_string()), false),
                    &Some(name.clone()),
                    false,
                );
                collect_schema_rows(definition, definition_path, None, rows, Some(name));
            }
        }
    }
}

pub(super) fn schema_type_name(value: &Value) -> String {
    match value {
        Value::String(type_name) => type_name.clone(),
        Value::Array(types) if !types.is_empty() => types
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .collect::<Vec<String>>()
            .join(" | "),
        _ => value.to_string(),
    }
}
