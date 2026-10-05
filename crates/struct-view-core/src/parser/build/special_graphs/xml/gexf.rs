use std::collections::{HashMap, HashSet};

use super::*;

struct GexfAttribute {
    id: String,
    domain: String,
    title: String,
    value_type: String,
    default: Option<Value>,
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
        let direction = gexf_direction(edge_type)?;
        directions.insert(direction);

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
        let mut edge = edge_record(
            source,
            target,
            element.attribute("id").map(str::to_string),
            attributes,
        );
        set_edge_direction(&mut edge, direction);
        edges.push(edge);
    }
    let graph_type = direction_graph_type(directions, default_directed);

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
        graph_type,
        "gexf",
        graph_attributes,
        metadata,
        nodes,
        edges,
    ))
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
