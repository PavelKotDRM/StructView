use std::collections::{BTreeSet, HashMap};

use struct_view_core::parser::{JsonNode, JsonValueType};

/// Узел графа сущностей, найденный по идентификатору или определению схемы.
#[derive(Debug, Clone)]
pub(in crate::app) struct GraphNode {
    pub(in crate::app) id: String,
    pub(in crate::app) label: String,
    pub(in crate::app) path: String,
    pub(in crate::app) search_paths: Vec<String>,
}

/// Направленное ребро от сущности, содержащей ссылку, к её целевой сущности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct GraphEdge {
    pub(in crate::app) source: usize,
    pub(in crate::app) target: usize,
    pub(in crate::app) label: String,
}

/// Граф идентифицированных сущностей и распознанных ссылок между ними.
#[derive(Debug, Default)]
pub(in crate::app) struct RelationshipGraph {
    pub(in crate::app) nodes: Vec<GraphNode>,
    pub(in crate::app) edges: Vec<GraphEdge>,
}

/// Построить граф по полям `id`, `_id`, `$id`, `$ref` и именам зависимостей.
pub(in crate::app) fn build_relationship_graph(root: &JsonNode) -> RelationshipGraph {
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
        })
        .collect();
    graph
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
            });
            nodes_by_path.insert(node.path.clone(), index);
            add_graph_alias(aliases, id, index);
            if is_definition {
                add_graph_alias(aliases, pointer.to_string(), index);
            }
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
                        && *target != source
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
    node.children
        .iter()
        .find(|child| {
            child.key.as_deref().is_some_and(|key| {
                keys.iter()
                    .any(|candidate| key.eq_ignore_ascii_case(candidate))
            })
        })
        .and_then(scalar_value)
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
    if matches!(normalized.as_str(), "id" | "$id") {
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
