use struct_view_core::parser::{JsonNode, JsonValueType, node_to_value};

mod explicit;
mod filter;
mod inferred;

pub use filter::schema_visible_indices;

/// Источник сведений, показанный в диаграмме схемы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaSource {
    JsonSchema,
    OpenApi,
    Inferred,
}

/// Описание одного поля в ожидаемой или выведенной структуре.
#[derive(Debug, Clone)]
pub struct SchemaRow {
    pub key: Option<String>,
    pub path: String,
    pub type_name: String,
    pub required: Option<bool>,
    pub constraints: String,
    pub reference: Option<String>,
}

/// Строковая схема, построенная из JSON Schema/OpenAPI или примера данных.
#[derive(Debug, Clone)]
pub struct SchemaDiagram {
    pub source: SchemaSource,
    pub title: Option<String>,
    pub rows: Vec<SchemaRow>,
}

pub fn build_schema_diagram(root: &JsonNode) -> Result<SchemaDiagram, String> {
    // Detection needs keywords and scalar types, not a lossy conversion of native numbers.
    if !explicit::is_schema_document(&schema_probe(root)?) {
        return Ok(inferred::inferred_schema(root));
    }
    let value = node_to_value(root)?;
    explicit::schema_diagram(&value)
        .ok_or_else(|| "Не удалось построить объявленную схему".to_string())
}

fn schema_probe(node: &JsonNode) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    match node.value_type {
        JsonValueType::Object => {
            let mut object = serde_json::Map::new();
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let key = child
                    .key
                    .as_ref()
                    .ok_or_else(|| format!("Отсутствует ключ в {}", child.path))?;
                if object.insert(key.clone(), schema_probe(child)?).is_some() {
                    return Err(format!("Повторяющийся ключ {key} в {}", node.path));
                }
            }
            Ok(Value::Object(object))
        }
        JsonValueType::Array => node
            .children
            .iter()
            .filter(|child| child.value_type != JsonValueType::Comment)
            .map(schema_probe)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        JsonValueType::Metadata => {
            let mut values = node
                .children
                .iter()
                .filter(|child| child.value_type != JsonValueType::Comment);
            let value = values
                .next()
                .ok_or_else(|| format!("У YAML-тега в {} отсутствует значение", node.path))?;
            if values.next().is_some() {
                return Err(format!(
                    "У YAML-тега в {} больше одного значения",
                    node.path
                ));
            }
            schema_probe(value)
        }
        JsonValueType::String => serde_json::from_str::<String>(&node.display_value)
            .map(Value::String)
            .map_err(|error| format!("Некорректная строка в {}: {error}", node.path)),
        JsonValueType::DateTime => Ok(Value::String(node.display_value.clone())),
        JsonValueType::Bool => node
            .display_value
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|error| format!("Некорректное bool в {}: {error}", node.path)),
        JsonValueType::Number | JsonValueType::Float | JsonValueType::Null => Ok(Value::Null),
        JsonValueType::Comment => Err("Комментарий не является документом схемы".to_string()),
    }
}
