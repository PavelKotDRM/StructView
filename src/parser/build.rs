//! Построение [`JsonNode`]-дерева из структурированных текстовых форматов.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::node::{JsonNode, JsonValueType, ParseError};

/// Поддерживаемый формат структурированных данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// JavaScript Object Notation.
    Json,
    /// YAML Ain't Markup Language.
    Yaml,
    /// Tom's Obvious Minimal Language.
    Toml,
    /// Расширенный синтаксис JSON с комментариями и trailing commas.
    Json5,
}

impl DataFormat {
    /// Все форматы, доступные для преобразования и сохранения.
    pub const ALL: [Self; 4] = [Self::Json, Self::Yaml, Self::Toml, Self::Json5];

    /// Определить формат по расширению пути.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "json5" => Some(Self::Json5),
            _ => None,
        }
    }

    /// Основное расширение файла без точки.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json5 => "json5",
        }
    }

    /// Расширения, принимаемые диалогом выбора файла для этого формата.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Json => &["json"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Json5 => &["json5"],
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Json5 => "JSON5",
        })
    }
}

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
pub(crate) fn node_to_value(node: &JsonNode) -> Result<Value, String> {
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

fn extract_comments(input: &str, format: DataFormat) -> Vec<String> {
    match format {
        DataFormat::Json => Vec::new(),
        DataFormat::Json5 => extract_json5_comments(input),
        DataFormat::Yaml | DataFormat::Toml => extract_hash_comments(input, format),
    }
}

fn extract_json5_comments(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut comments = Vec::new();
    let mut quote = None;
    let mut index = 0;

    while index < bytes.len() {
        if let Some(quote_byte) = quote {
            if bytes[index] == b'\\' {
                index = (index + 2).min(bytes.len());
            } else if bytes[index] == quote_byte {
                quote = None;
                index += 1;
            } else {
                index += 1;
            }
            continue;
        }

        match bytes[index] {
            b'"' | b'\'' => {
                quote = Some(bytes[index]);
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let start = index;
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                comments.push(input[start..index].trim_end().to_string());
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let start = index;
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                if index + 1 < bytes.len() {
                    index += 2;
                    comments.push(input[start..index].to_string());
                }
            }
            _ => index += 1,
        }
    }

    comments
}

#[derive(Clone, Copy)]
enum HashQuote {
    Single,
    Double,
    MultilineSingle,
    MultilineDouble,
}

fn extract_hash_comments(input: &str, format: DataFormat) -> Vec<String> {
    let mut comments = Vec::new();
    let mut quote = None;
    let mut yaml_block_indent = None;

    for line in input.lines() {
        let bytes = line.as_bytes();
        let indent = bytes.iter().take_while(|byte| **byte == b' ').count();
        if format == DataFormat::Yaml
            && let Some(block_indent) = yaml_block_indent
        {
            if line.trim().is_empty() {
                continue;
            }
            if indent > block_indent {
                continue;
            }
            yaml_block_indent = None;
        }

        let mut index = 0;
        let mut code_end = bytes.len();
        while index < bytes.len() {
            if let Some(active_quote) = quote {
                match active_quote {
                    HashQuote::Single => {
                        if bytes[index] == b'\'' {
                            if bytes.get(index + 1) == Some(&b'\'') {
                                index += 2;
                            } else {
                                quote = None;
                                index += 1;
                            }
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::Double => {
                        if bytes[index] == b'\\' {
                            index = (index + 2).min(bytes.len());
                        } else if bytes[index] == b'"' {
                            quote = None;
                            index += 1;
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineSingle => {
                        if bytes[index..].starts_with(b"'''") {
                            quote = None;
                            index += 3;
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineDouble => {
                        if bytes[index] == b'\\' {
                            index = (index + 2).min(bytes.len());
                        } else if bytes[index..].starts_with(b"\"\"\"") {
                            quote = None;
                            index += 3;
                        } else {
                            index += 1;
                        }
                    }
                }
                continue;
            }

            if bytes[index] == b'#'
                && (format == DataFormat::Toml
                    || index == 0
                    || bytes[index - 1].is_ascii_whitespace())
            {
                comments.push(line[index..].trim_end().to_string());
                code_end = index;
                break;
            }

            if format == DataFormat::Toml && bytes[index..].starts_with(b"'''") {
                quote = Some(HashQuote::MultilineSingle);
                index += 3;
            } else if format == DataFormat::Toml && bytes[index..].starts_with(b"\"\"\"") {
                quote = Some(HashQuote::MultilineDouble);
                index += 3;
            } else {
                match bytes[index] {
                    b'\'' => {
                        quote = Some(HashQuote::Single);
                        index += 1;
                    }
                    b'"' => {
                        quote = Some(HashQuote::Double);
                        index += 1;
                    }
                    _ => index += 1,
                }
            }
        }

        if format == DataFormat::Yaml && yaml_has_block_scalar_indicator(&line[..code_end]) {
            yaml_block_indent = Some(indent);
        }
    }

    comments
}

fn yaml_has_block_scalar_indicator(line: &str) -> bool {
    let line = line.trim_end();
    let bytes = line.as_bytes();
    for index in (0..bytes.len()).rev() {
        if !matches!(bytes[index], b'|' | b'>')
            || (index > 0 && !bytes[index - 1].is_ascii_whitespace())
        {
            continue;
        }
        if bytes[index + 1..]
            .iter()
            .all(|byte| matches!(byte, b'+' | b'-' | b'1'..=b'9'))
        {
            return true;
        }
    }
    false
}

fn add_comment_nodes(root: &mut JsonNode, comments: Vec<String>) {
    if comments.is_empty() {
        return;
    }

    let comment_nodes = comments
        .into_iter()
        .enumerate()
        .map(|(index, comment)| JsonNode {
            key: None,
            value_type: JsonValueType::Comment,
            display_value: comment,
            children: Vec::new(),
            expanded: false,
            path: format!("{}::comment[{index}]", root.path),
        })
        .collect::<Vec<_>>();
    root.children.splice(0..0, comment_nodes);
}

fn collect_comments(node: &JsonNode) -> Vec<String> {
    let mut comments = Vec::new();
    collect_comment_nodes(node, &mut comments);
    comments
}

fn collect_comment_nodes(node: &JsonNode, comments: &mut Vec<String>) {
    if node.value_type == JsonValueType::Comment {
        comments.push(node.display_value.clone());
        return;
    }
    for child in &node.children {
        collect_comment_nodes(child, comments);
    }
}

pub(crate) fn comment_input(comment: &str) -> String {
    let comment = comment.trim();
    if let Some(body) = comment
        .strip_prefix("/*")
        .and_then(|comment| comment.strip_suffix("*/"))
    {
        return body.trim().to_string();
    }

    comment
        .lines()
        .map(|line| {
            let indentation = &line[..line.len() - line.trim_start().len()];
            let content = line.trim_start();
            let content = content
                .strip_prefix("//")
                .or_else(|| content.strip_prefix('#'))
                .map(|content| content.strip_prefix(' ').unwrap_or(content));
            match content {
                Some(content) => format!("{indentation}{content}"),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn format_comment_for_format(
    comment: &str,
    format: DataFormat,
) -> Result<String, String> {
    let marker = match format {
        DataFormat::Json5 => "//",
        DataFormat::Yaml | DataFormat::Toml => "#",
        DataFormat::Json => {
            return Err("JSON не поддерживает комментарии".to_string());
        }
    };
    Ok(format_comment(comment, marker))
}

fn format_comment(comment: &str, marker: &str) -> String {
    let body = comment_input(comment);
    if body.is_empty() {
        return marker.to_string();
    }
    body.lines()
        .map(|line| {
            if line.is_empty() {
                marker.to_string()
            } else {
                format!("{marker} {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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
fn yaml_key_to_string(key: serde_yaml_ng::Value) -> Result<String, ParseError> {
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
fn yaml_number_to_json(value: serde_yaml_ng::Number) -> Result<serde_json::Number, ParseError> {
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
/// use struct_view::parser::parse_json;
///
/// let node = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
/// assert_eq!(node.children.len(), 2);
/// ```
pub fn parse_json(input: &str) -> Result<JsonNode, ParseError> {
    parse_data(input, Some(DataFormat::Json)).map(|(node, _)| node)
}

/// Рекурсивно построить [`JsonNode`] из [`serde_json::Value`].
///
/// # Arguments
///
/// * `key` — ключ текущего узла (имя поля или индекс массива).
/// * `is_index` — `true`, если `key` — индекс родительского массива, а не
///   имя поля родительского объекта.
/// * `value` — разобранное значение JSON.
/// * `parent_path` — путь родительского узла.
fn build_node(key: Option<String>, is_index: bool, value: &Value, parent_path: String) -> JsonNode {
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

fn build_yaml_node(
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

fn build_toml_node(
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

/// Построить путь к узлу из пути родителя и ключа текущего узла.
///
/// Индексы массивов оборачиваются в квадратные скобки: `arr[0]`. Ключи
/// объектов, являющиеся простым идентификатором, разделяются точкой:
/// `obj.field`. Остальные ключи объектов (пустая строка, число, содержащие
/// `.`/`[`/`]` и т. п.) оборачиваются в квадратные скобки с JSON-строкой:
/// `obj["0"]`, `obj["a.b"]`. Это исключает совпадение пути поля объекта
/// с путём индекса массива или с путём вложенного объекта.
///
/// `is_index` указывает, что `key` — индекс контейнера-массива, а не имя
/// поля объекта; вызывающий код определяет это по типу родительского узла,
/// а не по содержимому строки ключа.
///
/// # Examples
///
/// ```
/// use struct_view::parser::build_path;
///
/// assert_eq!(build_path("store.book", &Some("2".to_string()), true), "store.book[2]");
/// assert_eq!(build_path("store", &Some("title".to_string()), false), "store.title");
/// assert_eq!(build_path("", &Some("root".to_string()), false), "root");
/// assert_eq!(build_path("root", &None, false), "root");
/// // Ключ объекта, совпадающий по написанию с индексом массива, экранируется.
/// assert_eq!(build_path("store", &Some("0".to_string()), false), "store[\"0\"]");
/// ```
pub fn build_path(parent: &str, key: &Option<String>, is_index: bool) -> String {
    match key {
        None => parent.to_string(),
        Some(k) => {
            if is_index {
                if parent.is_empty() {
                    k.clone()
                } else {
                    format!("{}[{}]", parent, k)
                }
            } else {
                object_path_segment(parent, k)
            }
        }
    }
}

/// Построить путь к полю объекта, экранируя ключи, которые иначе были бы
/// неотличимы от индекса массива или от разделителя вложенности.
fn object_path_segment(parent: &str, key: &str) -> String {
    if is_plain_identifier(key) {
        if parent.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", parent, key)
        }
    } else {
        let quoted = serde_json::to_string(key)
            .unwrap_or_else(|error| format!("\"<key serialization error: {error}>\""));
        if parent.is_empty() {
            format!("[{}]", quoted)
        } else {
            format!("{}[{}]", parent, quoted)
        }
    }
}

/// Проверить, что ключ можно безопасно записать через точку без экранирования.
///
/// Ключ должен начинаться с буквы или `_` и состоять только из букв, цифр и
/// `_`. Это, в частности, отсекает ключи, которые целиком состоят из цифр
/// (совпадают по написанию с индексом массива) и ключи с `.`, `[`, `]`.
fn is_plain_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

/// Вспомогательная функция для русских форм множественного числа.
///
/// Возвращает нужную форму слова в зависимости от числа `n`.
///
/// # Examples
///
/// ```
/// use struct_view::parser::plural_ru;
///
/// assert_eq!(plural_ru(1, "поле", "поля", "полей"), "поле");
/// assert_eq!(plural_ru(3, "поле", "поля", "полей"), "поля");
/// assert_eq!(plural_ru(11, "поле", "поля", "полей"), "полей");
/// ```
pub fn plural_ru<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let rem100 = n % 100;
    let rem10 = n % 10;
    if (11..=19).contains(&rem100) {
        many
    } else if rem10 == 1 {
        one
    } else if (2..=4).contains(&rem10) {
        few
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml_array() {
        let (root, format) = parse_data("- one\n- two\n", Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(root.value_type, JsonValueType::Array);
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn parsed_containers_are_collapsed_by_default() {
        let root = parse_json(r#"{"object":{"value":1},"array":[{"value":2}]}"#).unwrap();

        assert!(!root.expanded);
        assert!(!root.children[0].expanded);
        assert!(!root.children[1].expanded);
        assert!(!root.children[1].children[0].expanded);
    }

    #[test]
    fn parses_toml_table() {
        let (root, format) = parse_data("[server]\nport = 8080\n", None).unwrap();
        assert_eq!(format, DataFormat::Toml);
        assert_eq!(root.children[0].key.as_deref(), Some("server"));
    }

    #[test]
    fn parses_json5_comments_and_trailing_comma() {
        let (root, format) = parse_data("{ // comment\n key: true,\n}", None).unwrap();
        assert_eq!(format, DataFormat::Json5);
        assert_eq!(root.children[0].value_type, JsonValueType::Comment);
        assert_eq!(root.children[0].display_value, "// comment");
        assert_eq!(root.children[1].key.as_deref(), Some("key"));
    }

    #[test]
    fn comments_roundtrip_in_json5_yaml_and_toml_without_becoming_data() {
        let cases = [
            (
                DataFormat::Json5,
                r#"{ value: "// inside string", // keep this
                    count: 2 }"#,
                "// keep this",
                1,
            ),
            (
                DataFormat::Yaml,
                " # document note\nquoted: '# not a comment'\nblock: |\n  # block text\ncount: 2 # inline note\n",
                "# document note",
                2,
            ),
            (
                DataFormat::Toml,
                "# document note\nquoted = \"# not a comment\" # inline note\ntext = \"\"\"\n# multiline string text\n\"\"\"\n",
                "# document note",
                2,
            ),
        ];

        for (format, source, expected_header, expected_count) in cases {
            let (root, _) = parse_data(source, Some(format)).unwrap();
            let comments = collect_comments(&root);
            assert_eq!(comments.len(), expected_count, "{format}");
            assert_eq!(comments[0], expected_header);

            let value = node_to_value(&root).unwrap();
            let output = serialize_node(&root, format, false).unwrap();
            assert!(output.starts_with(&format_comment(
                expected_header,
                if format == DataFormat::Json5 {
                    "//"
                } else {
                    "#"
                }
            )));
            let (reparsed, _) = parse_data(&output, Some(format)).unwrap();
            assert_eq!(node_to_value(&reparsed).unwrap(), value);
            assert_eq!(collect_comments(&reparsed).len(), expected_count);
        }
    }

    #[test]
    fn yaml_tags_are_metadata_nodes_and_roundtrip_only_in_yaml() {
        let (root, format) = parse_data("secret: !custom hello\n", Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);

        let metadata = root
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some("secret"))
            .unwrap();
        assert_eq!(metadata.value_type, JsonValueType::Metadata);
        assert_eq!(metadata.display_value, "!custom");
        assert_eq!(metadata.children[0].value_type, JsonValueType::String);
        assert_eq!(node_to_value(&root).unwrap()["secret"], "hello");

        let output = serialize_node(&root, DataFormat::Yaml, false).unwrap();
        assert!(output.contains("!custom"));
        let reparsed = parse_data(&output, Some(DataFormat::Yaml)).unwrap().0;
        assert_eq!(
            reparsed
                .children
                .iter()
                .find(|child| child.key.as_deref() == Some("secret"))
                .unwrap()
                .value_type,
            JsonValueType::Metadata
        );

        let error = serialize_node(&root, DataFormat::Json, false).unwrap_err();
        assert!(error.contains("YAML-теги"));
    }

    #[test]
    fn object_keys_that_look_like_array_indices_are_escaped_in_the_path() {
        // Объект с числовым ключом "0" не должен получить тот же path, что и
        // первый элемент массива (`arr[0]`), иначе find_node/selected_paths
        // не смогут различить эти узлы.
        let root = parse_json(r#"{"0": "object field"}"#).unwrap();
        assert_eq!(root.children[0].key.as_deref(), Some("0"));
        assert_eq!(root.children[0].path, "[\"0\"]");

        let array_root = parse_json(r#"["array element"]"#).unwrap();
        assert_eq!(array_root.children[0].path, "0");
        assert_ne!(root.children[0].path, array_root.children[0].path);
    }

    #[test]
    fn object_keys_with_dots_or_brackets_are_escaped_in_the_path() {
        let root = parse_json(r#"{"a": {"b.c": 1, "d[e]": 2}}"#).unwrap();
        let nested = &root.children[0];
        assert_eq!(nested.path, "a");
        let b_c = nested
            .children
            .iter()
            .find(|c| c.key.as_deref() == Some("b.c"))
            .unwrap();
        assert_eq!(b_c.path, "a[\"b.c\"]");
        let d_e = nested
            .children
            .iter()
            .find(|c| c.key.as_deref() == Some("d[e]"))
            .unwrap();
        assert_eq!(d_e.path, "a[\"d[e]\"]");
    }

    #[test]
    fn reports_invalid_yaml_location() {
        let error = parse_data("key: [broken", Some(DataFormat::Yaml)).unwrap_err();
        assert!(error.line.is_some());
    }

    #[test]
    fn parses_yaml_sequence_key() {
        let source = "- [name, age]: [Rae Smith, 4]";
        let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(
            root.children[0].children[0].key.as_deref(),
            Some("[\"name\",\"age\"]")
        );
        assert_eq!(
            root.children[0].children[0].children[0].display_value,
            "\"Rae Smith\""
        );
    }

    #[test]
    fn parses_yaml_stream_with_sequence_key() {
        let source = r#"--- # The Smiths
- {name: John Smith, age: 33}
- name: Mary Smith
  age: 27
- [name, age]: [Rae Smith, 4]
--- # People, by gender
men: [John Smith, Bill Jones]
women:
  - Mary Smith
  - Susan Williams"#;
        let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(root.value_type, JsonValueType::Array);
        let documents = root
            .children
            .iter()
            .filter(|child| child.value_type != JsonValueType::Comment)
            .collect::<Vec<_>>();
        assert_eq!(documents.len(), 2);
        assert_eq!(
            documents[0].children[2].children[0].key.as_deref(),
            Some("[\"name\",\"age\"]")
        );
        assert_eq!(documents[1].children[0].key.as_deref(), Some("men"));
    }

    #[test]
    fn serializes_toml_table() {
        let value = serde_json::json!({"server": {"port": 8080}});
        let output = serialize_data(&value, DataFormat::Toml, false).unwrap();
        assert!(output.contains("[server]"));
        assert!(output.contains("port = 8080"));
    }

    #[test]
    fn parses_and_roundtrips_native_toml_date_time_values() {
        let source = r#"date = 1979-05-27
time = 07:32:00
timestamp = 1979-05-27T07:32:00Z
timestamp_string = "1979-05-27T07:32:00Z"
whole_float = 1.0

[[services]]
id = "api"
depends_on = "database"

[[services]]
id = "database"
"#;
        let (root, format) = parse_data(source, None).unwrap();
        assert_eq!(format, DataFormat::Toml);
        let timestamp = root
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some("timestamp"))
            .unwrap();
        let timestamp_string = root
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some("timestamp_string"))
            .unwrap();
        let whole_float = root
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some("whole_float"))
            .unwrap();

        assert_eq!(timestamp.value_type, JsonValueType::DateTime);
        assert_eq!(timestamp.display_value, "1979-05-27T07:32:00Z");
        assert_eq!(timestamp_string.value_type, JsonValueType::String);
        assert_eq!(whole_float.value_type, JsonValueType::Float);

        let json_value = node_to_value(&root).unwrap();
        assert_eq!(json_value["timestamp"], "1979-05-27T07:32:00Z");
        let output = serialize_node(&root, DataFormat::Toml, false).unwrap();
        assert!(output.contains("timestamp = 1979-05-27T07:32:00Z"));
        assert!(output.contains("whole_float = 1.0"));
        assert!(!output.contains("$__toml_private_datetime"));
        let reparsed = parse_data(&output, Some(DataFormat::Toml)).unwrap().0;
        assert_eq!(node_to_value(&reparsed).unwrap(), json_value);
    }

    #[test]
    fn yaml_tags_are_accepted_but_non_finite_numbers_and_colliding_keys_are_rejected() {
        let non_finite = parse_data("value: .nan", Some(DataFormat::Yaml)).unwrap_err();
        assert!(
            non_finite
                .message
                .contains("не поддерживаются JSON-представлением")
        );

        let colliding_keys =
            parse_data("1: numeric key\n'1': string key", Some(DataFormat::Yaml)).unwrap_err();
        assert!(colliding_keys.message.contains("совпадают"));

        let tagged = parse_data("value: !secret sample", Some(DataFormat::Yaml))
            .unwrap()
            .0;
        assert_eq!(tagged.children[0].value_type, JsonValueType::Metadata);
    }

    #[test]
    fn strict_json_roundtrips_nested_types_and_reports_invalid_input() {
        let source = r#"{"text":"line\nbreak","items":[null,true,-4,2.5]}"#;
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        let output = serialize_node(&root, DataFormat::Json, false).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );
        assert!(parse_data("{invalid}", Some(DataFormat::Json)).is_err());
    }

    #[test]
    fn toml_non_finite_numbers_remain_toml_numbers() {
        let root = parse_data("value = nan", Some(DataFormat::Toml)).unwrap().0;
        assert_eq!(root.children[0].value_type, JsonValueType::Float);
        let toml_output = serialize_node(&root, DataFormat::Toml, false).unwrap();
        assert!(toml_output.contains("value = nan"));
        assert!(serialize_node(&root, DataFormat::Json, false).is_err());
    }
}
