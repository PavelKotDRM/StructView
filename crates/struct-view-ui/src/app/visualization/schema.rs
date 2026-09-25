use struct_view_core::parser::{JsonNode, node_to_value};

mod explicit;
mod filter;
mod inferred;

pub(in crate::app) use filter::schema_visible_indices;

/// Источник сведений, показанный в диаграмме схемы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SchemaSource {
    JsonSchema,
    OpenApi,
    Inferred,
}

/// Описание одного поля в ожидаемой или выведенной структуре.
#[derive(Debug, Clone)]
pub(in crate::app) struct SchemaRow {
    pub(in crate::app) path: String,
    pub(in crate::app) type_name: String,
    pub(in crate::app) required: Option<bool>,
    pub(in crate::app) constraints: String,
    pub(in crate::app) reference: Option<String>,
}

/// Строковая схема, построенная из JSON Schema/OpenAPI или примера данных.
#[derive(Debug, Clone)]
pub(in crate::app) struct SchemaDiagram {
    pub(in crate::app) source: SchemaSource,
    pub(in crate::app) title: Option<String>,
    pub(in crate::app) rows: Vec<SchemaRow>,
}

pub(in crate::app) fn build_schema_diagram(root: &JsonNode) -> Result<SchemaDiagram, String> {
    let value = node_to_value(root)?;
    Ok(explicit::schema_diagram(&value).unwrap_or_else(|| inferred::inferred_schema(root)))
}
