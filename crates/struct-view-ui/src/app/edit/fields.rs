use struct_view_core::parser::{
    DataFormat, JsonNode, JsonValueType, format_comment_for_format, parse_data,
};

use super::paths::{
    find_node_mut, find_parent, replace_node_at_path, update_container_label, update_paths,
};

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
pub(in crate::app) fn apply_primitive_edit(
    node: &mut JsonNode,
    edited: &str,
) -> Result<(), String> {
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
pub(in crate::app) fn add_typed_child_at_path(
    root: &mut JsonNode,
    parent_path: &str,
    key: &str,
    value_type: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    if *value_type == JsonValueType::Comment {
        return add_comment_child_at_path(root, parent_path, value, format);
    }

    let input = field_value_to_input(value_type, value)?;
    if *value_type != JsonValueType::Metadata {
        return add_child_at_path(root, parent_path, key, &input, format);
    }
    if format != DataFormat::Yaml {
        return Err("Metadata (YAML-теги) поддерживаются только в YAML".to_string());
    }

    let parent = find_node_mut(root, parent_path)
        .ok_or_else(|| "Не удалось найти контейнер для добавления данных".to_string())?;
    let (child_key, is_index) = child_key_for_insert(parent, key, false)?;
    let mut child = parse_child_value(&input, format)?;
    if child.value_type != JsonValueType::Metadata {
        return Err("Для Metadata введите YAML-тег, например «!custom value»".to_string());
    }

    child.key = child_key;
    update_paths(&mut child, &parent.path, is_index);
    parent.children.push(child);
    update_container_label(parent);
    Ok(())
}

fn add_comment_child_at_path(
    root: &mut JsonNode,
    parent_path: &str,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    let parent = find_node_mut(root, parent_path)
        .ok_or_else(|| "Не удалось найти контейнер для добавления данных".to_string())?;
    let display_value = format_comment_for_format(value, format)?;
    child_key_for_insert(parent, "", true)?;
    let mut child = JsonNode {
        key: None,
        value_type: JsonValueType::Comment,
        display_value,
        children: Vec::new(),
        expanded: false,
        path: String::new(),
    };
    child.path = next_comment_path(parent);
    parent.children.push(child);
    update_container_label(parent);
    Ok(())
}

/// Изменить существующее поле или элемент через конструктор значения.
pub(in crate::app) fn edit_child_at_path(
    root: &mut JsonNode,
    path: &str,
    new_key: Option<&str>,
    value_type: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    if *value_type == JsonValueType::Metadata && format != DataFormat::Yaml {
        return Err("Metadata (YAML-теги) поддерживаются только в YAML".to_string());
    }
    if *value_type == JsonValueType::Comment {
        if format == DataFormat::Json {
            return Err("JSON не поддерживает комментарии".to_string());
        }
        let node = find_node_mut(root, path)
            .ok_or_else(|| "Не удалось найти комментарий для редактирования".to_string())?;
        if node.value_type != JsonValueType::Comment {
            return Err("Редактировать комментарий можно только как комментарий".to_string());
        }
        node.display_value = format_comment_for_format(value, format)?;
        return Ok(());
    }

    if path.is_empty() && format == DataFormat::Toml && !matches!(value_type, JsonValueType::Object)
    {
        return Err("Корневое значение TOML должно быть объектом".to_string());
    }

    let input = field_value_to_input(value_type, value)?;
    let replacement = parse_child_value(&input, format)?;
    if *value_type == JsonValueType::Metadata && replacement.value_type != JsonValueType::Metadata {
        return Err("Для Metadata введите YAML-тег, например «!custom value»".to_string());
    }

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
pub(in crate::app) fn is_object_child(root: &JsonNode, path: &str) -> bool {
    find_parent(root, path).is_some_and(|parent| {
        parent.value_type == JsonValueType::Object
            && parent
                .children
                .iter()
                .any(|child| child.path == path && child.value_type != JsonValueType::Comment)
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
        JsonValueType::Comment | JsonValueType::Metadata => Ok(value.to_string()),
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
    let (key, is_index) = child_key_for_insert(parent, key, false)?;
    let mut child = parse_child_value(input, format)?;
    child.key = key;
    update_paths(&mut child, &parent.path, is_index);
    parent.children.push(child);
    update_container_label(parent);
    Ok(())
}

fn child_key_for_insert(
    parent: &JsonNode,
    key: &str,
    is_comment: bool,
) -> Result<(Option<String>, bool), String> {
    match parent.value_type {
        JsonValueType::Object => {
            if is_comment {
                return Ok((None, false));
            }
            let key = key.trim();
            if key.is_empty() {
                return Err("Имя поля не может быть пустым".to_string());
            }
            if parent.children.iter().any(|child| {
                child.value_type != JsonValueType::Comment && child.key.as_deref() == Some(key)
            }) {
                return Err(format!("Поле «{}» уже существует", key));
            }
            Ok((Some(key.to_string()), false))
        }
        JsonValueType::Array => {
            if is_comment {
                return Ok((None, true));
            }
            let index = parent
                .children
                .iter()
                .filter(|child| child.value_type != JsonValueType::Comment)
                .count();
            Ok((Some(index.to_string()), true))
        }
        _ => Err("Добавлять данные можно только в объект или массив".to_string()),
    }
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

fn next_comment_path(parent: &JsonNode) -> String {
    let index = parent
        .children
        .iter()
        .filter(|child| child.value_type == JsonValueType::Comment)
        .count();
    format!("{}::comment[{index}]", parent.path)
}
