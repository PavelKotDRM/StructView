use std::collections::{BTreeSet, HashMap};

use struct_view_core::graph::EdgeDirection;
use struct_view_core::parser::{JsonNode, JsonValueType};

mod explicit;
mod shapes;

/// Узел графа сущностей, найденный по идентификатору или определению схемы.
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub path: String,
    pub search_paths: Vec<String>,
    pub partition: Option<usize>,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowchartNodeShape {
    Process,
    Terminator,
    Decision,
    InputOutput,
    Preparation,
    Subprocess,
    Database,
    Connector,
}

impl FlowchartNodeShape {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Terminator => "terminator",
            Self::Decision => "decision",
            Self::InputOutput => "input_output",
            Self::Preparation => "preparation",
            Self::Subprocess => "subprocess",
            Self::Database => "database",
            Self::Connector => "connector",
        }
    }
}

impl GraphNode {
    pub fn hover_text(&self) -> String {
        let mut text = format!("{}\n{}\n{}", self.label, self.id, self.path);
        for (key, value) in &self.attributes {
            text.push_str(&format!("\n{key}: {value}"));
        }
        text
    }

    pub fn flowchart_shape(&self) -> Option<FlowchartNodeShape> {
        ["shape", "node_type", "kind", "type"]
            .iter()
            .find_map(|key| {
                self.attributes
                    .iter()
                    .find(|(attribute, _)| {
                        attribute
                            .rsplit('.')
                            .next()
                            .is_some_and(|name| name.eq_ignore_ascii_case(key))
                    })
                    .and_then(|(_, value)| {
                        let value = value.trim();
                        let value = value
                            .strip_prefix('"')
                            .and_then(|value| value.strip_suffix('"'))
                            .unwrap_or(value);
                        match value.to_ascii_lowercase().as_str() {
                            "process" | "rect" | "rectangle" | "box" | "square" => {
                                Some(FlowchartNodeShape::Process)
                            }
                            "terminator" | "terminal" | "start" | "end" | "ellipse" | "stadium"
                            | "round" | "rounded" => Some(FlowchartNodeShape::Terminator),
                            "decision" | "diamond" | "rhombus" => {
                                Some(FlowchartNodeShape::Decision)
                            }
                            "input_output" | "input-output" | "input" | "output" | "io"
                            | "parallelogram" => Some(FlowchartNodeShape::InputOutput),
                            "preparation" | "hexagon" => Some(FlowchartNodeShape::Preparation),
                            "subprocess" | "subroutine" => Some(FlowchartNodeShape::Subprocess),
                            "database" | "cylinder" => Some(FlowchartNodeShape::Database),
                            "connector" | "circle" => Some(FlowchartNodeShape::Connector),
                            _ => None,
                        }
                    })
            })
    }
}

fn record_attributes(record: &JsonNode) -> Vec<(String, String)> {
    if record.value_type != JsonValueType::Object {
        return Vec::new();
    }
    super::table::build_table(record)
        .rows
        .into_iter()
        .skip(1)
        .filter(|row| row.value_type != JsonValueType::Comment)
        .map(|row| {
            let value = match row.value_type {
                JsonValueType::Object => format!("{{{}}}", row.child_count),
                JsonValueType::Array => format!("[{}]", row.child_count),
                _ => row.value,
            };
            (
                row.path
                    .strip_prefix("$.")
                    .or_else(|| row.path.strip_prefix('$'))
                    .unwrap_or(&row.path)
                    .to_string(),
                value,
            )
        })
        .collect()
}

/// Направленное ребро от сущности, содержащей ссылку, к её целевой сущности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub source: usize,
    pub target: usize,
    pub label: String,
    pub direction: EdgeDirection,
    pub attributes: Vec<(String, String)>,
}

impl GraphEdge {
    pub fn hover_text(&self) -> String {
        let mut text = format!("{}\ndirection: {}", self.label, self.direction.as_str());
        for (key, value) in &self.attributes {
            text.push_str(&format!("\n{key}: {value}"));
        }
        text
    }
}

/// Граф идентифицированных сущностей и распознанных ссылок между ними.
#[derive(Debug)]
pub struct RelationshipGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub directed: bool,
    pub partition_names: Option<Vec<String>>,
}

impl Default for RelationshipGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            directed: true,
            partition_names: None,
        }
    }
}

impl RelationshipGraph {
    pub fn direction_name(&self) -> &'static str {
        match self.edges.first().map(|edge| edge.direction) {
            Some(direction) if self.edges.iter().all(|edge| edge.direction == direction) => {
                direction.as_str()
            }
            Some(_) => "mixed",
            None => EdgeDirection::from_directed(self.directed).as_str(),
        }
    }
}

