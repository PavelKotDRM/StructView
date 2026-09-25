use serde_json::Value;

use super::super::node::{JsonNode, JsonValueType};
use super::comments::{collect_comments, format_comment};
use super::format::DataFormat;

/// Сериализовать значение в выбранном формате.
///
/// # Errors
///
/// Возвращает ошибку, если значение нельзя представить в выбранном формате.
pub fn serialize_data(value: &Value, format: DataFormat, compact: bool) -> Result<String, String> {
    match format {
        DataFormat::Json | DataFormat::Json5 => if compact {
            serde_json::to_string(value)
        } else {
            serde_json::to_string_pretty(value)
        }
        .map_err(|error| format!("Ошибка сериализации {}: {}", format, error)),
        DataFormat::Yaml => serde_yaml_ng::to_string(value)
            .map_err(|error| format!("Ошибка сериализации YAML: {}", error)),
        DataFormat::Toml => if compact {
            toml::to_string(value)
        } else {
            toml::to_string_pretty(value)
        }
        .map_err(|error| format!("Ошибка сериализации TOML: {}", error)),
    }
}

/// Сериализовать дерево, сохраняя собственные типы TOML, например даты и время.
///
/// JSON-представления TOML-даты сериализуются как строки; при записи в TOML
/// исходное значение сохраняется как незакавыченный datetime. Комментарии
/// поддерживаемых форматов записываются отдельными строками в начало файла,
/// а YAML-теги сохраняются только при сериализации в YAML.
pub fn serialize_node(
    node: &JsonNode,
    format: DataFormat,
    compact: bool,
) -> Result<String, String> {
    if format != DataFormat::Yaml && contains_metadata(node) {
        return Err(format!("YAML-теги нельзя сохранить в формате {format}"));
    }

    let mut output = match format {
        DataFormat::Yaml => serde_yaml_ng::to_string(&node_to_yaml_value(node)?)
            .map_err(|error| format!("Ошибка сериализации YAML: {error}"))?,
        DataFormat::Toml => {
            let value = node_to_toml(node)?;
            if !matches!(value, toml::Value::Table(_)) {
                return Err("Корневое значение TOML должно быть таблицей".to_string());
            }
            if compact {
                toml::to_string(&value)
            } else {
                toml::to_string_pretty(&value)
            }
            .map_err(|error| format!("Ошибка сериализации TOML: {error}"))?
        }
        DataFormat::Json | DataFormat::Json5 => {
            serialize_data(&node_to_value(node)?, format, compact)?
        }
    };

    if format != DataFormat::Json {
        let comments = collect_comments(node);
        if !comments.is_empty() {
            let marker = if format == DataFormat::Json5 {
                "//"
            } else {
                "#"
            };
            let prefix = comments
                .iter()
                .map(|comment| format_comment(comment, marker))
                .collect::<Vec<_>>()
                .join("\n");
            output = format!("{prefix}\n{output}");
        }
    }

    Ok(output)
}

