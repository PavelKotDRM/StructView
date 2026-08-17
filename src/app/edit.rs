//! Правка примитивных значений дерева и обратное преобразование в [`serde_json::Value`].

use serde_json::Value;

use crate::parser::{DataFormat, JsonNode, JsonValueType, build_path, parse_data, plural_ru};

/// Преобразовать JSON-узел обратно в [`serde_json::Value`].
///
/// Используется при сохранении файла: дерево хранит значения как строки,
/// поэтому каждый лист заново разбирается в типизированное значение.
///
/// # Errors
///
/// Возвращает описание ошибки с путём узла, если отображаемое значение
/// не является корректным JSON-литералом ожидаемого типа.
pub(crate) fn node_to_value(node: &JsonNode) -> Result<Value, String> {
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

/// Добавить поле в объект или элемент в массив.
///
/// Значение разбирается синтаксисом открытого формата. Для TOML значение
/// временно оборачивается в поле, поскольку TOML не допускает корневые
/// скаляры.
pub(super) fn add_child(
    parent: &mut JsonNode,
    key: &str,
    input: &str,
    format: DataFormat,
) -> Result<(), String> {
    let key = match parent.value_type {
        JsonValueType::Object => {
            let key = key.trim();
            if key.is_empty() {
                return Err("Имя поля не может быть пустым".to_string());
            }
            if parent
                .children
                .iter()
                .any(|child| child.key.as_deref() == Some(key))
            {
                return Err(format!("Поле «{}» уже существует", key));
            }
            Some(key.to_string())
        }
        JsonValueType::Array => Some(parent.children.len().to_string()),
        _ => return Err("Добавлять данные можно только в объект или массив".to_string()),
    };

    let mut child = parse_child_value(input, format)?;
    child.key = key;
    update_paths(&mut child, &parent.path);
    parent.children.push(child);
    update_container_label(parent);
    Ok(())
}

/// Найти узел по пути и добавить в него дочерний узел.
pub(super) fn add_child_at_path(
    root: &mut JsonNode,
    parent_path: &str,
    key: &str,
    input: &str,
    format: DataFormat,
) -> Result<(), String> {
    let parent = find_node_mut(root, parent_path)
        .ok_or_else(|| "Не удалось найти контейнер для добавления данных".to_string())?;
    add_child(parent, key, input, format)
}

fn parse_child_value(input: &str, format: DataFormat) -> Result<JsonNode, String> {
    let source = match format {
        DataFormat::Toml => format!("value = {}", input),
        _ => input.to_string(),
    };
    let (root, _) = parse_data(&source, Some(format)).map_err(|error| error.to_string())?;

    if format == DataFormat::Toml {
        root.children
            .into_iter()
            .next()
            .ok_or_else(|| "TOML-значение не содержит данных".to_string())
    } else {
        Ok(root)
    }
}

fn find_node_mut<'a>(node: &'a mut JsonNode, path: &str) -> Option<&'a mut JsonNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, path))
}

fn update_paths(node: &mut JsonNode, parent_path: &str) {
    node.path = build_path(parent_path, &node.key);
    let path = node.path.clone();
    for (index, child) in node.children.iter_mut().enumerate() {
        if node.value_type == JsonValueType::Array {
            child.key = Some(index.to_string());
        }
        update_paths(child, &path);
    }
}

fn update_container_label(node: &mut JsonNode) {
    let count = node.children.len();
    node.display_value = match node.value_type {
        JsonValueType::Object => format!(
            "{{{}}} {}",
            count,
            plural_ru(count, "поле", "поля", "полей")
        ),
        JsonValueType::Array => format!(
            "[{}] {}",
            count,
            plural_ru(count, "элемент", "элемента", "элементов")
        ),
        _ => return,
    };
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

    #[test]
    fn adds_object_fields_and_array_elements_for_all_formats() {
        let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
        add_child(&mut root, "enabled", "true", DataFormat::Json).unwrap();
        add_child(&mut root, "profile", "name: Ada", DataFormat::Yaml).unwrap();
        add_child(&mut root, "server", "{ port = 8080 }", DataFormat::Toml).unwrap();
        add_child(&mut root, "items", "[1, 2,]", DataFormat::Json5).unwrap();

        assert_eq!(root.children.len(), 4);
        assert_eq!(root.children[0].path, "enabled");
        assert_eq!(root.children[1].children[0].key.as_deref(), Some("name"));
        assert_eq!(root.children[2].children[0].key.as_deref(), Some("port"));
        assert_eq!(root.children[3].children.len(), 2);

        let mut array = parse_data("[]", Some(DataFormat::Json)).unwrap().0;
        add_child(&mut array, "", "\"first\"", DataFormat::Json).unwrap();
        assert_eq!(array.children[0].key.as_deref(), Some("0"));
        assert_eq!(array.children[0].path, "0");
    }

    #[test]
    fn rejects_duplicate_object_field() {
        let (mut root, _) = parse_data(r#"{"name":"Ada"}"#, Some(DataFormat::Json)).unwrap();
        let error = add_child(&mut root, "name", "\"Grace\"", DataFormat::Json).unwrap_err();
        assert!(error.contains("уже существует"));
    }
}
