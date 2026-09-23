//! Конструктор значений, правка дерева и обратное преобразование
//! в [`serde_json::Value`].

use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use crate::clipboard::ClipboardEntry;
use crate::parser::{
    DataFormat, JsonNode, JsonValueType, build_path, node_to_value, parse_data, plural_ru,
};

/// Собрать выбранные узлы для копирования, не дублируя вложенные выборы.
///
/// Если одновременно выбраны контейнер и его потомок, в буфер попадает
/// только контейнер: его значение уже содержит всю вложенную иерархию.
pub(super) fn selected_structures(
    root: &JsonNode,
    selected_paths: &BTreeSet<String>,
) -> Result<Vec<ClipboardEntry>, String> {
    let mut entries = Vec::new();
    collect_selected_structures(root, selected_paths, &mut entries)?;
    if entries.is_empty() {
        return Err("Не выбрано ни одной структуры".to_string());
    }
    Ok(entries)
}

fn collect_selected_structures(
    node: &JsonNode,
    selected_paths: &BTreeSet<String>,
    entries: &mut Vec<ClipboardEntry>,
) -> Result<(), String> {
    if selected_paths.contains(&node.path) {
        entries.push(ClipboardEntry {
            key: node.key.clone(),
            value: node_to_value(node)?,
        });
        return Ok(());
    }

    for child in &node.children {
        collect_selected_structures(child, selected_paths, entries)?;
    }
    Ok(())
}

/// Вставить структуры в объект или массив по пути контейнера.
///
/// Ключи полей объектов сохраняются, а элементы массивов добавляются в конец
/// с новыми индексами. Неключевой объект (например, скопированный корень)
/// разворачивается в целевой объект своими полями; неключевой массив
/// разворачивается в целевой массив своими элементами.
pub(super) fn paste_structures_at_path(
    root: &mut JsonNode,
    target_path: &str,
    entries: &[ClipboardEntry],
) -> Result<usize, String> {
    if entries.is_empty() {
        return Err("Буфер структур пуст".to_string());
    }

    let parent = find_node_mut(root, target_path)
        .ok_or_else(|| "Не удалось найти контейнер для вставки данных".to_string())?;
    match parent.value_type {
        JsonValueType::Object => paste_into_object(parent, entries),
        JsonValueType::Array => paste_into_array(parent, entries),
        _ => Err("Вставлять структуры можно только в объект или массив".to_string()),
    }
}

