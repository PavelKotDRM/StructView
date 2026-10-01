use std::collections::HashSet;

use serde_json::Value;
use struct_view_core::parser::build_path;

use super::{SchemaDiagram, SchemaRow, SchemaSource};

pub(super) fn schema_diagram(value: &Value) -> Option<SchemaDiagram> {
    if let Some(diagram) = openapi_schema_diagram(value) {
        return Some(diagram);
    }
    if looks_like_json_schema(value) {
        let mut rows = Vec::new();
        collect_schema_rows(value, "$".to_string(), None, &mut rows, None);
        return Some(SchemaDiagram {
            source: SchemaSource::JsonSchema,
            title: value
                .get("title")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            rows,
        });
    }

    None
}

pub(super) fn is_schema_document(value: &Value) -> bool {
    is_openapi_document(value) || looks_like_json_schema(value)
}

fn is_openapi_document(value: &Value) -> bool {
    value
        .get("openapi")
        .and_then(Value::as_str)
        .is_some_and(|version| version.starts_with("3."))
        || value.get("swagger").and_then(Value::as_str) == Some("2.0")
}

fn looks_like_json_schema(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("$schema") || object.contains_key("$ref") {
        return true;
    }

    let has_schema_members = [
        "required",
        "additionalProperties",
        "patternProperties",
        "prefixItems",
        "items",
        "enum",
        "const",
        "format",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "contains",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "pattern",
        "uniqueItems",
        "minProperties",
        "maxProperties",
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
        "readOnly",
        "writeOnly",
        "deprecated",
        "discriminator",
        "nullable",
        "xml",
        "oneOf",
        "anyOf",
        "allOf",
        "not",
        "if",
        "then",
        "else",
    ]
    .iter()
    .any(|key| object.contains_key(*key));
    let property_schemas = object.get("properties").and_then(Value::as_object);
    let has_properties = property_schemas.is_some();
    let properties_look_like_schema = property_schemas.is_some_and(|properties| {
        !properties.is_empty()
            && properties.values().all(looks_like_schema_fragment)
            && properties.values().any(has_schema_fragment_evidence)
    });
    let has_known_type = object.get("type").is_some_and(|type_name| match type_name {
        Value::String(type_name) => matches!(
            type_name.as_str(),
            "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
        ),
        Value::Array(types) => types.iter().any(|type_name| {
            type_name.as_str().is_some_and(|type_name| {
                matches!(
                    type_name,
                    "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
                )
            })
        }),
        _ => false,
    });
    let has_definitions = ["definitions", "$defs"].iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_object)
            .is_some_and(|definitions| definitions.values().any(looks_like_schema_fragment))
    });
    let has_only_schema_keywords = object.keys().all(|key| {
        [
            "$schema",
            "$id",
            "$ref",
            "$defs",
            "definitions",
            "type",
            "title",
            "description",
            "default",
            "examples",
            "enum",
            "const",
            "format",
            "properties",
            "required",
            "additionalProperties",
            "patternProperties",
            "items",
            "prefixItems",
            "allOf",
            "anyOf",
            "oneOf",
            "not",
            "if",
            "then",
            "else",
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
            "readOnly",
            "writeOnly",
            "deprecated",
            "discriminator",
            "nullable",
            "xml",
        ]
        .contains(&key.as_str())
    });

    (has_properties && (has_schema_members || has_known_type || properties_look_like_schema))
        || (has_only_schema_keywords && (has_known_type || has_schema_members))
        || has_definitions
}

fn looks_like_schema_fragment(value: &Value) -> bool {
    if value.is_boolean() {
        return true;
    }
    value.as_object().is_some_and(|object| {
        object.is_empty()
            || [
                "$ref",
                "type",
                "properties",
                "items",
                "enum",
                "const",
                "allOf",
                "oneOf",
                "anyOf",
            ]
            .iter()
            .any(|key| object.contains_key(*key))
    })
}

fn has_schema_fragment_evidence(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        [
            "$ref",
            "type",
            "properties",
            "items",
            "enum",
            "const",
            "allOf",
            "oneOf",
            "anyOf",
        ]
        .iter()
        .any(|key| object.contains_key(*key))
    })
}

fn openapi_schema_diagram(value: &Value) -> Option<SchemaDiagram> {
    let object = value.as_object()?;
    if !is_openapi_document(value) {
        return None;
    }

    let mut rows = Vec::new();
    if let Some(schemas) = value
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
    {
        for (name, schema) in schemas {
            let path = build_path("$.components.schemas", &Some(name.clone()), false);
            collect_schema_rows(schema, path, None, &mut rows, Some(name));
        }
    }
    if let Some(definitions) = object.get("definitions").and_then(Value::as_object) {
        for (name, schema) in definitions {
            let path = build_path("$.definitions", &Some(name.clone()), false);
            collect_schema_rows(schema, path, None, &mut rows, Some(name));
        }
    }
    if let Some(components) = object.get("components").and_then(Value::as_object) {
        for (name, component) in components {
            if name != "schemas" && !name.starts_with("x-") {
                let path = build_path("$.components", &Some(name.clone()), false);
                collect_openapi_inline_schemas(component, &path, &mut rows);
            }
        }
    }
    if let Some(paths) = value.get("paths") {
        collect_openapi_inline_schemas(paths, "$.paths", &mut rows);
    }

    Some(SchemaDiagram {
        source: SchemaSource::OpenApi,
        title: value
            .pointer("/info/title")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        rows,
    })
}

fn collect_openapi_inline_schemas(value: &Value, path: &str, rows: &mut Vec<SchemaRow>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = build_path(path, &Some(key.clone()), false);
                if matches!(
                    key.as_str(),
                    "example" | "examples" | "default" | "enum" | "const"
                ) || key.starts_with("x-")
                {
                    continue;
                }
                if key == "schema" && (child.is_object() || child.is_boolean()) {
                    collect_schema_rows(child, child_path, None, rows, Some(key));
                } else {
                    collect_openapi_inline_schemas(child, &child_path, rows);
                }
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_openapi_inline_schemas(child, &format!("{path}[{index}]"), rows);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn collect_schema_rows(
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

fn schema_type_name(value: &Value) -> String {
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