/// Преобразовать узел дерева в JSON-совместимое значение.
///
/// TOML-дата и время представлены строкой при сравнении, копировании и
/// сериализации в форматы без собственного типа datetime. Комментарии
/// исключаются из JSON-совместимого значения, а YAML-теги разворачиваются до
/// содержащегося в них значения.
pub fn node_to_value(node: &JsonNode) -> Result<Value, String> {
    match node.value_type {
        JsonValueType::Object => {
            let mut map = serde_json::Map::new();
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let key = child.key.clone().unwrap_or_default();
                map.insert(key, node_to_value(child)?);
            }
            Ok(Value::Object(map))
        }
        JsonValueType::Array => {
            let mut values = Vec::with_capacity(data_child_count(node));
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

fn metadata_value(node: &JsonNode) -> Result<&JsonNode, String> {
    let mut values = node
        .children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment);
    let value = values
        .next()
        .ok_or_else(|| format!("У YAML-тега в {} отсутствует значение", node.path))?;
    if values.next().is_some() {
        return Err(format!(
            "У YAML-тега в {} должно быть ровно одно значение",
            node.path
        ));
    }
    Ok(value)
}

fn node_to_toml(node: &JsonNode) -> Result<toml::Value, String> {
    match node.value_type {
        JsonValueType::Object => {
            let mut table = toml::map::Map::new();
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let key = child
                    .key
                    .clone()
                    .ok_or_else(|| format!("Отсутствует ключ TOML в {}", child.path))?;
                if table.insert(key.clone(), node_to_toml(child)?).is_some() {
                    return Err(format!("Ключ TOML «{key}» повторяется в {}", node.path));
                }
            }
            Ok(toml::Value::Table(table))
        }
        JsonValueType::Array => node
            .children
            .iter()
            .filter(|child| child.value_type != JsonValueType::Comment)
            .map(node_to_toml)
            .collect::<Result<Vec<_>, _>>()
            .map(toml::Value::Array),
        JsonValueType::Metadata => node_to_toml(metadata_value(node)?),
        JsonValueType::Comment => Err(format!(
            "Комментарий в {} не является значением TOML",
            node.path
        )),
        JsonValueType::String => serde_json::from_str::<String>(&node.display_value)
            .map(toml::Value::String)
            .map_err(|error| format!("Некорректная строка в {}: {error}", node.path)),
        JsonValueType::DateTime => node
            .display_value
            .parse::<toml::value::Datetime>()
            .map(toml::Value::Datetime)
            .map_err(|error| format!("Некорректная дата/время в {}: {error}", node.path)),
        JsonValueType::Number => {
            if let Ok(number) = serde_json::from_str::<serde_json::Number>(&node.display_value) {
                if let Some(integer) = number.as_i64() {
                    return Ok(toml::Value::Integer(integer));
                }
                if let Some(unsigned) = number.as_u64() {
                    let integer = i64::try_from(unsigned).map_err(|_| {
                        format!("Число в {} выходит за диапазон TOML Integer", node.path)
                    })?;
                    return Ok(toml::Value::Integer(integer));
                }
                if let Some(float) = number.as_f64() {
                    return Ok(toml::Value::Float(float));
                }
            }

            node.display_value
                .parse::<f64>()
                .map(toml::Value::Float)
                .map_err(|error| format!("Некорректное число в {}: {error}", node.path))
        }
        JsonValueType::Float => node
            .display_value
            .parse::<f64>()
            .map(toml::Value::Float)
            .map_err(|error| format!("Некорректное число в {}: {error}", node.path)),
        JsonValueType::Bool => node
            .display_value
            .parse::<bool>()
            .map(toml::Value::Boolean)
            .map_err(|error| format!("Некорректное bool в {}: {error}", node.path)),
        JsonValueType::Null => Err(format!(
            "TOML не поддерживает null-значения (путь {})",
            node.path
        )),
    }
}

fn node_to_yaml_value(node: &JsonNode) -> Result<serde_yaml_ng::Value, String> {
    use serde_yaml_ng::Value as YamlValue;

    match node.value_type {
        JsonValueType::Object => {
            let mut mapping = serde_yaml_ng::Mapping::new();
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let key = child
                    .key
                    .clone()
                    .ok_or_else(|| format!("Отсутствует ключ YAML в {}", child.path))?;
                mapping.insert(YamlValue::String(key), node_to_yaml_value(child)?);
            }
            Ok(YamlValue::Mapping(mapping))
        }
        JsonValueType::Array => node
            .children
            .iter()
            .filter(|child| child.value_type != JsonValueType::Comment)
            .map(node_to_yaml_value)
            .collect::<Result<Vec<_>, _>>()
            .map(YamlValue::Sequence),
        JsonValueType::Metadata => {
            if node.display_value.trim().is_empty() {
                return Err(format!("Пустой YAML-тег в {}", node.path));
            }
            Ok(YamlValue::Tagged(Box::new(
                serde_yaml_ng::value::TaggedValue {
                    tag: serde_yaml_ng::value::Tag::new(node.display_value.clone()),
                    value: node_to_yaml_value(metadata_value(node)?)?,
                },
            )))
        }
        JsonValueType::Comment => Err(format!(
            "Комментарий в {} не является значением YAML",
            node.path
        )),
        JsonValueType::String => serde_json::from_str::<String>(&node.display_value)
            .map(YamlValue::String)
            .map_err(|error| format!("Некорректная строка в {}: {error}", node.path)),
        JsonValueType::DateTime => Ok(YamlValue::String(node.display_value.clone())),
        JsonValueType::Number | JsonValueType::Float => {
            let number = serde_json::from_str::<serde_json::Number>(&node.display_value)
                .map_err(|error| format!("Некорректное число в {}: {error}", node.path))?;
            serde_yaml_ng::to_value(number)
                .map_err(|error| format!("Ошибка преобразования числа YAML: {error}"))
        }
        JsonValueType::Bool => node
            .display_value
            .parse::<bool>()
            .map(YamlValue::Bool)
            .map_err(|error| format!("Некорректное bool в {}: {error}", node.path)),
        JsonValueType::Null => Ok(YamlValue::Null),
    }
}

fn contains_metadata(node: &JsonNode) -> bool {
    node.value_type == JsonValueType::Metadata || node.children.iter().any(contains_metadata)
}

fn data_child_count(node: &JsonNode) -> usize {
    node.children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment)
        .count()
}
