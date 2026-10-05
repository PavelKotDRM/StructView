use std::borrow::Cow;
use std::collections::HashMap;

use roxmltree::{Document, Node};
use serde_json::{Map, Value};

use super::{
    EdgeDirection, JsonObject, edge_record, graph_document, node_record, set_edge_direction,
    value_as_string,
};

mod gexf;
mod graphml;
mod values;

use values::{
    direction_graph_type, gexf_direction, parse_typed_value, parse_weight, parse_xml_bool,
};

pub(super) fn parse_graphml(input: &str) -> Result<Value, String> {
    graphml::parse_graphml(input)
}

pub(super) fn parse_gexf(input: &str) -> Result<Value, String> {
    gexf::parse_gexf(input)
}

fn insert_data_attribute(
    attributes: &mut JsonObject,
    names: &mut HashMap<String, String>,
    name: &str,
    id: &str,
    value: Value,
) -> Result<(), String> {
    if let Some(previous_id) = names.get(name)
        && previous_id != id
    {
        return Err(format!(
            "Имя атрибута {name:?} совпадает для разных XML-ключей: {previous_id} и {id}"
        ));
    }
    names.insert(name.to_string(), id.to_string());
    attributes.insert(name.to_string(), value);
    Ok(())
}

fn children_named<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Vec<Node<'a, 'input>> {
    element_children(parent)
        .filter(|element| element.tag_name().name() == name)
        .collect()
}

fn element_children<'a, 'input>(
    parent: Node<'a, 'input>,
) -> impl Iterator<Item = Node<'a, 'input>> {
    parent.children().filter(|child| child.is_element())
}

fn required_attribute(element: Node<'_, '_>, name: &str, context: &str) -> Result<String, String> {
    element
        .attribute(name)
        .map(str::to_string)
        .ok_or_else(|| format!("В {context} отсутствует атрибут {name}"))
}

fn text_content(element: Node<'_, '_>) -> String {
    element
        .descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

fn attribute_key(domain: &str, id: &str) -> String {
    format!("{domain}:{id}")
}

fn strip_external_doctype(input: &str) -> Result<Cow<'_, str>, String> {
    let mut cursor = 0;
    while let Some(relative_start) = input[cursor..].find("<!") {
        let start = cursor + relative_start;
        let remaining = &input[start..];
        if remaining.starts_with("<!--") {
            let comment_end = remaining
                .find("-->")
                .ok_or_else(|| "Незакрытый XML-комментарий".to_string())?;
            cursor = start + comment_end + 3;
            continue;
        }
        if remaining.starts_with("<![CDATA[") {
            let cdata_end = remaining
                .find("]]>")
                .ok_or_else(|| "Незакрытый XML CDATA-блок".to_string())?;
            cursor = start + cdata_end + 3;
            continue;
        }
        if !remaining.starts_with("<!DOCTYPE")
            || !remaining[9..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            cursor = start + 2;
            continue;
        }

        let bytes = input.as_bytes();
        let mut quote = None;
        let mut subset_depth = 0;
        let mut has_internal_subset = false;
        for (offset, byte) in bytes[start + 9..].iter().copied().enumerate() {
            if let Some(quote_byte) = quote {
                if byte == quote_byte {
                    quote = None;
                }
                continue;
            }
            match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'[' => {
                    subset_depth += 1;
                    has_internal_subset = true;
                }
                b']' if subset_depth > 0 => subset_depth -= 1,
                b'>' if subset_depth == 0 => {
                    if has_internal_subset {
                        return Err("Внутренние сущности XML DTD не поддерживаются".to_string());
                    }
                    let end = start + 9 + offset + 1;
                    let mut stripped = String::with_capacity(input.len() - (end - start));
                    stripped.push_str(&input[..start]);
                    stripped.push_str(&input[end..]);
                    return Ok(Cow::Owned(stripped));
                }
                _ => {}
            }
        }
        return Err("Незакрытая декларация XML DOCTYPE".to_string());
    }
    Ok(Cow::Borrowed(input))
}

fn add_xml_attributes(
    attributes: &mut JsonObject,
    element: Node<'_, '_>,
    excluded: &[&str],
) -> Result<(), String> {
    let mut xml_attributes = Map::new();
    for attribute in element.attributes() {
        if excluded.contains(&attribute.name()) {
            continue;
        }
        let name = attribute.namespace().map_or_else(
            || attribute.name().to_string(),
            |namespace| format!("{{{namespace}}}{}", attribute.name()),
        );
        xml_attributes.insert(name, Value::String(attribute.value().to_string()));
    }
    if !xml_attributes.is_empty() {
        if attributes.contains_key("xml_attributes") {
            return Err(
                "XML-метаданные xml_attributes конфликтуют с пользовательскими данными".to_string(),
            );
        }
        attributes.insert("xml_attributes".to_string(), Value::Object(xml_attributes));
    }
    Ok(())
}

fn add_xml_extensions(
    attributes: &mut JsonObject,
    element: Node<'_, '_>,
    excluded: &[&str],
) -> Result<(), String> {
    let extensions = element_children(element)
        .filter(|child| !excluded.contains(&child.tag_name().name()))
        .map(xml_element_value)
        .collect::<Vec<_>>();
    append_xml_values(attributes, "xml_extensions", extensions)
}

fn append_xml_values(
    attributes: &mut JsonObject,
    key: &str,
    values: Vec<Value>,
) -> Result<(), String> {
    if values.is_empty() {
        return Ok(());
    }
    match attributes.get_mut(key) {
        Some(Value::Array(existing)) => existing.extend(values),
        None => {
            attributes.insert(key.to_string(), Value::Array(values));
        }
        Some(_) => {
            return Err(format!(
                "XML-метаданные {key} конфликтуют с пользовательскими данными"
            ));
        }
    }
    Ok(())
}

fn xml_element_value(element: Node<'_, '_>) -> Value {
    let mut value = Map::new();
    value.insert(
        "name".to_string(),
        Value::String(element.tag_name().name().to_string()),
    );
    if let Some(namespace) = element.tag_name().namespace() {
        value.insert(
            "namespace".to_string(),
            Value::String(namespace.to_string()),
        );
    }
    let attributes = element
        .attributes()
        .map(|attribute| {
            let name = attribute.namespace().map_or_else(
                || attribute.name().to_string(),
                |namespace| format!("{{{namespace}}}{}", attribute.name()),
            );
            (name, Value::String(attribute.value().to_string()))
        })
        .collect::<Map<_, _>>();
    if !attributes.is_empty() {
        value.insert("attributes".to_string(), Value::Object(attributes));
    }
    let text = element
        .children()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<String>();
    if !text.trim().is_empty() {
        value.insert("text".to_string(), Value::String(text));
    }
    let children = element_children(element)
        .map(xml_element_value)
        .collect::<Vec<_>>();
    if !children.is_empty() {
        value.insert("children".to_string(), Value::Array(children));
    }
    Value::Object(value)
}
