use std::collections::HashSet;

use serde_json::Value;

use super::super::node::{JsonNode, JsonValueType, ParseError};
use super::parse::{yaml_key_to_string, yaml_number_to_json};
use super::paths::{build_path, plural_ru};

/// Рекурсивно построить [`JsonNode`] из [`serde_json::Value`].
///
/// # Arguments
///
/// * `key` — ключ текущего узла (имя поля или индекс массива).
/// * `is_index` — `true`, если `key` — индекс родительского массива, а не
///   имя поля родительского объекта.
/// * `value` — разобранное значение JSON.
/// * `parent_path` — путь родительского узла.
pub(super) fn build_node(
    key: Option<String>,
    is_index: bool,
    value: &Value,
    parent_path: String,
) -> JsonNode {
    let path = build_path(&parent_path, &key, is_index);
    match value {
        Value::Object(map) => {
            let children = map
                .iter()
                .map(|(k, v)| build_node(Some(k.clone()), false, v, path.clone()))
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Object,
                display_value: format!(
                    "{{{}}} {}",
                    count,
                    plural_ru(count, "поле", "поля", "полей")
                ),
                children,
                expanded: false,
                path,
            }
        }
        Value::Array(arr) => {
            let children = arr
                .iter()
                .enumerate()
                .map(|(i, v)| build_node(Some(i.to_string()), true, v, path.clone()))
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Array,
                display_value: format!(
                    "[{}] {}",
                    count,
                    plural_ru(count, "элемент", "элемента", "элементов")
                ),
                children,
                expanded: false,
                path,
            }
        }
        Value::String(s) => leaf(
            key,
            JsonValueType::String,
            serde_json::Value::String(s.clone()).to_string(),
            path,
        ),
        Value::Number(n) => {
            let value_type = if n.is_f64() {
                JsonValueType::Float
            } else {
                JsonValueType::Number
            };
            leaf(key, value_type, n.to_string(), path)
        }
        Value::Bool(b) => leaf(key, JsonValueType::Bool, b.to_string(), path),
        Value::Null => leaf(key, JsonValueType::Null, "null".to_string(), path),
    }
}

pub(super) fn build_yaml_node(
    key: Option<String>,
    is_index: bool,
    value: &serde_yaml_ng::Value,
    parent_path: String,
) -> Result<JsonNode, ParseError> {
    use serde_yaml_ng::Value as YamlValue;

    let path = build_path(&parent_path, &key, is_index);
    match value {
        YamlValue::Mapping(entries) => {
            let mut keys = HashSet::new();
            let mut children = Vec::with_capacity(entries.len());
            for (raw_key, value) in entries {
                let child_key = yaml_key_to_string(raw_key.clone())?;
                if !keys.insert(child_key.clone()) {
                    return Err(ParseError {
                        message: format!(
                            "Ключи YAML после преобразования в строки совпадают: {child_key}"
                        ),
                        line: None,
                        column: None,
                    });
                }
                children.push(build_yaml_node(
                    Some(child_key),
                    false,
                    value,
                    path.clone(),
                )?);
            }
            let count = children.len();
            Ok(JsonNode {
                key,
                value_type: JsonValueType::Object,
                display_value: format!("{{{count}}} {}", plural_ru(count, "поле", "поля", "полей")),
                children,
                expanded: false,
                path,
            })
        }
        YamlValue::Sequence(values) => {
            let children = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    build_yaml_node(Some(index.to_string()), true, value, path.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let count = children.len();
            Ok(JsonNode {
                key,
                value_type: JsonValueType::Array,
                display_value: format!(
                    "[{count}] {}",
                    plural_ru(count, "элемент", "элемента", "элементов")
                ),
                children,
                expanded: false,
                path,
            })
        }
        YamlValue::Tagged(tagged) => {
            let value_path = format!("{path}::metadata-value");
            let value = build_yaml_node(None, false, &tagged.value, value_path)?;
            Ok(JsonNode {
                key,
                value_type: JsonValueType::Metadata,
                display_value: tagged.tag.to_string(),
                children: vec![value],
                expanded: false,
                path,
            })
        }
        YamlValue::Null => Ok(leaf(key, JsonValueType::Null, "null".to_string(), path)),
        YamlValue::Bool(value) => Ok(leaf(key, JsonValueType::Bool, value.to_string(), path)),
        YamlValue::Number(value) => {
            let value_type = if value.is_f64() {
                JsonValueType::Float
            } else {
                JsonValueType::Number
            };
            Ok(leaf(
                key,
                value_type,
                yaml_number_to_json(value.clone())?.to_string(),
                path,
            ))
        }
        YamlValue::String(value) => Ok(leaf(
            key,
            JsonValueType::String,
            serde_json::Value::String(value.clone()).to_string(),
            path,
        )),
    }
}

pub(super) fn build_toml_node(
    key: Option<String>,
    is_index: bool,
    value: &toml::Value,
    parent_path: String,
) -> JsonNode {
    let path = build_path(&parent_path, &key, is_index);
    match value {
        toml::Value::Table(table) => {
            let children = table
                .iter()
                .map(|(key, value)| build_toml_node(Some(key.clone()), false, value, path.clone()))
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Object,
                display_value: format!(
                    "{{{}}} {}",
                    count,
                    plural_ru(count, "поле", "поля", "полей")
                ),
                children,
                expanded: false,
                path,
            }
        }
        toml::Value::Array(values) => {
            let children = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    build_toml_node(Some(index.to_string()), true, value, path.clone())
                })
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Array,
                display_value: format!(
                    "[{}] {}",
                    count,
                    plural_ru(count, "элемент", "элемента", "элементов")
                ),
                children,
                expanded: false,
                path,
            }
        }
        toml::Value::String(value) => leaf(
            key,
            JsonValueType::String,
            serde_json::Value::String(value.clone()).to_string(),
            path,
        ),
        toml::Value::Integer(value) => leaf(key, JsonValueType::Number, value.to_string(), path),
        toml::Value::Float(value) => leaf(key, JsonValueType::Float, value.to_string(), path),
        toml::Value::Boolean(value) => leaf(key, JsonValueType::Bool, value.to_string(), path),
        toml::Value::Datetime(value) => leaf(key, JsonValueType::DateTime, value.to_string(), path),
    }
}

/// Создать листовой (бездетный) узел дерева.
fn leaf(
    key: Option<String>,
    value_type: JsonValueType,
    display_value: String,
    path: String,
) -> JsonNode {
    JsonNode {
        key,
        value_type,
        display_value,
        children: vec![],
        expanded: false,
        path,
    }
}
