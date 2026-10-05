use std::collections::{HashMap, HashSet};

use super::*;

struct GraphMlKey {
    id: String,
    domain: String,
    name: String,
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
