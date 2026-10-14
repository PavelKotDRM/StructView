use serde_json::Value;
mod detection;
mod openapi;
mod rows;

use super::{SchemaDiagram, SchemaRow, SchemaSource};
use detection::looks_like_json_schema;
use rows::collect_schema_rows;

pub(super) fn schema_diagram(value: &Value) -> Option<SchemaDiagram> {
    if let Some(diagram) = openapi::openapi_schema_diagram(value) {
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
    detection::is_openapi_document(value) || looks_like_json_schema(value)
}