/// Построить граф по полям `id`, `_id`, `$id`, `$ref` и именам зависимостей.
pub fn build_relationship_graph(root: &JsonNode) -> RelationshipGraph {
    try_build_relationship_graph(root).expect("valid graph fixture")
}

pub fn try_build_relationship_graph(root: &JsonNode) -> Result<RelationshipGraph, String> {
    let editable = crate::app::graph_editor::GraphDocument::read(root)?;
    let explicit_graph = match shapes::build(root)? {
        Some(graph) => Some(graph),
        None => explicit::build_supported_graph(root)?,
    };
    if let Some(mut graph) = explicit_graph {
        if object_child(root, "graph")
            .and_then(|metadata| object_scalar(metadata, &["type"]))
            .is_some_and(|kind| {
                matches!(
                    kind.to_ascii_lowercase().as_str(),
                    "flowchart" | "flowchart_multigraph"
                )
            })
        {
            for node in &mut graph.nodes {
                if node.flowchart_shape().is_some() {
                    continue;
                }
                let has_shape_attribute = node.attributes.iter().any(|(key, _)| {
                    matches!(
                        key.rsplit('.')
                            .next()
                            .unwrap_or(key)
                            .to_ascii_lowercase()
                            .as_str(),
                        "shape" | "node_type" | "kind" | "type"
                    )
                });
                if has_shape_attribute {
                    return Err(format!("Unsupported flowchart node shape in {}", node.path));
                }
                node.attributes
                    .push(("shape".to_string(), "process".to_string()));
            }
        }
        if let Some(document) = editable {
            let edge_records = object_child(root, "edges").ok_or("Missing editor edges")?;
            let unit = document
                .metadata
                .get("weight_unit")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let records = edge_records
                .children
                .iter()
                .filter(|node| node.value_type != JsonValueType::Comment);
            for (edge, record) in graph.edges.iter_mut().zip(records) {
                edge.label = explicit::edge_label_with_id(record, unit, false);
            }
            if !document.partitions.is_empty() {
                let mut names = document.partitions.clone();
                let ungrouped = names.len();
                if document
                    .nodes
                    .iter()
                    .any(|record| record.get("partition").is_none())
                {
                    names.push(String::new());
                }
                for (node, record) in graph.nodes.iter_mut().zip(&document.nodes) {
                    node.partition = Some(
                        record["partition"]
                            .as_str()
                            .and_then(|name| {
                                document.partitions.iter().position(|item| item == name)
                            })
                            .unwrap_or(ungrouped),
                    );
                }
                graph.partition_names = Some(names);
            }
        }
        if !graph.edges.is_empty() {
            graph.directed = graph
                .edges
                .iter()
                .any(|edge| edge.direction != EdgeDirection::Undirected);
        }
        return Ok(graph);
    }

    let mut graph = RelationshipGraph::default();
    let mut nodes_by_path = HashMap::new();
    let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();
    collect_graph_nodes(
        root,
        "#",
        false,
        &mut graph.nodes,
        &mut nodes_by_path,
        &mut aliases,
    );

    let mut edges = BTreeSet::new();
    collect_graph_edges(root, None, &nodes_by_path, &aliases, &mut edges);
    graph.edges = edges
        .into_iter()
        .map(|(source, target, label)| GraphEdge {
            source,
            target,
            label,
            direction: EdgeDirection::Directed,
            attributes: Vec::new(),
        })
        .collect();
    Ok(graph)
}

fn collect_graph_nodes(
    node: &JsonNode,
    pointer: &str,
    is_definition: bool,
    nodes: &mut Vec<GraphNode>,
    nodes_by_path: &mut HashMap<String, usize>,
    aliases: &mut HashMap<String, Vec<usize>>,
) {
    if node.value_type == JsonValueType::Comment {
        return;
    }
    if node.value_type == JsonValueType::Metadata {
        for child in &node.children {
            if child.value_type != JsonValueType::Comment {
                collect_graph_nodes(child, pointer, is_definition, nodes, nodes_by_path, aliases);
            }
        }
        return;
    }

    if node.value_type == JsonValueType::Object {
        let identity = object_scalar(node, &["id", "_id", "$id"]);
        let name = object_scalar(node, &["name", "title", "label"]);
        let contains_reference = node
            .children
            .iter()
            .any(|child| child.key.as_deref().is_some_and(is_reference_key));
        let inferred_identity = if identity.is_none() && contains_reference {
            name.clone()
        } else {
            None
        };

        if identity.is_some() || inferred_identity.is_some() || is_definition {
            let id = identity
                .or(inferred_identity)
                .unwrap_or_else(|| pointer.to_string());
            let label = name
                .or_else(|| node.key.clone())
                .unwrap_or_else(|| id.clone());
            let index = nodes.len();
            nodes.push(GraphNode {
                id: id.clone(),
                label,
                path: node.path.clone(),
                search_paths: std::iter::once(node.path.clone())
                    .chain(node.children.iter().map(|child| child.path.clone()))
                    .collect(),
                partition: None,
                attributes: record_attributes(node),
            });
            nodes_by_path.insert(node.path.clone(), index);
            add_graph_alias(aliases, id, index);
            add_graph_alias(aliases, pointer.to_string(), index);
        }
    }

    let is_definition_container = node
        .key
        .as_deref()
        .is_some_and(|key| matches!(key, "definitions" | "$defs" | "schemas"));
    for child in &node.children {
        let child_pointer = append_json_pointer(pointer, child.key.as_deref().unwrap_or_default());
        let child_is_definition =
            is_definition_container && child.value_type == JsonValueType::Object;
        collect_graph_nodes(
            child,
            &child_pointer,
            child_is_definition,
            nodes,
            nodes_by_path,
            aliases,
        );
    }
}

