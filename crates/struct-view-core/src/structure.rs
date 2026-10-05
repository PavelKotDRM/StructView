//! Independent, source-ordered data hierarchy. YAML aliases remain leaves.

use crate::parser::{DataFormat, ParseError};
use serde::Deserialize;
use std::collections::HashSet;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, Scanner, TScalarStyle, TokenType};

const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
    Date,
    Reference,
}

#[derive(Debug)]
pub struct Node {
    pub key: String,
    pub value: String,
    pub kind: Kind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Unambiguous JSON Pointer, including array indices.
    pub path: String,
}

#[derive(Debug)]
pub struct Document {
    pub format: DataFormat,
    pub nodes: Vec<Node>,
}

struct Raw {
    kind: Kind,
    value: String,
    children: Vec<(String, Raw)>,
}

impl Raw {
    fn leaf(kind: Kind, value: String) -> Self {
        Self {
            kind,
            value,
            children: Vec::new(),
        }
    }

    fn container(kind: Kind, children: Vec<(String, Raw)>) -> Self {
        Self {
            kind,
            value: children.len().to_string(),
            children,
        }
    }
}

fn error(message: impl Into<String>, line: Option<usize>) -> ParseError {
    ParseError {
        message: message.into(),
        line,
        column: None,
    }
}

/// Parse file text with automatic detection or an explicit data format.
pub fn parse(input: &str, format: Option<DataFormat>) -> Result<Document, ParseError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let format = format.unwrap_or_else(|| {
        if serde_json::from_str::<serde_json::Value>(input).is_ok() {
            DataFormat::Json
        } else if toml::from_str::<toml::Value>(input).is_ok() {
            DataFormat::Toml
        } else {
            DataFormat::Yaml
        }
    });
    let raw = match format {
        DataFormat::Json => {
            let raw: &serde_json::value::RawValue =
                serde_json::from_str(input).map_err(json_error)?;
            json(raw.get(), 0)?
        }
        DataFormat::Toml => {
            let value = input.parse::<toml_edit::DocumentMut>().map_err(|e| {
                let offset = e
                    .span()
                    .map(|span| span.start)
                    .unwrap_or(0)
                    .min(input.len());
                error(
                    e.to_string(),
                    Some(input[..offset].bytes().filter(|&b| b == b'\n').count() + 1),
                )
            })?;
            toml_table(value.as_table(), 0)?
        }
        DataFormat::Yaml => yaml(input)?,
        _ => {
            return Err(error(
                "Structure mode supports JSON, YAML and TOML only",
                None,
            ));
        }
    };
    let mut document = Document {
        format,
        nodes: Vec::new(),
    };
    flatten(raw, "$".into(), None, String::new(), &mut document.nodes);
    Ok(document)
}

fn check_depth(depth: usize) -> Result<(), ParseError> {
    if depth >= MAX_DEPTH {
        Err(error("Maximum structure depth is 128", None))
    } else {
        Ok(())
    }
}

fn json_error(e: serde_json::Error) -> ParseError {
    ParseError {
        message: e.to_string(),
        line: Some(e.line()),
        column: Some(e.column()),
    }
}

struct OrderedObject(Vec<(String, Box<serde_json::value::RawValue>)>);

impl<'de> Deserialize<'de> for OrderedObject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedObject;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(OrderedObject(entries))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

fn json(input: &str, depth: usize) -> Result<Raw, ParseError> {
    use serde_json::Value;
    check_depth(depth)?;
    if input.starts_with('{') {
        let entries: OrderedObject = serde_json::from_str(input).map_err(json_error)?;
        return Ok(Raw::container(
            Kind::Object,
            entries
                .0
                .into_iter()
                .map(|(key, value)| Ok((key, json(value.get(), depth + 1)?)))
                .collect::<Result<_, ParseError>>()?,
        ));
    }
    if input.starts_with('[') {
        let values: Vec<Box<serde_json::value::RawValue>> =
            serde_json::from_str(input).map_err(json_error)?;
        return Ok(Raw::container(
            Kind::Array,
            values
                .into_iter()
                .enumerate()
                .map(|(i, value)| Ok((i.to_string(), json(value.get(), depth + 1)?)))
                .collect::<Result<_, ParseError>>()?,
        ));
    }
    let value = serde_json::from_str(input).map_err(json_error)?;
    Ok(match value {
        Value::Object(_) | Value::Array(_) => return Err(error("Expected JSON scalar", None)),
        Value::String(value) => Raw::leaf(Kind::String, value),
        Value::Number(value) => Raw::leaf(Kind::Number, value.to_string()),
        Value::Bool(value) => Raw::leaf(Kind::Bool, value.to_string()),
        Value::Null => Raw::leaf(Kind::Null, "null".into()),
    })
}

