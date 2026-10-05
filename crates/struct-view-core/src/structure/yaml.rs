use std::collections::HashSet;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, Scanner, TScalarStyle, TokenType};

use super::*;

fn next(parser: &mut Parser<impl Iterator<Item = char>>) -> Result<(Event, Marker), ParseError> {
    parser.next_token().map_err(|e| ParseError {
        message: e.to_string(),
        line: Some(e.marker().line()),
        column: Some(e.marker().col() + 1),
    })
}

pub(super) fn parse(input: &str) -> Result<Raw, ParseError> {
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