fn add_graph_alias(aliases: &mut HashMap<String, Vec<usize>>, alias: String, index: usize) {
    let indices = aliases.entry(alias).or_default();
    if !indices.contains(&index) {
        indices.push(index);
    }
}

fn collect_graph_edges(
    node: &JsonNode,
    source: Option<usize>,
    nodes_by_path: &HashMap<String, usize>,
    aliases: &HashMap<String, Vec<usize>>,
    edges: &mut BTreeSet<(usize, usize, String)>,
) {
    let source = nodes_by_path.get(&node.path).copied().or(source);
    if node.value_type == JsonValueType::Object
        && let Some(source) = source
    {
        for child in &node.children {
            if let Some(label) = child.key.as_deref().filter(|key| is_reference_key(key)) {
                let mut references = Vec::new();
                collect_reference_values(child, &mut references);
                for reference in references {
                    if let Some(targets) = aliases.get(&reference)
                        && let [target] = targets.as_slice()
                    {
                        edges.insert((source, *target, label.to_string()));
                    }
                }
            }
        }
    }

    for child in &node.children {
        collect_graph_edges(child, source, nodes_by_path, aliases, edges);
    }
}

fn collect_reference_values(node: &JsonNode, references: &mut Vec<String>) {
    match node.value_type {
        JsonValueType::Comment => {}
        JsonValueType::Metadata => {
            for child in &node.children {
                if child.value_type != JsonValueType::Comment {
                    collect_reference_values(child, references);
                }
            }
        }
        JsonValueType::String => {
            if let Ok(value) = serde_json::from_str::<String>(&node.display_value) {
                references.push(value);
            }
        }
        JsonValueType::Number | JsonValueType::Float | JsonValueType::Bool => {
            references.push(node.display_value.clone());
        }
        JsonValueType::Array => {
            for child in &node.children {
                collect_reference_values(child, references);
            }
        }
        JsonValueType::Object => {
            for child in &node.children {
                if child
                    .key
                    .as_deref()
                    .is_some_and(|key| matches!(key, "id" | "_id" | "$id" | "$ref"))
                {
                    collect_reference_values(child, references);
                }
            }
        }
        JsonValueType::DateTime | JsonValueType::Null => {}
    }
}

fn object_scalar(node: &JsonNode, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|candidate| {
        node.children.iter().find_map(|child| {
            child
                .key
                .as_deref()
                .filter(|key| key.eq_ignore_ascii_case(candidate))
                .and_then(|_| scalar_value(child))
        })
    })
}

fn object_child<'a>(node: &'a JsonNode, key: &str) -> Option<&'a JsonNode> {
    node.children.iter().find(|child| {
        child
            .key
            .as_deref()
            .is_some_and(|field| field.eq_ignore_ascii_case(key))
    })
}

fn scalar_value(node: &JsonNode) -> Option<String> {
    match node.value_type {
        JsonValueType::String => serde_json::from_str::<String>(&node.display_value).ok(),
        JsonValueType::Number | JsonValueType::Float | JsonValueType::Bool => {
            Some(node.display_value.clone())
        }
        JsonValueType::Metadata => node
            .children
            .iter()
            .find(|child| child.value_type != JsonValueType::Comment)
            .and_then(scalar_value),
        JsonValueType::DateTime
        | JsonValueType::Object
        | JsonValueType::Array
        | JsonValueType::Null
        | JsonValueType::Comment => None,
    }
}

fn is_reference_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if matches!(normalized.as_str(), "id" | "$id") || normalized.ends_with("count") {
        return false;
    }

    normalized.ends_with("id")
        || [
            "ref", "depend", "require", "parent", "child", "target", "source", "owner", "link",
            "relation", "use",
        ]
        .iter()
        .any(|part| normalized.contains(part))
}

fn append_json_pointer(parent: &str, segment: &str) -> String {
    let escaped = segment.replace('~', "~0").replace('/', "~1");
    format!("{parent}/{escaped}")
}