fn paste_into_object(parent: &mut JsonNode, entries: &[ClipboardEntry]) -> Result<usize, String> {
    let mut candidates = Vec::new();
    for entry in entries {
        match (&entry.key, &entry.value) {
            (Some(key), value) => candidates.push((key.clone(), value.clone())),
            (None, Value::Object(fields)) => {
                candidates.extend(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
            }
            (None, _) => {
                return Err(
                    "Для вставки значения без ключа в объект скопируйте поле объекта".to_string(),
                );
            }
        }
    }
    if candidates.is_empty() {
        return Err("В буфере нет полей для вставки в объект".to_string());
    }

    let mut known_keys = parent
        .children
        .iter()
        .filter_map(|child| child.key.as_deref())
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    for (key, _) in &candidates {
        if !known_keys.insert(key.clone()) {
            return Err(format!("Поле «{}» уже существует", key));
        }
    }

    let parent_path = parent.path.clone();
    let nodes = candidates
        .into_iter()
        .map(|(key, value)| value_to_node(Some(key), false, &value, &parent_path))
        .collect::<Result<Vec<_>, _>>()?;
    let count = nodes.len();
    parent.children.extend(nodes);
    update_container_label(parent);
    Ok(count)
}

fn paste_into_array(parent: &mut JsonNode, entries: &[ClipboardEntry]) -> Result<usize, String> {
    let mut values = Vec::new();
    for entry in entries {
        if entry.key.is_none()
            && let Value::Array(items) = &entry.value
        {
            values.extend(items.iter().cloned());
            continue;
        }
        values.push(entry.value.clone());
    }
    if values.is_empty() {
        return Err("В буфере нет элементов для вставки в массив".to_string());
    }

    let parent_path = parent.path.clone();
    let first_index = parent.children.len();
    let nodes = values
        .into_iter()
        .enumerate()
        .map(|(offset, value)| {
            value_to_node(
                Some((first_index + offset).to_string()),
                true,
                &value,
                &parent_path,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = nodes.len();
    parent.children.extend(nodes);
    update_container_label(parent);
    Ok(count)
}

fn value_to_node(
    key: Option<String>,
    is_index: bool,
    value: &Value,
    parent_path: &str,
) -> Result<JsonNode, String> {
    let source = serde_json::to_string(value)
        .map_err(|error| format!("Ошибка подготовки структуры к вставке: {}", error))?;
    let (mut node, _) = parse_data(&source, Some(DataFormat::Json))
        .map_err(|error| format!("Не удалось подготовить структуру к вставке: {}", error))?;
    node.key = key;
    update_paths(&mut node, parent_path, is_index);
    Ok(node)
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
        let value_type = if number.is_f64() {
            JsonValueType::Float
        } else {
            JsonValueType::Number
        };
        (value_type, number.to_string())
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

/// Добавить поле или элемент с типом, выбранным в конструкторе значения.
pub(super) fn add_typed_child_at_path(
    root: &mut JsonNode,
    parent_path: &str,
    key: &str,
    value_type: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    let input = field_value_to_input(value_type, value)?;
    add_child_at_path(root, parent_path, key, &input, format)
}

/// Изменить существующее поле или элемент через конструктор значения.
pub(super) fn edit_child_at_path(
    root: &mut JsonNode,
    path: &str,
    new_key: Option<&str>,
    value_type: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    if path.is_empty() && format == DataFormat::Toml && !matches!(value_type, JsonValueType::Object)
    {
        return Err("Корневое значение TOML должно быть объектом".to_string());
    }

    let input = field_value_to_input(value_type, value)?;
    let replacement = parse_child_value(&input, format)?;

    let parent = find_parent(root, path);
    let is_index = parent.is_some_and(|parent| parent.value_type == JsonValueType::Array);

    if let Some(new_key) = new_key {
        let new_key = new_key.trim();
        if new_key.is_empty() {
            return Err("Имя поля не может быть пустым".to_string());
        }
        let parent =
            parent.ok_or_else(|| "Не удалось найти поле для редактирования".to_string())?;
        if parent.value_type != JsonValueType::Object {
            return Err("Имя можно изменить только у поля объекта".to_string());
        }
        if parent
            .children
            .iter()
            .any(|child| child.path != path && child.key.as_deref() == Some(new_key))
        {
            return Err(format!("Поле «{}» уже существует", new_key));
        }
    }

    if replace_node_at_path(root, path, &replacement, new_key, "", is_index) {
        Ok(())
    } else {
        Err("Не удалось найти поле для редактирования".to_string())
    }
}

/// Проверить, является ли узел полем объекта и поэтому допускает переименование.
pub(super) fn is_object_child(root: &JsonNode, path: &str) -> bool {
    find_parent(root, path).is_some_and(|parent| {
        parent.value_type == JsonValueType::Object
            && parent.children.iter().any(|child| child.path == path)
    })
}

/// Преобразовать значение из конструктора в JSON-совместимый литерал.
fn field_value_to_input(value_type: &JsonValueType, value: &str) -> Result<String, String> {
    match value_type {
        JsonValueType::String => serde_json::to_string(value)
            .map_err(|error| format!("Ошибка сериализации строки: {}", error)),
        JsonValueType::DateTime => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err("Дата/время TOML не может быть пустым".to_string());
            }
            Ok(trimmed.to_string())
        }
        JsonValueType::Number => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err("Число не может быть пустым".to_string());
            }
            let number = serde_json::from_str::<serde_json::Number>(trimmed)
                .map_err(|error| format!("Некорректное число: {}", error))?;
            Ok(number.to_string())
        }
        JsonValueType::Float => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err("Число не может быть пустым".to_string());
            }
            if let Ok(number) = serde_json::from_str::<serde_json::Number>(trimmed) {
                if number.is_f64() {
                    Ok(number.to_string())
                } else {
                    Ok(format!("{number}.0"))
                }
            } else if matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "nan" | "+nan" | "-nan" | "inf" | "+inf" | "-inf"
            ) {
                Ok(trimmed.to_ascii_lowercase())
            } else {
                Err("Некорректное вещественное число".to_string())
            }
        }
        JsonValueType::Bool => match value.trim() {
            "true" | "false" => Ok(value.trim().to_string()),
            _ => Err("Логическое значение должно быть true или false".to_string()),
        },
        JsonValueType::Null => Ok("null".to_string()),
        JsonValueType::Object => Ok("{}".to_string()),
        JsonValueType::Array => Ok("[]".to_string()),
    }
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
    let (key, is_index) = match parent.value_type {
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
            (Some(key.to_string()), false)
        }
        JsonValueType::Array => (Some(parent.children.len().to_string()), true),
        _ => return Err("Добавлять данные можно только в объект или массив".to_string()),
    };

    let mut child = parse_child_value(input, format)?;
    child.key = key;
    update_paths(&mut child, &parent.path, is_index);
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

