use std::collections::HashMap;

use graphviz_rust::dot_structures::{
    Attribute, Edge, EdgeTy, Graph, GraphAttributes, Id, NodeId, Stmt, Subgraph, Vertex,
};
use serde_json::{Map, Value};

use super::{JsonObject, edge_record, graph_document, node_record};

#[derive(Clone, Default)]
struct DotDefaults {
    node: JsonObject,
    edge: JsonObject,
}

struct DotGraphBuilder {
    nodes: Vec<DotNode>,
    node_indices: HashMap<String, usize>,
    edges: Vec<Value>,
    graph_attributes: JsonObject,
    subgraphs: Vec<Value>,
}

struct DotNode {
    id: String,
    attributes: JsonObject,
}

impl DotGraphBuilder {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_indices: HashMap::new(),
            edges: Vec::new(),
            graph_attributes: JsonObject::new(),
            subgraphs: Vec::new(),
        }
    }

    fn process_statements(
        &mut self,
        statements: &[Stmt],
        defaults: &mut DotDefaults,
        graph_attributes: &mut JsonObject,
        members: &mut Vec<String>,
    ) {
        for statement in statements {
            match statement {
                Stmt::Node(node) => {
                    let id = dot_id(&node.id.0);
                    self.add_node(&id, &defaults.node, &node.attributes);
                    push_unique(members, id);
                }
                Stmt::Edge(edge) => {
                    self.process_edge(edge, defaults, members);
                }
                Stmt::Attribute(attribute) => {
                    insert_dot_attribute(graph_attributes, attribute);
                }
                Stmt::GAttribute(attributes) => match attributes {
                    GraphAttributes::Graph(attributes) => {
                        merge_dot_attributes(graph_attributes, attributes);
                    }
                    GraphAttributes::Node(attributes) => {
                        merge_dot_attributes(&mut defaults.node, attributes);
                    }
                    GraphAttributes::Edge(attributes) => {
                        merge_dot_attributes(&mut defaults.edge, attributes);
                    }
                },
                Stmt::Subgraph(subgraph) => {
                    for id in self.process_subgraph(subgraph, defaults) {
                        push_unique(members, id);
                    }
                }
            }
        }
    }

    fn process_subgraph(
        &mut self,
        subgraph: &Subgraph,
        parent_defaults: &DotDefaults,
    ) -> Vec<String> {
        let mut defaults = parent_defaults.clone();
        let mut attributes = JsonObject::new();
        let mut members = Vec::new();
        self.process_statements(
            &subgraph.stmts,
            &mut defaults,
            &mut attributes,
            &mut members,
        );

        let mut entry = Map::new();
        entry.insert("id".to_string(), Value::String(dot_id(&subgraph.id)));
        entry.insert(
            "nodes".to_string(),
            Value::Array(members.iter().cloned().map(Value::String).collect()),
        );
        entry.insert("attributes".to_string(), Value::Object(attributes));
        self.subgraphs.push(Value::Object(entry));
        members
    }

    fn process_edge(&mut self, edge: &Edge, defaults: &DotDefaults, members: &mut Vec<String>) {
        let vertices = match &edge.ty {
            EdgeTy::Pair(source, target) => vec![source, target],
            EdgeTy::Chain(vertices) => vertices.iter().collect(),
        };
        let groups = vertices
            .into_iter()
            .map(|vertex| self.vertex_nodes(vertex, defaults, members))
            .collect::<Vec<_>>();
        let mut attributes = defaults.edge.clone();
        merge_dot_attributes(&mut attributes, &edge.attributes);
        let edge_id = attributes
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string);

        for pair in groups.windows(2) {
            for source in &pair[0] {
                for target in &pair[1] {
                    self.edges.push(edge_record(
                        source.clone(),
                        target.clone(),
                        edge_id.clone(),
                        attributes.clone(),
                    ));
                }
            }
        }
    }

    fn vertex_nodes(
        &mut self,
        vertex: &Vertex,
        defaults: &DotDefaults,
        members: &mut Vec<String>,
    ) -> Vec<String> {
        let ids = match vertex {
            Vertex::N(node) => vec![self.add_node(&dot_node_id(node), &defaults.node, &[])],
            Vertex::S(subgraph) => self.process_subgraph(subgraph, defaults),
        };
        for id in &ids {
            push_unique(members, id.clone());
        }
        ids
    }

    fn add_node(&mut self, id: &str, defaults: &JsonObject, attributes: &[Attribute]) -> String {
        let index = if let Some(index) = self.node_indices.get(id) {
            *index
        } else {
            let index = self.nodes.len();
            self.nodes.push(DotNode {
                id: id.to_string(),
                attributes: defaults.clone(),
            });
            self.node_indices.insert(id.to_string(), index);
            index
        };

        merge_dot_attributes(&mut self.nodes[index].attributes, attributes);
        id.to_string()
    }
}

pub(super) fn parse(input: &str) -> Result<Value, String> {
    let graph = graphviz_rust::parse(input)
        .map_err(|error| format!("Не удалось разобрать Graphviz DOT: {error}"))?;
    let (name, directed, strict, statements) = match graph {
        Graph::Graph { id, strict, stmts } => (dot_id(&id), false, strict, stmts),
        Graph::DiGraph { id, strict, stmts } => (dot_id(&id), true, strict, stmts),
    };

    let mut builder = DotGraphBuilder::new();
    let mut defaults = DotDefaults::default();
    let mut members = Vec::new();
    let mut graph_attributes = std::mem::take(&mut builder.graph_attributes);
    builder.process_statements(
        &statements,
        &mut defaults,
        &mut graph_attributes,
        &mut members,
    );
    builder.graph_attributes = graph_attributes;

    let mut metadata = Map::new();
    metadata.insert("strict".to_string(), Value::Bool(strict));
    if !builder.subgraphs.is_empty() {
        metadata.insert("subgraphs".to_string(), Value::Array(builder.subgraphs));
    }
    Ok(graph_document(
        Some(name),
        if directed {
            "directed_multigraph"
        } else {
            "undirected_multigraph"
        },
        "dot",
        builder.graph_attributes,
        metadata,
        builder
            .nodes
            .into_iter()
            .map(|node| node_record(node.id, node.attributes))
            .collect(),
        builder.edges,
    ))
}

fn dot_node_id(node: &NodeId) -> String {
    dot_id(&node.0)
}

fn dot_id(id: &Id) -> String {
    match id {
        Id::Html(value) | Id::Escaped(value) | Id::Plain(value) | Id::Anonymous(value) => {
            value.clone()
        }
    }
}

fn merge_dot_attributes(target: &mut JsonObject, attributes: &[Attribute]) {
    for attribute in attributes {
        insert_dot_attribute(target, attribute);
    }
}

fn insert_dot_attribute(target: &mut JsonObject, attribute: &Attribute) {
    let key = dot_id(&attribute.0);
    let value = dot_id(&attribute.1);
    let value = if key.eq_ignore_ascii_case("weight") {
        serde_json::from_str::<Value>(&value)
            .ok()
            .filter(Value::is_number)
            .unwrap_or(Value::String(value))
    } else {
        Value::String(value)
    };
    target.insert(key, value);
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}
