use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use roxmltree::{Document, Node};
use serde_json::{Map, Number, Value};

use super::{JsonObject, edge_record, graph_document, node_record, value_as_string};

struct GraphMlKey {
    id: String,
    domain: String,
    name: String,
    value_type: String,
    default: Option<Value>,
}

struct GexfAttribute {
    id: String,
    domain: String,
    title: String,
    value_type: String,
    default: Option<Value>,
}

pub(super) fn parse_graphml(input: &str) -> Result<Value, String> {
    let input = strip_external_doctype(input)?;
    let document = Document::parse(input.as_ref())
        .map_err(|error| format!("Не удалось разобрать XML GraphML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "graphml" {
        return Err("Корневым элементом GraphML должен быть <graphml>".to_string());
    }
    let graphs = children_named(root, "graph");
    let [graph] = graphs.as_slice() else {
        return Err("GraphML должен содержать ровно один граф верхнего уровня".to_string());
    };
    if graph
        .descendants()
        .any(|element| element.is_element() && element.tag_name().name() == "hyperedge")
    {
        return Err("Гиперрёбра GraphML не поддерживаются".to_string());
    }

    let keys = graphml_keys(root)?;
    let keys_by_id = keys
        .iter()
        .map(|key| (key.id.as_str(), key))
        .collect::<HashMap<_, _>>();
    let default_directed = match graph.attribute("edgedefault") {
        Some("directed") => true,
        Some("undirected") => false,
        Some(value) => {
            return Err(format!(
                "Неподдерживаемое значение GraphML edgedefault: {value}"
            ));
        }
        None => return Err("В GraphML отсутствует атрибут edgedefault".to_string()),
    };

    let node_elements = children_named(*graph, "node");
    let mut node_ids = HashSet::with_capacity(node_elements.len());
    let mut nodes = Vec::with_capacity(node_elements.len());
    for element in node_elements {
        if !children_named(element, "graph").is_empty() {
            return Err("Вложенные графы GraphML не поддерживаются".to_string());
        }
        let id = required_attribute(element, "id", "узла GraphML")?;
        if !node_ids.insert(id.clone()) {
            return Err(format!("ID узла GraphML повторяется: {id}"));
        }
        let mut attributes = graphml_data(element, "node", &keys, &keys_by_id)?;
        add_xml_attributes(&mut attributes, element, &["id"])?;
        add_xml_extensions(&mut attributes, element, &["data"])?;
        nodes.push(node_record(id, attributes));
    }

    let mut directions = HashSet::new();
    let mut edges = Vec::new();
    for element in children_named(*graph, "edge") {
        let source = required_attribute(element, "source", "ребра GraphML")?;
        let target = required_attribute(element, "target", "ребра GraphML")?;
        if !node_ids.contains(&source) || !node_ids.contains(&target) {
            return Err(format!(
                "Ребро GraphML ссылается на неизвестный узел: {source} -> {target}"
            ));
        }
        let directed = element
            .attribute("directed")
            .map(|value| parse_xml_bool(value, "GraphML edge directed"))
            .transpose()?
            .unwrap_or(default_directed);
        directions.insert(directed);

        let mut attributes = graphml_data(element, "edge", &keys, &keys_by_id)?;
        add_xml_attributes(
            &mut attributes,
            element,
            &["id", "source", "target", "directed"],
        )?;
        add_xml_extensions(&mut attributes, element, &["data"])?;
        edges.push(edge_record(
            source,
            target,
            element.attribute("id").map(str::to_string),
            attributes,
        ));
    }
    let directed = uniform_direction(directions, default_directed, "GraphML")?;

    let mut graph_attributes = graphml_data(*graph, "graph", &keys, &keys_by_id)?;
    add_xml_attributes(&mut graph_attributes, *graph, &["id", "edgedefault"])?;
    add_xml_extensions(&mut graph_attributes, *graph, &["node", "edge", "data"])?;

    let mut metadata = Map::new();
    metadata.insert(
        "key_definitions".to_string(),
        Value::Array(keys.iter().map(graphml_key_value).collect()),
    );
    Ok(graph_document(
        graph
            .attribute("id")
            .map(str::to_string)
            .or_else(|| graph_attributes.get("name").and_then(value_as_string)),
        graph_type(directed),
        "graphml",
        graph_attributes,
        metadata,
        nodes,
        edges,
    ))
}

pub(super) fn parse_gexf(input: &str) -> Result<Value, String> {
    let input = strip_external_doctype(input)?;
    let document = Document::parse(input.as_ref())
        .map_err(|error| format!("Не удалось разобрать XML GEXF: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "gexf" {
        return Err("Корневым элементом GEXF должен быть <gexf>".to_string());
    }
    let graphs = children_named(root, "graph");
    let [graph] = graphs.as_slice() else {
        return Err("GEXF должен содержать ровно один граф".to_string());
    };
    let default_edge_type = graph.attribute("defaultedgetype").unwrap_or("directed");
    let default_directed = gexf_direction(default_edge_type)?;
    let definitions = gexf_attributes(*graph)?;
    let definitions_by_key = definitions
        .iter()
        .map(|attribute| (attribute_key(&attribute.domain, &attribute.id), attribute))
        .collect::<HashMap<_, _>>();

    let node_containers = children_named(*graph, "nodes");
    if node_containers.len() > 1 {
        return Err("GEXF должен содержать не более одного элемента <nodes>".to_string());
    }
    let node_elements = node_containers
        .first()
        .map(|container| children_named(*container, "node"))
        .unwrap_or_default();
    let mut node_ids = HashSet::with_capacity(node_elements.len());
    let mut nodes = Vec::with_capacity(node_elements.len());
    for element in node_elements {
        let id = required_attribute(element, "id", "узла GEXF")?;
        if !node_ids.insert(id.clone()) {
            return Err(format!("ID узла GEXF повторяется: {id}"));
        }
        let mut attributes = gexf_data(element, "node", &definitions, &definitions_by_key)?;
        if let Some(label) = element.attribute("label") {
            insert_native_attribute(&mut attributes, "label", Value::String(label.to_string()))?;
        }
        add_xml_attributes(&mut attributes, element, &["id", "label"])?;
        add_xml_extensions(&mut attributes, element, &["attvalues"])?;
        nodes.push(node_record(id, attributes));
    }

    let edge_containers = children_named(*graph, "edges");
    if edge_containers.len() > 1 {
        return Err("GEXF должен содержать не более одного элемента <edges>".to_string());
    }
    let edge_elements = edge_containers
        .first()
        .map(|container| children_named(*container, "edge"))
        .unwrap_or_default();
    let mut directions = HashSet::new();
    let mut edges = Vec::with_capacity(edge_elements.len());
    for element in edge_elements {
        let source = required_attribute(element, "source", "ребра GEXF")?;
        let target = required_attribute(element, "target", "ребра GEXF")?;
        if !node_ids.contains(&source) || !node_ids.contains(&target) {
            return Err(format!(
                "Ребро GEXF ссылается на неизвестный узел: {source} -> {target}"
            ));
        }
        let edge_type = element.attribute("type").unwrap_or(default_edge_type);
        directions.insert(gexf_direction(edge_type)?);

        let mut attributes = gexf_data(element, "edge", &definitions, &definitions_by_key)?;
        if let Some(label) = element.attribute("label") {
            insert_native_attribute(&mut attributes, "label", Value::String(label.to_string()))?;
        }
        if let Some(weight) = element.attribute("weight") {
            insert_native_attribute(&mut attributes, "weight", parse_weight(weight)?)?;
        }
        add_xml_attributes(
            &mut attributes,
            element,
            &["id", "source", "target", "label", "weight"],
        )?;
        add_xml_extensions(&mut attributes, element, &["attvalues"])?;
        edges.push(edge_record(
            source,
            target,
            element.attribute("id").map(str::to_string),
            attributes,
        ));
    }
    let directed = uniform_direction(directions, default_directed, "GEXF")?;

    let mut graph_attributes = gexf_data(*graph, "graph", &definitions, &definitions_by_key)?;
    add_xml_attributes(&mut graph_attributes, *graph, &["defaultedgetype"])?;
    add_xml_extensions(
        &mut graph_attributes,
        *graph,
        &["attributes", "nodes", "edges"],
    )?;

    let mut metadata = Map::new();
    metadata.insert(
        "attribute_definitions".to_string(),
        Value::Array(definitions.iter().map(gexf_attribute_value).collect()),
    );
    if let Some(meta) = children_named(root, "meta").into_iter().next() {
        metadata.insert("metadata".to_string(), xml_element_value(meta));
    }
    let name = children_named(root, "meta")
        .into_iter()
        .next()
        .and_then(|meta| children_named(meta, "title").into_iter().next())
        .map(text_content)
        .filter(|title| !title.is_empty());
    Ok(graph_document(
        name,
        graph_type(directed),
        "gexf",
        graph_attributes,
        metadata,
        nodes,
        edges,
    ))
}

fn graphml_keys(root: Node<'_, '_>) -> Result<Vec<GraphMlKey>, String> {
    let mut keys = Vec::new();
    let mut ids = HashSet::new();
    for element in children_named(root, "key") {
        let id = required_attribute(element, "id", "определения ключа GraphML")?;
        if !ids.insert(id.clone()) {
            return Err(format!("ID ключа GraphML повторяется: {id}"));
        }
        let name = element
            .attribute("attr.name")
            .map(str::to_string)
            .unwrap_or_else(|| id.clone());
        let value_type = element
            .attribute("attr.type")
            .unwrap_or("string")
            .to_string();
        let default = children_named(element, "default")
            .into_iter()
            .next()
            .map(text_content)
            .map(|value| parse_typed_value(&value, &value_type, &id))
            .transpose()?;
        keys.push(GraphMlKey {
            id: id.to_string(),
            domain: element.attribute("for").unwrap_or("all").to_string(),
            name,
            value_type,
            default,
        });
    }
    Ok(keys)
}

fn graphml_data(
    element: Node<'_, '_>,
    domain: &str,
    keys: &[GraphMlKey],
    keys_by_id: &HashMap<&str, &GraphMlKey>,
) -> Result<JsonObject, String> {
    let mut attributes = JsonObject::new();
    let mut names = HashMap::new();
    for key in keys {
        if (key.domain == domain || key.domain == "all")
            && let Some(value) = &key.default
        {
            insert_data_attribute(
                &mut attributes,
                &mut names,
                &key.name,
                &key.id,
                value.clone(),
            )?;
        }
    }

    let mut seen = HashSet::new();
    let mut rich_data = Vec::new();
    for data in children_named(element, "data") {
        let key_id = required_attribute(data, "key", "элемента data GraphML")?;
        if !seen.insert(key_id.clone()) {
            return Err(format!(
                "Ключ GraphML повторяется в элементе data: {key_id}"
            ));
        }
        let definition = keys_by_id.get(key_id.as_str()).copied();
        if let Some(definition) = definition
            && definition.domain != domain
            && definition.domain != "all"
        {
            return Err(format!(
                "Ключ GraphML {key_id} объявлен для {}, но использован для {domain}",
                definition.domain
            ));
        }
        let name = definition.map_or(key_id.as_str(), |definition| definition.name.as_str());
        let value_type = definition.map_or("string", |definition| definition.value_type.as_str());
        let value = text_content(data);
        insert_data_attribute(
            &mut attributes,
            &mut names,
            name,
            &key_id,
            parse_typed_value(&value, value_type, &key_id)?,
        )?;
        if element_children(data).next().is_some() {
            rich_data.push(xml_element_value(data));
        }
    }
    append_xml_values(&mut attributes, "xml_extensions", rich_data)?;
    Ok(attributes)
}

fn graphml_key_value(key: &GraphMlKey) -> Value {
    let mut value = Map::new();
    value.insert("id".to_string(), Value::String(key.id.clone()));
    value.insert("for".to_string(), Value::String(key.domain.clone()));
    value.insert("name".to_string(), Value::String(key.name.clone()));
    value.insert("type".to_string(), Value::String(key.value_type.clone()));
    if let Some(default) = &key.default {
        value.insert("default".to_string(), default.clone());
    }
    Value::Object(value)
}

fn gexf_attributes(graph: Node<'_, '_>) -> Result<Vec<GexfAttribute>, String> {
    let mut definitions = Vec::new();
    let mut ids = HashSet::new();
    for container in children_named(graph, "attributes") {
        let domain = container.attribute("class").unwrap_or("node");
        for element in children_named(container, "attribute") {
            let id = required_attribute(element, "id", "определения атрибута GEXF")?;
            if !ids.insert(attribute_key(domain, &id)) {
                return Err(format!(
                    "ID атрибута GEXF повторяется для класса {domain}: {id}"
                ));
            }
            let title = element
                .attribute("title")
                .map(str::to_string)
                .unwrap_or_else(|| id.clone());
            let value_type = element.attribute("type").unwrap_or("string").to_string();
            let default = children_named(element, "default")
                .into_iter()
                .next()
                .map(text_content)
                .map(|value| parse_typed_value(&value, &value_type, &id))
                .transpose()?;
            definitions.push(GexfAttribute {
                id: id.to_string(),
                domain: domain.to_string(),
                title,
                value_type,
                default,
            });
        }
    }
    Ok(definitions)
}

fn gexf_data(
    element: Node<'_, '_>,
    domain: &str,
    definitions: &[GexfAttribute],
    definitions_by_key: &HashMap<String, &GexfAttribute>,
) -> Result<JsonObject, String> {
    let mut attributes = JsonObject::new();
    let mut names = HashMap::new();
    for definition in definitions {
        if definition.domain == domain
            && let Some(value) = &definition.default
        {
            insert_data_attribute(
                &mut attributes,
                &mut names,
                &definition.title,
                &definition.id,
                value.clone(),
            )?;
        }
    }
    let mut seen = HashSet::new();
    for values in children_named(element, "attvalues") {
        for entry in children_named(values, "attvalue") {
            let id = required_attribute(entry, "for", "атрибута GEXF")?;
            if !seen.insert(id.clone()) {
                return Err(format!("Атрибут GEXF повторяется: {id}"));
            }
            let key = attribute_key(domain, &id);
            let definition = definitions_by_key.get(&key).copied();
            let name = definition.map_or(id.as_str(), |definition| definition.title.as_str());
            let value_type =
                definition.map_or("string", |definition| definition.value_type.as_str());
            let value = entry
                .attribute("value")
                .map(str::to_string)
                .unwrap_or_else(|| text_content(entry));
            insert_data_attribute(
                &mut attributes,
                &mut names,
                name,
                &id,
                parse_typed_value(&value, value_type, &id)?,
            )?;
        }
    }
    Ok(attributes)
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

fn insert_native_attribute(
    attributes: &mut JsonObject,
    name: &str,
    value: Value,
) -> Result<(), String> {
    if attributes
        .get(name)
        .is_some_and(|existing| existing != &value)
    {
        return Err(format!(
            "Атрибут GEXF {name:?} имеет разные значения в XML и пользовательских данных"
        ));
    }
    attributes.insert(name.to_string(), value);
    Ok(())
}

fn gexf_attribute_value(attribute: &GexfAttribute) -> Value {
    let mut value = Map::new();
    value.insert("id".to_string(), Value::String(attribute.id.clone()));
    value.insert("class".to_string(), Value::String(attribute.domain.clone()));
    value.insert("title".to_string(), Value::String(attribute.title.clone()));
    value.insert(
        "type".to_string(),
        Value::String(attribute.value_type.clone()),
    );
    if let Some(default) = &attribute.default {
        value.insert("default".to_string(), default.clone());
    }
    Value::Object(value)
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

fn parse_typed_value(value: &str, value_type: &str, context: &str) -> Result<Value, String> {
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

fn parse_weight(value: &str) -> Result<Value, String> {
    let number = value
        .parse::<f64>()
        .map_err(|error| format!("некорректный вес {value:?}: {error}"))?;
    Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| format!("вес {value:?} не является конечным числом"))
}

fn parse_xml_bool(value: &str, context: &str) -> Result<bool, String> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!(
            "Некорректное логическое значение {context}: {value}"
        )),
    }
}

fn uniform_direction(
    directions: HashSet<bool>,
    fallback: bool,
    format: &str,
) -> Result<bool, String> {
    let mut directions = directions.into_iter();
    match (directions.next(), directions.next()) {
        (None, None) => Ok(fallback),
        (Some(direction), None) => Ok(direction),
        _ => Err(format!(
            "{format} содержит одновременно ориентированные и неориентированные ребра"
        )),
    }
}

fn gexf_direction(edge_type: &str) -> Result<bool, String> {
    match edge_type.to_ascii_lowercase().as_str() {
        "directed" => Ok(true),
        "undirected" | "mutual" => Ok(false),
        _ => Err(format!("Неподдерживаемый тип ребра GEXF: {edge_type}")),
    }
}

fn graph_type(directed: bool) -> &'static str {
    if directed {
        "directed_multigraph"
    } else {
        "undirected_multigraph"
    }
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