fn toml_table(table: &toml_edit::Table, depth: usize) -> Result<Raw, ParseError> {
    check_depth(depth)?;
    Ok(Raw::container(
        Kind::Object,
        table
            .iter()
            .map(|(key, item)| {
                let raw = match item {
                    toml_edit::Item::Table(table) => toml_table(table, depth + 1)?,
                    toml_edit::Item::ArrayOfTables(tables) => Raw::container(
                        Kind::Array,
                        tables
                            .iter()
                            .enumerate()
                            .map(|(i, table)| Ok((i.to_string(), toml_table(table, depth + 2)?)))
                            .collect::<Result<_, ParseError>>()?,
                    ),
                    toml_edit::Item::Value(value) => toml_node(value, depth + 1)?,
                    toml_edit::Item::None => return Err(error("Missing TOML value", None)),
                };
                Ok((key.to_string(), raw))
            })
            .collect::<Result<_, ParseError>>()?,
    ))
}

fn toml_node(value: &toml_edit::Value, depth: usize) -> Result<Raw, ParseError> {
    use toml_edit::Value;
    check_depth(depth)?;
    Ok(match value {
        Value::InlineTable(entries) => Raw::container(
            Kind::Object,
            entries
                .iter()
                .map(|(key, value)| Ok((key.to_string(), toml_node(value, depth + 1)?)))
                .collect::<Result<_, ParseError>>()?,
        ),
        Value::Array(values) => Raw::container(
            Kind::Array,
            values
                .iter()
                .enumerate()
                .map(|(i, value)| Ok((i.to_string(), toml_node(value, depth + 1)?)))
                .collect::<Result<_, ParseError>>()?,
        ),
        Value::String(value) => Raw::leaf(Kind::String, value.value().clone()),
        Value::Integer(value) => Raw::leaf(Kind::Number, value.value().to_string()),
        Value::Float(value) => Raw::leaf(Kind::Number, value.value().to_string()),
        Value::Boolean(value) => Raw::leaf(Kind::Bool, value.value().to_string()),
        Value::Datetime(value) => Raw::leaf(Kind::Date, value.value().to_string()),
    })
}

fn next(parser: &mut Parser<impl Iterator<Item = char>>) -> Result<(Event, Marker), ParseError> {
    parser.next_token().map_err(|e| ParseError {
        message: e.to_string(),
        line: Some(e.marker().line()),
        column: Some(e.marker().col() + 1),
    })
}