fn find_parent<'a>(node: &'a JsonNode, path: &str) -> Option<&'a JsonNode> {
    if node.children.iter().any(|child| child.path == path) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_parent(child, path))
}

pub(super) fn find_node<'a>(node: &'a JsonNode, path: &str) -> Option<&'a JsonNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, path))
}

fn replace_node_at_path(
    node: &mut JsonNode,
    path: &str,
    replacement: &JsonNode,
    new_key: Option<&str>,
    parent_path: &str,
    is_index: bool,
) -> bool {
    if node.path == path {
        let mut updated = replacement.clone();
        updated.key = new_key
            .map(|key| key.trim().to_string())
            .or_else(|| node.key.clone());
        if matches!(
            (&node.value_type, &updated.value_type),
            (JsonValueType::Object, JsonValueType::Object)
                | (JsonValueType::Array, JsonValueType::Array)
        ) {
            updated.children = node.children.clone();
            updated.display_value = node.display_value.clone();
        }
        updated.expanded = node.expanded;
        update_paths(&mut updated, parent_path, is_index);
        *node = updated;
        return true;
    }

    let current_path = node.path.clone();
    for child in &mut node.children {
        if replace_node_at_path(child, path, replacement, new_key, &current_path, is_index) {
            update_container_label(node);
            return true;
        }
    }
    false
}

