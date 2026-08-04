//! Правка примитивных значений дерева и обратное преобразование в [`serde_json::Value`].

use serde_json::Value;

use crate::parser::{JsonNode, JsonValueType};

/// Преобразовать JSON-узел обратно в [`serde_json::Value`].
///
/// Используется при сохранении файла: дерево хранит значения как строки,
/// поэтому каждый лист заново разбирается в типизированное значение.
///
/// # Errors
///
/// Возвращает описание ошибки с путём узла, если отображаемое значение
/// не является корректным JSON-литералом ожидаемого типа.
pub(super) fn node_to_value(node: &JsonNode) -> Result<Value, String> {
    match node.value_type {
        JsonValueType::Object => {
            let mut map = serde_json::Map::new();
            for child in &node.children {
                let key = child.key.clone().unwrap_or_default();
                map.insert(key, node_to_value(child)?);
            }
            Ok(Value::Object(map))
        }
        JsonValueType::Array => {
            let mut values = Vec::with_capacity(node.children.len());
            for child in &node.children {
                values.push(node_to_value(child)?);
            }
            Ok(Value::Array(values))
        }
        JsonValueType::String => {
            let text = serde_json::from_str::<String>(&node.display_value)
                .map_err(|e| format!("Некорректная строка в {}: {}", node.path, e))?;
            Ok(Value::String(text))
        }
        JsonValueType::Number => {
            let number = serde_json::from_str::<serde_json::Number>(&node.display_value)
                .map_err(|e| format!("Некорректное число в {}: {}", node.path, e))?;
            Ok(Value::Number(number))
        }
        JsonValueType::Bool => {
            let boolean = node
                .display_value
                .parse::<bool>()
                .map_err(|e| format!("Некорректное bool в {}: {}", node.path, e))?;
            Ok(Value::Bool(boolean))
        }
        JsonValueType::Null => Ok(Value::Null),
    }
}

/// Применить правку к примитивному узлу (string / number / bool / null).
///
/// Тип узла определяется по введённому литералу: `null`, `true`/`false`,
/// строка в двойных кавычках или число. Строки нормализуются через
/// `serde_json`, чтобы экранирование оставалось корректным.
///
/// # Errors
///
/// Возвращает сообщение для пользователя, если значение пустое или не
/// является допустимым JSON-литералом.
pub(super) fn apply_primitive_edit(node: &mut JsonNode, edited: &str) -> Result<(), String> {
    let trimmed = edited.trim();
    if trimmed.is_empty() {
        return Err("Значение не может быть пустым".to_string());
    }

    let (value_type, display_value) = if trimmed == "null" {
        (JsonValueType::Null, "null".to_string())
    } else if trimmed == "true" || trimmed == "false" {
        (JsonValueType::Bool, trimmed.to_string())
    } else if trimmed.starts_with('"') {
        let parsed = serde_json::from_str::<String>(trimmed)
            .map_err(|e| format!("Некорректная строка: {}", e))?;
        let normalized = serde_json::to_string(&parsed)
            .map_err(|e| format!("Ошибка сериализации строки: {}", e))?;
        (JsonValueType::String, normalized)
    } else if let Ok(number) = serde_json::from_str::<serde_json::Number>(trimmed) {
        (JsonValueType::Number, number.to_string())
    } else {
        return Err(
            "Некорректный JSON-литерал. Допустимо: строка в кавычках, число, true/false или null"
                .to_string(),
        );
    };

    node.value_type = value_type;
    node.display_value = display_value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_json;

    fn leaf(value_type: JsonValueType, display_value: &str) -> JsonNode {
        JsonNode {
            key: None,
            value_type,
            display_value: display_value.to_string(),
            children: vec![],
            expanded: false,
            path: "root".to_string(),
        }
    }

    #[test]
    fn roundtrip_preserves_structure() {
        let source = r#"{"a":[1,2],"b":"x","c":null,"d":true}"#;
        let node = parse_json(source).unwrap();
        let value = node_to_value(&node).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), source);
    }

    #[test]
    fn edit_detects_literal_type() {
        let mut node = leaf(JsonValueType::Null, "null");

        apply_primitive_edit(&mut node, " 42 ").unwrap();
        assert_eq!(node.value_type, JsonValueType::Number);
        assert_eq!(node.display_value, "42");

        apply_primitive_edit(&mut node, "\"текст\"").unwrap();
        assert_eq!(node.value_type, JsonValueType::String);

        apply_primitive_edit(&mut node, "false").unwrap();
        assert_eq!(node.value_type, JsonValueType::Bool);
    }

    #[test]
    fn edit_rejects_invalid_literal() {
        let mut node = leaf(JsonValueType::String, "\"x\"");
        assert!(apply_primitive_edit(&mut node, "").is_err());
        assert!(apply_primitive_edit(&mut node, "нет кавычек").is_err());
        assert_eq!(node.display_value, "\"x\"");
    }
}