fn yaml(input: &str) -> Result<Raw, ParseError> {
    let anchors: Vec<String> = Scanner::new(input.chars())
        .filter_map(|token| {
            if let TokenType::Anchor(name) = token.1 {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    let mut parser = Parser::new_from_str(input);
    let mut documents = Vec::new();
    let mut active_anchors = HashSet::new();
    loop {
        let (event, mark) = next(&mut parser)?;
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart => {
                active_anchors.clear();
                let (event, mark) = next(&mut parser)?;
                documents.push((
                    documents.len().to_string(),
                    yaml_node(event, mark, &mut parser, &anchors, &mut active_anchors, 0)?,
                ));
                let (end, mark) = next(&mut parser)?;
                if end != Event::DocumentEnd {
                    return Err(error("Expected YAML document end", Some(mark.line())));
                }
            }
            Event::StreamStart => {}
            _ => return Err(error("Unexpected YAML event", Some(mark.line()))),
        }
    }
    Ok(if documents.is_empty() {
        Raw::leaf(Kind::Null, "null".into())
    } else if documents.len() == 1 {
        documents.remove(0).1
    } else {
        Raw::container(Kind::Array, documents)
    })
}

fn yaml_node(
    event: Event,
    mark: Marker,
    parser: &mut Parser<impl Iterator<Item = char>>,
    anchors: &[String],
    active_anchors: &mut HashSet<usize>,
    depth: usize,
) -> Result<Raw, ParseError> {
    check_depth(depth).map_err(|mut e| {
        e.line = Some(mark.line());
        e
    })?;
    match &event {
        Event::Scalar(_, _, id, _) | Event::SequenceStart(id, _) | Event::MappingStart(id, _)
            if *id != 0 =>
        {
            active_anchors.insert(*id);
        }
        _ => {}
    }
    let (mut raw, anchor) = match event {
        Event::Alias(id) => {
            if !active_anchors.contains(&id) {
                return Err(error(
                    "YAML alias is not defined in this document",
                    Some(mark.line()),
                ));
            }
            let name = anchors
                .get(id.saturating_sub(1))
                .ok_or_else(|| error("Unknown YAML alias", Some(mark.line())))?;
            return Ok(Raw::leaf(Kind::Reference, format!("*{name}")));
        }
        Event::Scalar(value, style, anchor, tag) => {
            let mut kind = Kind::String;
            if style == TScalarStyle::Plain {
                kind = match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&value) {
                    Ok(serde_yaml_ng::Value::Null) => Kind::Null,
                    Ok(serde_yaml_ng::Value::Bool(_)) => Kind::Bool,
                    Ok(serde_yaml_ng::Value::Number(_)) => Kind::Number,
                    _ => Kind::String,
                };
            }
            if let Some(tag) = &tag {
                kind = match tag.suffix.as_str() {
                    "str" => Kind::String,
                    "bool" => Kind::Bool,
                    "int" | "float" => Kind::Number,
                    "null" => Kind::Null,
                    "timestamp" => Kind::Date,
                    _ => kind,
                };
            }
            if kind == Kind::String
                && style == TScalarStyle::Plain
                && tag.is_none()
                && value.parse::<toml::value::Datetime>().is_ok()
            {
                kind = Kind::Date;
            }
            let value = if let Some(tag) = tag {
                format!("{}{} {value}", tag.handle, tag.suffix)
            } else {
                value
            };
            (Raw::leaf(kind, value), anchor)
        }
        Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
            let mapping = matches!(event, Event::MappingStart(..));
            let mut children = Vec::new();
            loop {
                let (event, mark) = next(parser)?;
                if (mapping && event == Event::MappingEnd)
                    || (!mapping && event == Event::SequenceEnd)
                {
                    break;
                }
                let node = yaml_node(event, mark, parser, anchors, active_anchors, depth + 1)?;
                if mapping {
                    let key = yaml_key(&node);
                    let (event, mark) = next(parser)?;
                    children.push((
                        key,
                        yaml_node(event, mark, parser, anchors, active_anchors, depth + 1)?,
                    ));
                } else {
                    children.push((children.len().to_string(), node));
                }
            }
            (
                Raw::container(if mapping { Kind::Object } else { Kind::Array }, children),
                anchor,
            )
        }
        _ => return Err(error("Expected YAML value", Some(mark.line()))),
    };
    if anchor != 0
        && let Some(name) = anchors.get(anchor - 1)
    {
        raw.value = format!("&{name} {}", raw.value);
    }
    Ok(raw)
}