fn update_paths(node: &mut JsonNode, parent_path: &str, is_index: bool) {
    node.path = build_path(parent_path, &node.key, is_index);
    let path = node.path.clone();
    let child_is_index = node.value_type == JsonValueType::Array;
    for (index, child) in node.children.iter_mut().enumerate() {
        if child_is_index {
            child.key = Some(index.to_string());
        }
        update_paths(child, &path, child_is_index);
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
    fn roundtrip_escapes_control_characters_in_strings() {
        let source = r#"{"text":"line\n\u0000"}"#;
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
    fn typed_constructor_adds_values_without_format_literals() {
        for format in [
            DataFormat::Json,
            DataFormat::Yaml,
            DataFormat::Toml,
            DataFormat::Json5,
        ] {
            let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
            add_typed_child_at_path(&mut root, "", "name", &JsonValueType::String, "Ada", format)
                .unwrap();
            add_typed_child_at_path(&mut root, "", "age", &JsonValueType::Number, "37", format)
                .unwrap();
            add_typed_child_at_path(
                &mut root,
                "",
                "enabled",
                &JsonValueType::Bool,
                "false",
                format,
            )
            .unwrap();
            add_typed_child_at_path(&mut root, "", "profile", &JsonValueType::Object, "", format)
                .unwrap();
            add_typed_child_at_path(&mut root, "", "roles", &JsonValueType::Array, "", format)
                .unwrap();

            let value = node_to_value(&root).unwrap();
            assert_eq!(value["name"], "Ada");
            assert_eq!(value["age"], 37);
            assert_eq!(value["enabled"], false);
            assert_eq!(value["profile"], serde_json::json!({}));
            assert_eq!(value["roles"], serde_json::json!([]));
        }

        let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
        add_typed_child_at_path(
            &mut root,
            "",
            "missing",
            &JsonValueType::Null,
            "",
            DataFormat::Json,
        )
        .unwrap();
        assert_eq!(node_to_value(&root).unwrap()["missing"], Value::Null);
    }

    #[test]
    fn toml_datetime_fields_can_be_edited_without_becoming_strings() {
        let (mut root, _) =
            parse_data("created = 1979-05-27T07:32:00Z", Some(DataFormat::Toml)).unwrap();

        edit_child_at_path(
            &mut root,
            "created",
            None,
            &JsonValueType::DateTime,
            "1980-01-02T03:04:05Z",
            DataFormat::Toml,
        )
        .unwrap();

        assert_eq!(root.children[0].value_type, JsonValueType::DateTime);
        assert_eq!(
            node_to_value(&root).unwrap()["created"],
            "1980-01-02T03:04:05Z"
        );
        let serialized = crate::parser::serialize_node(&root, DataFormat::Toml, false).unwrap();
        assert!(serialized.contains("created = 1980-01-02T03:04:05Z"));
    }

    #[test]
    fn typed_toml_float_constructor_preserves_float_type() {
        let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
        add_typed_child_at_path(
            &mut root,
            "",
            "ratio",
            &JsonValueType::Float,
            "1",
            DataFormat::Toml,
        )
        .unwrap();

        assert_eq!(root.children[0].value_type, JsonValueType::Float);
        assert_eq!(node_to_value(&root).unwrap()["ratio"], 1.0);
        let serialized = crate::parser::serialize_node(&root, DataFormat::Toml, false).unwrap();
        assert!(serialized.contains("ratio = 1.0"));
    }

    #[test]
    fn typed_constructor_edits_type_renames_field_and_preserves_containers() {
        let (mut root, _) = parse_data(
            r#"{"profile":{"name":"Ada"},"value":1,"items":[1]}"#,
            Some(DataFormat::Json),
        )
        .unwrap();

        edit_child_at_path(
            &mut root,
            "value",
            Some("count"),
            &JsonValueType::Number,
            "2",
            DataFormat::Json,
        )
        .unwrap();
        edit_child_at_path(
            &mut root,
            "profile",
            None,
            &JsonValueType::Object,
            "",
            DataFormat::Json,
        )
        .unwrap();
        edit_child_at_path(
            &mut root,
            "items",
            None,
            &JsonValueType::Array,
            "",
            DataFormat::Json,
        )
        .unwrap();

        assert_eq!(
            node_to_value(&root).unwrap(),
            serde_json::json!({
                "profile": {"name": "Ada"},
                "count": 2,
                "items": [1]
            })
        );
        assert!(is_object_child(&root, "count"));
        assert_eq!(find_node(&root, "count").unwrap().path, "count");
    }

    #[test]
    fn typed_constructor_rejects_toml_null_values() {
        let (mut root, _) = parse_data("{}", Some(DataFormat::Json)).unwrap();
        let error = add_typed_child_at_path(
            &mut root,
            "",
            "missing",
            &JsonValueType::Null,
            "",
            DataFormat::Toml,
        )
        .unwrap_err();
        assert!(!error.is_empty());
        assert!(root.children.is_empty());
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

    #[test]
    fn selected_structures_keep_hierarchy_and_skip_nested_duplicates() {
        let root =
            parse_json(r#"{"profile":{"name":"Ada","roles":["admin"]},"enabled":true}"#).unwrap();
        let selected_paths = BTreeSet::from([
            "profile".to_string(),
            "profile.name".to_string(),
            "enabled".to_string(),
        ]);

        let entries = selected_structures(&root, &selected_paths).unwrap();

        assert_eq!(entries.len(), 2);
        let profile = entries
            .iter()
            .find(|entry| entry.key.as_deref() == Some("profile"))
            .unwrap();
        assert_eq!(
            profile.value,
            serde_json::json!({"name": "Ada", "roles": ["admin"]})
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.key.as_deref() == Some("enabled"))
        );
    }

    #[test]
    fn pasting_object_structures_preserves_nested_values_and_paths() {
        let source = parse_json(r#"{"profile":{"name":"Ada","roles":["admin"]}}"#).unwrap();
        let selected_paths = BTreeSet::from(["profile".to_string()]);
        let entries = selected_structures(&source, &selected_paths).unwrap();
        let mut target = parse_json(r#"{"existing":true}"#).unwrap();

        let inserted = paste_structures_at_path(&mut target, "", &entries).unwrap();

        assert_eq!(inserted, 1);
        assert_eq!(
            node_to_value(&target).unwrap(),
            serde_json::json!({
                "existing": true,
                "profile": {"name": "Ada", "roles": ["admin"]}
            })
        );
        assert_eq!(target.children[1].path, "profile");
        assert_eq!(target.children[1].children[0].path, "profile.name");
    }

    #[test]
    fn pasting_root_array_into_array_appends_its_elements() {
        let source = parse_json(r#"[{"id":1},{"id":2}]"#).unwrap();
        let selected_paths = BTreeSet::from(["".to_string()]);
        let entries = selected_structures(&source, &selected_paths).unwrap();
        let mut target = parse_json(r#"[{"id":0}]"#).unwrap();

        let inserted = paste_structures_at_path(&mut target, "", &entries).unwrap();

        assert_eq!(inserted, 2);
        assert_eq!(
            node_to_value(&target).unwrap(),
            serde_json::json!([{"id": 0}, {"id": 1}, {"id": 2}])
        );
        assert_eq!(target.children[2].path, "2");
    }

    #[test]
    fn duplicate_paste_does_not_partially_modify_object() {
        let entry = ClipboardEntry {
            key: Some("name".to_string()),
            value: Value::String("Grace".to_string()),
        };
        let mut target = parse_json(r#"{"name":"Ada"}"#).unwrap();

        let error = paste_structures_at_path(&mut target, "", &[entry]).unwrap_err();

        assert!(error.contains("уже существует"));
        assert_eq!(
            node_to_value(&target).unwrap(),
            serde_json::json!({"name": "Ada"})
        );
    }
}
