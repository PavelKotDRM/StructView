use serde_json::{Map, Value};

use super::{EdgeDirection, JsonObject, edge_record, graph_document, set_edge_direction};

#[derive(Default)]
struct Flowchart {
    nodes: Vec<Value>,
    node_indices: std::collections::HashMap<String, usize>,
    edges: Vec<Value>,
    direction: Option<String>,
}

impl Flowchart {
    fn add_node(&mut self, expression: &str) -> Result<String, String> {
        let (id, label, shape) = parse_node(expression)?;
        let has_shape = shape.is_some();
        let shape = shape.or(Some("process"));
        if let Some(index) = self.node_indices.get(&id).copied() {
            let node = self.nodes[index]
                .as_object_mut()
                .ok_or("Invalid internal Mermaid node")?;
            let attributes = node
                .get_mut("attributes")
                .and_then(Value::as_object_mut)
                .ok_or("Invalid internal Mermaid node attributes")?;
            if !has_shape && label == id {
                return Ok(id);
            }
            attributes.insert("label".to_string(), Value::String(label.clone()));
            if let Some(shape) = shape {
                attributes.insert("shape".to_string(), Value::String(shape.to_string()));
            }
            node.insert("label".to_string(), Value::String(label));
            return Ok(id);
        }

        let mut attributes = JsonObject::new();
        attributes.insert("label".to_string(), Value::String(label.clone()));
        if let Some(shape) = shape {
            attributes.insert("shape".to_string(), Value::String(shape.to_string()));
        }
        let mut node = Map::new();
        node.insert("id".to_string(), Value::String(id.clone()));
        node.insert("label".to_string(), Value::String(label));
        node.insert("attributes".to_string(), Value::Object(attributes));
        self.node_indices.insert(id.clone(), self.nodes.len());
        self.nodes.push(Value::Object(node));
        Ok(id)
    }
}

pub(super) fn parse(input: &str) -> Result<Value, String> {
    let mut graph = Flowchart::default();
    for statement in input
        .lines()
        .flat_map(|line| line.split(';'))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
    {
        if statement.starts_with("flowchart ") || statement.starts_with("graph ") {
            let direction = statement.split_whitespace().nth(1).unwrap_or_default();
            if matches!(direction, "TB" | "TD" | "BT" | "RL" | "LR") {
                graph.direction = Some(direction.to_string());
            }
            continue;
        }
        if statement.starts_with("subgraph ") || statement == "end" {
            continue;
        }

        let Some((mut operator_index, mut operator)) = find_operator(statement, 0) else {
            graph.add_node(statement)?;
            continue;
        };
        let mut source = graph.add_node(&statement[..operator_index])?;
        loop {
            let cursor = operator_index + operator.len();
            let rest = &statement[cursor..];
            let (target_end, next_operator) = find_operator(rest, 0)
                .map_or((rest.len(), None), |(index, operator)| {
                    (index, Some((index, operator)))
                });
            let mut target_expression = rest[..target_end].trim();
            let mut label = None;
            if let Some(after_label) = target_expression.strip_prefix('|') {
                let label_end = after_label.find('|').ok_or_else(|| {
                    format!("Unclosed edge label in Mermaid flowchart: {statement}")
                })?;
                label = Some(after_label[..label_end].trim().to_string());
                target_expression = after_label[label_end + 1..].trim();
            }
            let target = graph.add_node(target_expression)?;
            let (edge_source, edge_target) = (source.clone(), target);
            let direction = match operator {
                "---" | "--" => EdgeDirection::Undirected,
                "<-->" => EdgeDirection::Bidirectional,
                "<--" | "<---" => EdgeDirection::Reverse,
                _ => EdgeDirection::Directed,
            };
            let mut attributes = JsonObject::new();
            if let Some(label) = label.filter(|label| !label.is_empty()) {
                attributes.insert("label".to_string(), Value::String(label));
            }
            let mut edge = edge_record(edge_source, edge_target, None, attributes);
            set_edge_direction(&mut edge, direction);
            graph.edges.push(edge);

            let Some((index, next)) = next_operator else {
                break;
            };
            source = graph.add_node(target_expression)?;
            operator_index = cursor + index;
            operator = next;
        }
    }
    if graph.nodes.is_empty() {
        return Err("Mermaid flowchart contains no nodes".to_string());
    }

    let mut graph_attributes = JsonObject::new();
    if let Some(direction) = graph.direction {
        graph_attributes.insert("orientation".to_string(), Value::String(direction));
    }
    let mut metadata = Map::new();
    metadata.insert("layout".to_string(), Value::String("flowchart".to_string()));
    Ok(graph_document(
        None,
        "flowchart_multigraph",
        "mermaid",
        graph_attributes,
        metadata,
        graph.nodes,
        graph.edges,
    ))
}

fn find_operator(input: &str, from: usize) -> Option<(usize, &'static str)> {
    const OPERATORS: [&str; 10] = [
        "<-->", "<---", "-.->", "-->", "==>", "---", "<--", "--o", "--x", "--",
    ];
    OPERATORS
        .iter()
        .filter_map(|operator| {
            input[from..]
                .find(operator)
                .map(|index| (from + index, *operator))
        })
        .min_by_key(|(index, operator)| (*index, std::cmp::Reverse(operator.len())))
}

fn parse_node(input: &str) -> Result<(String, String, Option<&'static str>), String> {
    let input = input.trim();
    let id_end = input
        .find(|character: char| character.is_whitespace() || "([{<".contains(character))
        .unwrap_or(input.len());
    let id = input[..id_end].trim();
    if id.is_empty()
        || !id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err(format!("Invalid Mermaid node identifier: {input}"));
    }
    let suffix = input[id_end..].trim();
    if suffix.is_empty() {
        return Ok((id.to_string(), id.to_string(), None));
    }
    let (label, shape) = match suffix {
        value if value.starts_with("(((") && value.ends_with(")))") => {
            (&value[3..value.len() - 3], "connector")
        }
        value if value.starts_with("((") && value.ends_with("))") => {
            (&value[2..value.len() - 2], "connector")
        }
        value if value.starts_with("{{") && value.ends_with("}}") => {
            (&value[2..value.len() - 2], "preparation")
        }
        value if value.starts_with("[[") && value.ends_with("]]") => {
            (&value[2..value.len() - 2], "subprocess")
        }
        value if value.starts_with("[(") && value.ends_with(")]") => {
            (&value[2..value.len() - 2], "database")
        }
        value if value.starts_with("([") && value.ends_with("])") => {
            (&value[2..value.len() - 2], "terminator")
        }
        value if value.starts_with('{') && value.ends_with('}') => {
            (&value[1..value.len() - 1], "decision")
        }
        value if value.starts_with('(') && value.ends_with(')') => {
            (&value[1..value.len() - 1], "terminator")
        }
        value if value.starts_with("[/") && value.ends_with("/]") => {
            (&value[2..value.len() - 2], "input_output")
        }
        value if value.starts_with("[\\") && value.ends_with("\\]") => {
            (&value[2..value.len() - 2], "input_output")
        }
        value if value.starts_with('[') && value.ends_with(']') => {
            (&value[1..value.len() - 1], "process")
        }
        value if value.starts_with('>') && value.ends_with(']') => {
            (&value[1..value.len() - 1], "input_output")
        }
        _ => return Err(format!("Unsupported Mermaid node shape: {input}")),
    };
    let label = label.trim();
    let label = label
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(label)
        .replace("\\\"", "\"");
    if label.is_empty() {
        return Err(format!("Mermaid node label must not be empty: {input}"));
    }
    Ok((id.to_string(), label, Some(shape)))
}