fn yaml_key(raw: &Raw) -> String {
    if raw.children.is_empty() {
        return raw.value.clone();
    }
    format!(
        "[{}]",
        raw.children
            .iter()
            .map(|(key, child)| format!("{key}: {}", yaml_key(child)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn flatten(
    raw: Raw,
    key: String,
    parent: Option<usize>,
    path: String,
    nodes: &mut Vec<Node>,
) -> usize {
    let id = nodes.len();
    nodes.push(Node {
        key,
        parent,
        path: path.clone(),
        kind: raw.kind,
        value: raw.value,
        children: Vec::new(),
    });
    for (key, child) in raw.children {
        let child_path = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
        let child_id = flatten(child, key, Some(id), child_path, nodes);
        nodes[id].children.push(child_id);
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_order_and_tree_invariants() {
        for (input, format) in [
            (r#"{"z": 1, "a": {"b": [true, null]}}"#, DataFormat::Json),
            ("z = 1\n[a]\nb = [true, false]", DataFormat::Toml),
            ("z: 1\na:\n  b: [true, null]", DataFormat::Yaml),
        ] {
            let doc = parse(input, Some(format)).unwrap();
            assert_eq!(doc.nodes[1].key, "z");
            assert_eq!(doc.nodes[2].key, "a");
            assert_eq!(doc.nodes.iter().filter(|n| n.parent.is_none()).count(), 1);
            assert_eq!(
                doc.nodes.iter().map(|n| n.children.len()).sum::<usize>(),
                doc.nodes.len() - 1
            );
            for (id, node) in doc.nodes.iter().enumerate().skip(1) {
                let parent = node.parent.unwrap();
                assert!(parent < id);
                assert!(doc.nodes[parent].children.contains(&id));
            }
        }
    }

    #[test]
    fn aliases_including_cycles_are_references_not_edges() {
        let doc = parse(
            "z: &thing {self: *thing, x: 1}\na: *thing",
            Some(DataFormat::Yaml),
        )
        .unwrap();
        assert_eq!(doc.nodes.len(), 5);
        assert_eq!(doc.nodes[1].value, "&thing 2");
        assert_eq!(doc.nodes[2].value, "*thing");
        assert_eq!(doc.nodes[4].kind, Kind::Reference);
        assert!(doc.nodes[4].children.is_empty());
    }

    #[test]
    fn toml_tables_dates_and_errors() {
        let doc = parse(
            "date = 2026-10-05\n[[items]]\nz = 1\na = 2\n[items.sub]\nx = 3",
            Some(DataFormat::Toml),
        )
        .unwrap();
        assert_eq!(doc.nodes[1].kind, Kind::Date);
        assert_eq!(doc.nodes[2].kind, Kind::Array);
        assert_eq!(doc.nodes[4].key, "z");
        for (input, format) in [
            ("{\n bad", DataFormat::Json),
            ("a:\n b: [", DataFormat::Yaml),
            ("a = 1\nb = ", DataFormat::Toml),
        ] {
            assert!(parse(input, Some(format)).unwrap_err().line.is_some());
        }
    }

    #[test]
    fn automatic_detection_and_pointer_escaping() {
        let doc = parse("true", None).unwrap();
        assert_eq!(doc.nodes[0].kind, Kind::Bool);
        assert_eq!(doc.format, DataFormat::Json);
        assert_eq!(parse("a = 1", None).unwrap().format, DataFormat::Toml);
        assert_eq!(
            parse(r#"{"a/b~": 1}"#, None).unwrap().nodes[1].path,
            "/a~1b~0"
        );
    }

    #[test]
    fn ordered_json_preserves_precision_and_number_named_keys() {
        let doc = parse(
            r#"{"z": 123456789012345678901234567890, "a": {"$serde_json::private::Number":"123"}}"#,
            Some(DataFormat::Json),
        )
        .unwrap();
        assert_eq!(doc.nodes[1].value, "123456789012345678901234567890");
        assert_eq!(doc.nodes[2].kind, Kind::Object);
        assert_eq!(doc.nodes[3].kind, Kind::String);
    }

    #[test]
    fn yaml_streams_quoted_scalars_dates_and_anchor_scope() {
        let doc = parse(
            "---\nz: &a 'true'\nb: *a\n---\nz: &b 2026-10-05\nb: *b",
            Some(DataFormat::Yaml),
        )
        .unwrap();
        assert_eq!(doc.nodes[0].kind, Kind::Array);
        assert_eq!(doc.nodes[2].kind, Kind::String);
        assert_eq!(doc.nodes[3].value, "*a");
        assert_eq!(doc.nodes[5].kind, Kind::Date);
        assert_eq!(doc.nodes[6].value, "*b");
        assert!(parse("---\nz: &a 1\n---\nb: *a", Some(DataFormat::Yaml)).is_err());
        assert!(parse("a: *missing", Some(DataFormat::Yaml)).is_err());
        let deep = format!("{}null{}", "[".repeat(200), "]".repeat(200));
        assert!(parse(&deep, Some(DataFormat::Yaml)).is_err());
    }

    #[test]
    fn toml_inline_tables_dotted_keys_and_table_order() {
        let doc = parse(
            "z = { last = 1, first = 2 }\na.b = true\n[second]\nx = 3\n[first]\nx = 4",
            Some(DataFormat::Toml),
        )
        .unwrap();
        assert_eq!(doc.nodes[1].key, "z");
        assert_eq!(doc.nodes[2].key, "last");
        assert_eq!(doc.nodes[3].key, "first");
        assert_eq!(doc.nodes[6].key, "second");
        assert_eq!(doc.nodes[8].key, "first");
    }
}
