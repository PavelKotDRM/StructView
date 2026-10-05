use serde_json::Value;
use struct_view_core::parser::build_path;

use super::super::{SchemaDiagram, SchemaRow, SchemaSource};
use super::detection::is_openapi_document;
use super::rows::collect_schema_rows;

pub(super) fn openapi_schema_diagram(value: &Value) -> Option<SchemaDiagram> {
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
