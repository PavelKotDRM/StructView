use std::collections::HashSet;

use serde_json::{Number, Value};

use super::EdgeDirection;

pub(super) fn parse_typed_value(
    value: &str,
    value_type: &str,
    context: &str,
) -> Result<Value, String> {
    let trimmed = value.trim();
    match value_type.to_ascii_lowercase().as_str() {
        "boolean" | "bool" => match trimmed {
            "true" | "1" => Ok(Value::Bool(true)),
            "false" | "0" => Ok(Value::Bool(false)),
            _ => Err(format!(
                "Некорректное логическое значение {value:?} атрибута {context}"
            )),
        },
        "int" | "integer" | "long" => {
            let number = trimmed.parse::<i64>().map_err(|error| {
                format!("Некорректное целое значение {value:?} атрибута {context}: {error}")
            })?;
            Ok(Value::Number(Number::from(number)))
        }
        "float" | "double" => parse_weight(trimmed).map_err(|error| {
            format!("Некорректное вещественное значение {value:?} атрибута {context}: {error}")
        }),
        _ => Ok(Value::String(value.to_string())),
    }
}

pub(super) fn parse_weight(value: &str) -> Result<Value, String> {
    let number = value
        .parse::<f64>()
        .map_err(|error| format!("некорректный вес {value:?}: {error}"))?;
    Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| format!("вес {value:?} не является конечным числом"))
}

pub(super) fn parse_xml_bool(value: &str, context: &str) -> Result<bool, String> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!(
            "Некорректное логическое значение {context}: {value}"
        )),
    }
}

pub(super) fn direction_graph_type(
    directions: HashSet<EdgeDirection>,
    fallback: EdgeDirection,
) -> &'static str {
    if directions.len() > 1 {
        "mixed_multigraph"
    } else {
        match directions.into_iter().next().unwrap_or(fallback) {
            EdgeDirection::Undirected => "undirected_multigraph",
            EdgeDirection::Bidirectional => "bidirectional_multigraph",
            EdgeDirection::Directed | EdgeDirection::Reverse => "directed_multigraph",
        }
    }
}

pub(super) fn gexf_direction(edge_type: &str) -> Result<EdgeDirection, String> {
    EdgeDirection::parse(edge_type).map_err(|error| format!("GEXF: {error}"))
}
