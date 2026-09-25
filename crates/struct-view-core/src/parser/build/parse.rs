use serde::Deserialize;
use serde_json::Value;

use super::super::node::{JsonNode, ParseError};
use super::comments::{add_comment_nodes, extract_comments};
use super::format::DataFormat;
use super::tree::{build_node, build_toml_node, build_yaml_node};

/// Разобрать данные указанного формата или определить формат автоматически.
///
/// При успешном автодетекте возвращает фактически использованный формат.
///
/// # Errors
///
/// Возвращает [`ParseError`], если содержимое не соответствует указанному
/// формату или ни одному из поддерживаемых форматов при автодетекте.
pub fn parse_data(
    input: &str,
    format: Option<DataFormat>,
) -> Result<(JsonNode, DataFormat), ParseError> {
    if let Some(format) = format {
        let root = build_document(input, format)?;
        return Ok((root, format));
    }

    let mut errors = Vec::new();
    for candidate in [
        DataFormat::Json,
        DataFormat::Toml,
        DataFormat::Json5,
        DataFormat::Yaml,
    ] {
        match build_document(input, candidate) {
            Ok(root) => return Ok((root, candidate)),
            Err(error) => errors.push(format!("{}: {}", candidate, error.message)),
        }
    }

    Err(ParseError {
        message: format!(
            "Не удалось определить формат. Поддерживаются JSON, YAML, TOML и JSON5.\n{}",
            errors.join("\n")
        ),
        line: None,
        column: None,
    })
}

enum ParsedDocument {
    Json(Value),
    Yaml(serde_yaml_ng::Value),
    Toml(toml::Value),
}

fn build_document(input: &str, format: DataFormat) -> Result<JsonNode, ParseError> {
    let mut root = match parse_document(input, format)? {
        ParsedDocument::Json(value) => build_node(None, false, &value, String::new()),
        ParsedDocument::Yaml(value) => build_yaml_node(None, false, &value, String::new())?,
        ParsedDocument::Toml(value) => build_toml_node(None, false, &value, String::new()),
    };
    add_comment_nodes(&mut root, extract_comments(input, format));
    Ok(root)
}

fn parse_document(input: &str, format: DataFormat) -> Result<ParsedDocument, ParseError> {
    match format {
        DataFormat::Json => serde_json::from_str(input)
            .map(ParsedDocument::Json)
            .map_err(|error| ParseError {
                message: error.to_string(),
                line: Some(error.line()),
                column: Some(error.column()),
            }),
        DataFormat::Yaml => parse_yaml_documents(input).map(ParsedDocument::Yaml),
        DataFormat::Toml => toml::from_str(input)
            .map(ParsedDocument::Toml)
            .map_err(|error| ParseError {
                message: error.to_string(),
                line: None,
                column: None,
            }),
        DataFormat::Json5 => json5::from_str(input)
            .map(ParsedDocument::Json)
            .map_err(|error| ParseError {
                message: error.to_string(),
                line: None,
                column: None,
            }),
    }
}

/// Разобрать YAML-поток из одного или нескольких документов.
fn parse_yaml_documents(input: &str) -> Result<serde_yaml_ng::Value, ParseError> {
    let documents = serde_yaml_ng::Deserializer::from_str(input)
        .map(serde_yaml_ng::Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            let location = error.location();
            ParseError {
                message: error.to_string(),
                line: location.as_ref().map(serde_yaml_ng::Location::line),
                column: location.as_ref().map(serde_yaml_ng::Location::column),
            }
        })?;

    let mut documents = documents.into_iter();
    match (documents.next(), documents.next()) {
        (None, _) => Ok(serde_yaml_ng::Value::Null),
        (Some(document), None) => Ok(document),
        (Some(first), Some(second)) => Ok(serde_yaml_ng::Value::Sequence(
            std::iter::once(first)
                .chain(std::iter::once(second))
                .chain(documents)
                .collect(),
        )),
    }
}

/// Преобразовать YAML-значение в представление дерева.
///
/// JSON требует строковые ключи объектов, а YAML допускает значения любой
/// структуры. Составные YAML-ключи представляются компактной JSON-строкой.
/// Если это преобразование приводит к совпадающим ключам или теряет тип YAML,
/// документ отклоняется, а не преобразуется с потерей данных.
fn yaml_value_to_json(value: serde_yaml_ng::Value) -> Result<Value, ParseError> {
    match value {
        serde_yaml_ng::Value::Null => Ok(Value::Null),
        serde_yaml_ng::Value::Bool(value) => Ok(Value::Bool(value)),
        serde_yaml_ng::Value::Number(value) => yaml_number_to_json(value).map(Value::Number),
        serde_yaml_ng::Value::String(value) => Ok(Value::String(value)),
        serde_yaml_ng::Value::Sequence(values) => values
            .into_iter()
            .map(yaml_value_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        serde_yaml_ng::Value::Mapping(entries) => {
            let mut mapping = serde_json::Map::new();
            for (key, value) in entries {
                let key = yaml_key_to_string(key)?;
                if mapping.contains_key(&key) {
                    return Err(ParseError {
                        message: format!(
                            "Ключи YAML после преобразования в строки совпадают: {key}"
                        ),
                        line: None,
                        column: None,
                    });
                }
                mapping.insert(key, yaml_value_to_json(value)?);
            }
            Ok(Value::Object(mapping))
        }
        serde_yaml_ng::Value::Tagged(_) => Err(ParseError {
            message: "Явные теги YAML нельзя сохранить без потери их типа".to_string(),
            line: None,
            column: None,
        }),
    }
}

/// Создать отображаемое имя YAML-ключа, включая составные ключи.
pub(super) fn yaml_key_to_string(key: serde_yaml_ng::Value) -> Result<String, ParseError> {
    match key {
        serde_yaml_ng::Value::String(value) => Ok(value),
        other => serde_json::to_string(&yaml_value_to_json(other)?).map_err(|error| ParseError {
            message: format!("Не удалось преобразовать ключ YAML: {error}"),
            line: None,
            column: None,
        }),
    }
}

/// Сохранить целочисленное YAML-число без промежуточного `f64`.
pub(super) fn yaml_number_to_json(
    value: serde_yaml_ng::Number,
) -> Result<serde_json::Number, ParseError> {
    if let Some(value) = value.as_i64() {
        Ok(serde_json::Number::from(value))
    } else if let Some(value) = value.as_u64() {
        Ok(serde_json::Number::from(value))
    } else if value.is_f64() {
        value
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .ok_or_else(|| ParseError {
                message: "Числа YAML NaN и Infinity не поддерживаются JSON-представлением"
                    .to_string(),
                line: None,
                column: None,
            })
    } else {
        Err(ParseError {
            message: "Целое число YAML выходит за диапазон JSON".to_string(),
            line: None,
            column: None,
        })
    }
}

/// Разобрать строку с JSON-содержимым в дерево [`JsonNode`].
///
/// Возвращает корневой узел дерева или [`ParseError`] с указанием места ошибки.
///
/// # Errors
///
/// Возвращает [`ParseError`], если входная строка содержит синтаксическую
/// ошибку JSON. Поля `line` и `column` заполняются из информации `serde_json`.
///
/// # Examples
///
/// ```
/// use struct_view_core::parser::parse_json;
///
/// let node = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
/// assert_eq!(node.children.len(), 2);
/// ```
pub fn parse_json(input: &str) -> Result<JsonNode, ParseError> {
    parse_data(input, Some(DataFormat::Json)).map(|(node, _)| node)
}
