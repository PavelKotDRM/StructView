use serde_json::Value;

use super::super::super::node::{JsonNode, JsonValueType};
use super::{metadata_value, object_key};

/// Преобразовать узел дерева в JSON-совместимое значение.
///
/// TOML-дата и время представлены строкой при сравнении, копировании и
/// сериализации в форматы без собственного типа datetime. Комментарии
/// исключаются из JSON-совместимого значения, а YAML-теги разворачиваются до
/// содержащегося в них значения. Нестроковые ключи YAML невозможно представить
/// в JSON-объекте, поэтому такое преобразование завершается ошибкой.
pub fn node_to_value(node: &JsonNode) -> Result<Value, String> {
    match node.value_type {
        JsonValueType::Object => {
            let mut map = serde_json::Map::new();
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let key = object_key(child)?;
                if map.insert(key.to_string(), node_to_value(child)?).is_some() {
                    return Err(format!("Ключ «{key}» повторяется в {}", node.path));
                }
            }
            Ok(Value::Object(map))
        }
        JsonValueType::Array => {
            let mut values = Vec::with_capacity(node.data_child_count());
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                values.push(node_to_value(child)?);
            }
            Ok(Value::Array(values))
        }
        JsonValueType::Metadata => metadata_value(node).and_then(node_to_value),
        JsonValueType::Comment => Err(format!(
            "Комментарий в {} не является значением данных",
            node.path
        )),
        JsonValueType::String => {
            let text = serde_json::from_str::<String>(&node.display_value)
                .map_err(|error| format!("Некорректная строка в {}: {error}", node.path))?;
            Ok(Value::String(text))
        }
        JsonValueType::DateTime => Ok(Value::String(node.display_value.clone())),
        JsonValueType::Number | JsonValueType::Float => {
            let number = serde_json::from_str::<serde_json::Number>(&node.display_value)
                .map_err(|error| format!("Некорректное число в {}: {error}", node.path))?;
            Ok(Value::Number(number))
        }
        JsonValueType::Bool => {
            let boolean = node
                .display_value
                .parse::<bool>()
                .map_err(|error| format!("Некорректное bool в {}: {error}", node.path))?;
            Ok(Value::Bool(boolean))
        }
        JsonValueType::Null => Ok(Value::Null),
    }
}
