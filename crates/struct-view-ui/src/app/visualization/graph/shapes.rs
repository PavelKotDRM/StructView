//! Strict adapters for common node-link, adjacency, and Cytoscape documents.

use super::explicit::{edge_label_with_id, graph_format, record_direction};
use super::*;

fn items(node: &JsonNode) -> impl Iterator<Item = &JsonNode> {
    node.children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment)
}

fn field<'a>(node: &'a JsonNode, names: &[&str]) -> Result<Option<&'a JsonNode>, String> {
    let mut found = names.iter().filter_map(|name| object_child(node, name));
    let first = found.next();
    if found.next().is_some() {
        return Err(format!(
            "Ambiguous {} fields at {}",
            names.join("/"),
            node.path
        ));
    }
    Ok(first)
}

fn boolean(node: &JsonNode, name: &str) -> Result<Option<bool>, String> {
    object_child(node, name)
        .map(|value| {
            if value.value_type != JsonValueType::Bool {
                return Err(format!("{name} must be a boolean at {}", value.path));
            }
            match value.display_value.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("Invalid {name} at {}", value.path)),
            }
        })
        .transpose()
}

fn identity(node: &JsonNode) -> Result<String, String> {
    if !matches!(
        node.value_type,
        JsonValueType::String | JsonValueType::Number
    ) {
        return Err(format!(
            "Node ID must be a string or integer at {}",
            node.path
        ));
    }
    scalar_value(node).ok_or_else(|| format!("Invalid node ID at {}", node.path))
}

fn data(node: &JsonNode) -> Result<&JsonNode, String> {
    match object_child(node, "data") {
        Some(value) if value.value_type == JsonValueType::Object => Ok(value),
        Some(value) => Err(format!("Element data must be an object at {}", value.path)),
        None => Err(format!("Cytoscape element requires data at {}", node.path)),
    }
}

pub(super) fn build(root: &JsonNode) -> Result<Option<RelationshipGraph>, String> {
    if root.value_type != JsonValueType::Object {
        return Ok(None);
    }
    let metadata =
        object_child(root, "graph").filter(|node| node.value_type == JsonValueType::Object);
    let networkx = object_child(root, "directed").is_some()
        && object_child(root, "multigraph").is_some()
        && object_child(root, "nodes").is_some();
    let nested = metadata.filter(|node| {
        !networkx && {
            ["nodes", "edges", "links", "elements", "adjacency"]
                .iter()
                .any(|name| object_child(node, name).is_some())
        }
    });
    let container = nested.unwrap_or(root);
    if nested.is_some()
        && ["nodes", "edges", "links", "elements", "adjacency"]
            .iter()
            .any(|name| object_child(root, name).is_some())
    {
        return Err("Ambiguous graph containers at the root and inside graph".to_string());
    }
    let elements = object_child(container, "elements");
    let nodes = object_child(container, "nodes");
    let edge_fields = ["edges", "links"]
        .iter()
        .filter_map(|name| object_child(container, name))
        .collect::<Vec<_>>();
    let edges = edge_fields.first().copied();
    let adjacency = object_child(container, "adjacency");
    let adjacency_array = adjacency.filter(|value| value.value_type == JsonValueType::Array);
    let marked = object_scalar(metadata.unwrap_or(container), &["type"])
        .is_some_and(|kind| graph_format(&kind.to_ascii_lowercase()).is_some())
        || object_child(root, "directed").is_some()
        || object_child(container, "directed").is_some();
    let keyed_adjacency =
        adjacency.filter(|value| value.value_type == JsonValueType::Object && marked);
    if elements.is_none()
        && !(nodes.is_some() && (edges.is_some() || adjacency_array.is_some()))
        && keyed_adjacency.is_none()
    {
        if marked
            && (nodes.is_some() || edges.is_some() || adjacency.is_some())
            && object_child(container, "adjacency_matrix").is_none()
        {
            return Err(
                "Graph requires nodes and edges/links, or a valid adjacency container".to_string(),
            );
        }
        return Ok(None);
    }
    if !marked && elements.is_none() {
        let plausible_nodes = nodes.is_some_and(|nodes| {
            items(nodes).any(|node| {
                matches!(
                    node.value_type,
                    JsonValueType::String | JsonValueType::Number
                ) || object_scalar(node, &["id", "_id", "$id"]).is_some()
                    || nodes.value_type == JsonValueType::Object
            })
        });
        let plausible_edges = edges.is_some_and(|edges| {
            items(edges).any(|edge| {
                edge.value_type == JsonValueType::Array && edge.data_child_count() >= 2
                    || object_child(edge, "source").is_some()
                    || object_child(edge, "from").is_some()
            })
        });
        if !plausible_nodes || !plausible_edges {
            return Ok(None);
        }
    }
    if let Some(elements) = elements
        && !marked
    {
        let records = match elements.value_type {
            JsonValueType::Object => object_child(elements, "nodes")
                .map(|nodes| items(nodes).collect::<Vec<_>>())
                .unwrap_or_default(),
            JsonValueType::Array => items(elements).collect(),
            _ => return Ok(None),
        };
        if !records.iter().any(|node| {
            object_child(node, "data").is_some_and(|data| object_child(data, "id").is_some())
        }) {
            return Ok(None);
        }
    }
    if edge_fields.len() > 1 {
        return Err("Ambiguous edges/links fields".to_string());
    }
    if edges.is_some() && adjacency.is_some() {
        return Err("Ambiguous graph: both edge list and adjacency supplied".to_string());
    }
    if elements.is_some() && (nodes.is_some() || edges.is_some() || adjacency.is_some()) {
        return Err("Ambiguous Cytoscape elements and node-link fields".to_string());
    }
    let settings = if networkx {
        root
    } else {
        metadata.unwrap_or(container)
    };
    let kind = object_scalar(settings, &["type"]);
    let (mut direction, mut multigraph) = match kind.as_deref() {
        Some(kind) => graph_format(&kind.to_ascii_lowercase())
            .ok_or_else(|| format!("Unsupported graph type: {kind}"))?,
        None => (EdgeDirection::Directed, false),
    };
    for config in [root, settings] {
        if let Some(directed) = boolean(config, "directed")? {
            direction = EdgeDirection::from_directed(directed);
        }
        if let Some(multi) = boolean(config, "multigraph")? {
            multigraph = multi;
        }
        direction = record_direction(config, direction)?;
    }
    let unit = object_scalar(settings, &["weight_unit"]).unwrap_or_default();
    let mut builder = Builder {
        graph: RelationshipGraph {
            directed: direction != EdgeDirection::Undirected,
            ..Default::default()
        },
        ids: HashMap::new(),
        direction,
        multigraph,
        deduplicate_mirrors: adjacency.is_some(),
        unit,
        unique: HashMap::new(),
        adjacency_keys: HashMap::new(),
    };
    if let Some(elements) = elements {
        match elements.value_type {
            JsonValueType::Object => {
                let nodes = object_child(elements, "nodes").ok_or("elements.nodes is required")?;
                let edges = object_child(elements, "edges").ok_or("elements.edges is required")?;
                builder.nodes(nodes, true)?;
                builder.edges(edges, true)?;
            }
            JsonValueType::Array => {
                for element in items(elements) {
                    let record = data(element)?;
                    if object_child(record, "source").is_none()
                        && object_child(record, "target").is_none()
                    {
                        builder.node(record, None, element)?;
                    }
                }
                for element in items(elements) {
                    let record = data(element)?;
                    if object_child(record, "source").is_some()
                        || object_child(record, "target").is_some()
                    {
                        builder.edge(record, None, None, None, Some(element))?;
                    }
                }
            }
            _ => return Err("elements must be an object or array".to_string()),
        }
    } else if let Some(adjacency) = keyed_adjacency {
        builder.keyed_adjacency(adjacency, nodes)?;
    } else {
        builder.nodes(nodes.ok_or("nodes are required")?, false)?;
        if let Some(adjacency) = adjacency_array {
            if adjacency.data_child_count() != builder.graph.nodes.len() {
                return Err("NetworkX adjacency rows must match node count".to_string());
            }
            for (index, row) in items(adjacency).enumerate() {
                if row.value_type != JsonValueType::Array {
                    return Err(format!("Adjacency row must be an array at {}", row.path));
                }
                let source = builder.graph.nodes[index].id.clone();
                for neighbor in items(row) {
                    let target = field(neighbor, &["id", "target"])?.ok_or_else(|| {
                        format!("Adjacency entry requires id at {}", neighbor.path)
                    })?;
                    let target = identity(target)?;
                    builder.edge(neighbor, Some(&source), Some(&target), None, None)?;
                }
            }
        } else {
            builder.edges(edges.ok_or("edges or links are required")?, false)?;
        }
    }
    if !builder.multigraph
        && nodes.is_some_and(|nodes| nodes.value_type == JsonValueType::Array)
        && elements.is_none()
        && adjacency.is_none()
    {
        builder.graph.edges.sort_by(|left, right| {
            (left.source, left.target, &left.label, left.direction).cmp(&(
                right.source,
                right.target,
                &right.label,
                right.direction,
            ))
        });
    }
    Ok(Some(builder.graph))
}

struct Builder {
    graph: RelationshipGraph,
    ids: HashMap<String, usize>,
    direction: EdgeDirection,
    multigraph: bool,
    deduplicate_mirrors: bool,
    unit: String,
    unique: HashMap<(usize, usize, String, EdgeDirection), usize>,
    adjacency_keys: HashMap<(usize, usize, String), (String, EdgeDirection)>,
}

impl Builder {
    fn keyed_adjacency(
        &mut self,
        adjacency: &JsonNode,
        nodes: Option<&JsonNode>,
    ) -> Result<(), String> {
        if let Some(nodes) = nodes {
            self.nodes(nodes, false)?;
        }
        for entry in items(adjacency) {
            if entry.yaml_key.is_some() {
                return Err("Adjacency keys must be strings".to_string());
            }
            let id = entry.key.as_deref().ok_or("Adjacency source needs a key")?;
            if nodes.is_none() && !self.ids.contains_key(id) {
                self.implicit_node(id, entry);
            }
        }
        for entry in items(adjacency) {
            let source = entry.key.as_deref().ok_or("Adjacency source needs a key")?;
            if !self.ids.contains_key(source) {
                return Err(format!("Unknown adjacency source: {source}"));
            }
            if !matches!(
                entry.value_type,
                JsonValueType::Array | JsonValueType::Object
            ) {
                return Err(format!(
                    "Adjacency neighbors must be an array or object at {}",
                    entry.path
                ));
            }
            for neighbor in items(entry) {
                let target = if entry.value_type == JsonValueType::Object {
                    if neighbor.yaml_key.is_some() {
                        return Err("Adjacency keys must be strings".to_string());
                    }
                    neighbor.key.clone().ok_or("Adjacency target needs a key")?
                } else if neighbor.value_type == JsonValueType::Object {
                    identity(
                        field(neighbor, &["id", "target", "to"])?
                            .ok_or("Adjacency neighbor requires id/target")?,
                    )?
                } else {
                    identity(neighbor)?
                };
                if nodes.is_none() && !self.ids.contains_key(&target) {
                    self.implicit_node(&target, neighbor);
                }
                if neighbor.value_type == JsonValueType::Object {
                    self.edge(neighbor, Some(source), Some(&target), None, None)?;
                } else {
                    let source_index = *self.ids.get(source).ok_or("Unknown adjacency source")?;
                    let target_index = *self
                        .ids
                        .get(&target)
                        .ok_or_else(|| format!("Unknown adjacency target: {target}"))?;
                    let label = if entry.value_type == JsonValueType::Object {
                        let weight =
                            scalar_value(neighbor).ok_or("Adjacency weight must be scalar")?;
                        if self.unit.is_empty() {
                            weight
                        } else {
                            format!("{weight} {}", self.unit)
                        }
                    } else {
                        String::new()
                    };
                    self.push_edge(
                        source_index,
                        target_index,
                        label,
                        self.direction,
                        Vec::new(),
                    );
                }
            }
        }
        Ok(())
    }

    fn implicit_node(&mut self, id: &str, original: &JsonNode) {
        self.ids.insert(id.to_string(), self.graph.nodes.len());
        self.graph.nodes.push(GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            path: original.path.clone(),
            search_paths: vec![original.path.clone()],
            partition: None,
            attributes: Vec::new(),
        });
    }

    fn nodes(&mut self, nodes: &JsonNode, cytoscape: bool) -> Result<(), String> {
        if !matches!(
            nodes.value_type,
            JsonValueType::Array | JsonValueType::Object
        ) {
            return Err(format!(
                "nodes must be an array or object at {}",
                nodes.path
            ));
        }
        for node in items(nodes) {
            let record = if cytoscape { data(node)? } else { node };
            let key = if nodes.value_type == JsonValueType::Object {
                Some(node.key.as_deref().ok_or("Keyed node has no key")?)
            } else {
                None
            };
            self.node(record, key, node)?;
        }
        Ok(())
    }

    fn node(
        &mut self,
        record: &JsonNode,
        key: Option<&str>,
        original: &JsonNode,
    ) -> Result<(), String> {
        if record.yaml_key.is_some() {
            return Err(format!(
                "Graph dictionary keys must be strings at {}",
                record.path
            ));
        }
        if key.is_some()
            && record.value_type != JsonValueType::Object
            && scalar_value(record).is_none()
        {
            return Err(format!(
                "Node dictionary value must be a record or scalar at {}",
                record.path
            ));
        }
        let id = if record.value_type == JsonValueType::Object {
            let declared = field(record, &["id", "_id", "$id"])?
                .map(identity)
                .transpose()?;
            if let (Some(key), Some(declared)) = (key, declared.as_deref())
                && key != declared
            {
                return Err(format!(
                    "Node dictionary key {key} conflicts with ID {declared}"
                ));
            }
            declared
                .or_else(|| key.map(str::to_string))
                .ok_or_else(|| format!("Node requires ID at {}", record.path))?
        } else {
            match key {
                Some(key) => key.to_string(),
                None => identity(record)?,
            }
        };
        if self.ids.contains_key(&id) {
            return Err(format!("Duplicate node ID: {id}"));
        }
        let label = if record.value_type == JsonValueType::Object {
            object_scalar(record, &["label", "name", "title"]).unwrap_or_else(|| id.clone())
        } else {
            scalar_value(record).unwrap_or_else(|| id.clone())
        };
        self.ids.insert(id.clone(), self.graph.nodes.len());
        self.graph.nodes.push(GraphNode {
            id,
            label,
            path: original.path.clone(),
            partition: None,
            attributes: record_attributes(original),
            search_paths: std::iter::once(original.path.clone())
                .chain(std::iter::once(record.path.clone()))
                .chain(items(record).map(|child| child.path.clone()))
                .collect(),
        });
        Ok(())
    }

    fn edges(&mut self, edges: &JsonNode, cytoscape: bool) -> Result<(), String> {
        if !matches!(
            edges.value_type,
            JsonValueType::Array | JsonValueType::Object
        ) {
            return Err(format!(
                "edges must be an array or object at {}",
                edges.path
            ));
        }
        for edge in items(edges) {
            if edge.yaml_key.is_some() {
                return Err(format!(
                    "Edge dictionary keys must be strings at {}",
                    edge.path
                ));
            }
            let key = if edges.value_type == JsonValueType::Object {
                edge.key.as_deref()
            } else {
                None
            };
            self.edge(
                if cytoscape { data(edge)? } else { edge },
                None,
                None,
                key,
                cytoscape.then_some(edge),
            )?;
        }
        Ok(())
    }

    fn endpoint(&self, node: &JsonNode) -> Result<String, String> {
        if node.value_type == JsonValueType::Object {
            return identity(
                field(node, &["id", "_id", "$id"])?.ok_or("Endpoint object requires ID")?,
            );
        }
        identity(node)
    }

    fn edge(
        &mut self,
        edge: &JsonNode,
        source_override: Option<&str>,
        target_override: Option<&str>,
        dictionary_key: Option<&str>,
        original: Option<&JsonNode>,
    ) -> Result<(), String> {
        let (source, target, mut label, direction) = match edge.value_type {
            JsonValueType::Object => {
                let endpoint = |names: &[&str], provided: Option<&str>| -> Result<String, String> {
                    let declared = field(edge, names)?
                        .map(|value| self.endpoint(value))
                        .transpose()?;
                    if let Some(value) = provided {
                        if declared
                            .as_deref()
                            .is_some_and(|declared| declared != value)
                        {
                            return Err(format!("Conflicting adjacency endpoint at {}", edge.path));
                        }
                        return Ok(value.to_string());
                    }
                    declared.ok_or_else(|| format!("Edge requires {} at {}", names[0], edge.path))
                };
                (
                    endpoint(&["source", "from"], source_override)?,
                    endpoint(&["target", "to"], target_override)?,
                    self.edge_label(edge)?,
                    record_direction(edge, self.direction)?,
                )
            }
            JsonValueType::Array => {
                let values = items(edge).collect::<Vec<_>>();
                if !(2..=4).contains(&values.len()) {
                    return Err(format!("Edge tuple requires 2-4 items at {}", edge.path));
                }
                let attributes = values
                    .get(2)
                    .filter(|value| value.value_type == JsonValueType::Object);
                let label = if let Some(attributes) = attributes {
                    self.edge_label(attributes)?
                } else {
                    values
                        .get(2)
                        .map(|value| {
                            let label = scalar_value(value)
                                .ok_or("Edge tuple label/weight must be scalar")?;
                            Ok::<_, String>(
                                if !self.unit.is_empty()
                                    && matches!(
                                        value.value_type,
                                        JsonValueType::Number | JsonValueType::Float
                                    )
                                {
                                    format!("{label} {}", self.unit)
                                } else {
                                    label
                                },
                            )
                        })
                        .transpose()?
                        .unwrap_or_default()
                };
                let attribute_direction = match attributes {
                    Some(attributes) => record_direction(attributes, self.direction)?,
                    None => self.direction,
                };
                let tuple_direction = values
                    .get(3)
                    .map(|value| {
                        let text = scalar_value(value).ok_or("Tuple direction must be a string")?;
                        EdgeDirection::parse(&text)
                    })
                    .transpose()?;
                if let Some(direction) = tuple_direction
                    && attributes.is_some_and(|attributes| {
                        ["direction", "directed", "bidirectional"]
                            .iter()
                            .any(|name| object_child(attributes, name).is_some())
                            || object_child(attributes, "attributes")
                                .is_some_and(|attributes| object_child(attributes, "dir").is_some())
                    })
                    && direction != attribute_direction
                {
                    return Err(format!("Conflicting tuple directions at {}", edge.path));
                }
                let direction = tuple_direction.unwrap_or(attribute_direction);
                (
                    self.endpoint(values[0])?,
                    self.endpoint(values[1])?,
                    label,
                    direction,
                )
            }
            _ => return Err(format!("Edge must be an object or tuple at {}", edge.path)),
        };
        if let Some(key) = dictionary_key
            && let Some(declared) = object_scalar(edge, &["id", "edge_id"])
            && declared != key
        {
            return Err(format!(
                "Edge dictionary key {key} conflicts with ID {declared}"
            ));
        }
        if let Some(key) = dictionary_key
            && object_child(edge, "id").is_none()
            && object_child(edge, "edge_id").is_none()
        {
            if !label.is_empty() {
                label.push_str(" · ");
            }
            label.push_str(key);
        }
        let source = *self
            .ids
            .get(&source)
            .ok_or_else(|| format!("Unknown source node: {source} at {}", edge.path))?;
        let target = *self
            .ids
            .get(&target)
            .ok_or_else(|| format!("Unknown target node: {target} at {}", edge.path))?;
        if self.multigraph && self.deduplicate_mirrors && direction == EdgeDirection::Undirected {
            let key = identity(
                object_child(edge, "key").ok_or("Multigraph adjacency requires an edge key")?,
            )?;
            let pair = (source.min(target), source.max(target), key);
            let value = (label.clone(), direction);
            if let Some(previous) = self.adjacency_keys.get(&pair)
                && previous != &value
            {
                return Err(format!(
                    "Conflicting mirrored adjacency edge at {}",
                    edge.path
                ));
            }
            self.adjacency_keys.insert(pair, value);
        }
        let attributes = if let Some(original) = original {
            record_attributes(original)
        } else if edge.value_type == JsonValueType::Array {
            items(edge)
                .nth(2)
                .map(record_attributes)
                .unwrap_or_default()
        } else {
            record_attributes(edge)
        };
        self.push_edge(source, target, label, direction, attributes);
        Ok(())
    }

    fn push_edge(
        &mut self,
        source: usize,
        target: usize,
        label: String,
        direction: EdgeDirection,
        attributes: Vec<(String, String)>,
    ) {
        let (source, target) = if direction == EdgeDirection::Undirected && source > target {
            (target, source)
        } else {
            (source, target)
        };
        let key = (source, target, label.clone(), direction);
        let keep_parallel = self.multigraph
            && !(self.deduplicate_mirrors && direction == EdgeDirection::Undirected);
        if !keep_parallel && let Some(&index) = self.unique.get(&key) {
            let edge = &mut self.graph.edges[index];
            for attribute in attributes {
                if !edge.attributes.contains(&attribute) {
                    edge.attributes.push(attribute);
                }
            }
            return;
        }
        self.unique.insert(key, self.graph.edges.len());
        self.graph.edges.push(GraphEdge {
            source,
            target,
            label,
            direction,
            attributes,
        });
    }

    fn edge_label(&self, edge: &JsonNode) -> Result<String, String> {
        boolean(edge, "directed")?;
        for name in ["weight", "value"] {
            if let Some(value) = object_child(edge, name)
                && scalar_value(value).is_none()
            {
                return Err(format!("Edge {name} must be scalar at {}", value.path));
            }
        }
        let mut label = edge_label_with_id(edge, &self.unit, !self.deduplicate_mirrors);
        if let Some(key) = object_child(edge, "key") {
            let key = identity(key)?;
            if !label.is_empty() {
                label.push_str(" · ");
            }
            label.push_str(&key);
        } else if self.multigraph
            && self.deduplicate_mirrors
            && self.direction == EdgeDirection::Undirected
        {
            return Err(format!(
                "Undirected multigraph adjacency requires an edge key at {}",
                edge.path
            ));
        }
        Ok(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use struct_view_core::parser::{DataFormat, parse_data, serialize_node};

    fn graph(source: &str) -> RelationshipGraph {
        let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
        super::super::try_build_relationship_graph(&root).unwrap()
    }

    #[test]
    fn adapters_work_in_every_writable_format() {
        let cases = [
            (
                r#"{"directed":false,"multigraph":true,"graph":{},"nodes":[{"id":1,"label":"First"},{"id":2}],"links":[{"source":1,"target":2,"key":0,"weight":3},{"source":{"id":1},"target":{"id":2},"key":1,"direction":"bidirectional"},{"source":2,"target":2,"key":2,"direction":"reverse"}]}"#,
                "nodes[0]",
            ),
            (
                r#"{"graph":{"type":"mixed_multigraph","nodes":{"a":{"label":"First"},"b":{"name":"Second"}},"links":{"e1":{"from":"a","to":"b","weight":3},"e2":{"source":"a","target":"b","direction":"bidirectional"},"loop":{"source":"b","target":"b","direction":"reverse"}}}}"#,
                "graph.nodes.a",
            ),
            (
                r#"{"elements":{"nodes":[{"data":{"id":"a","label":"First"}},{"data":{"id":"b"}}],"edges":[{"data":{"id":"e1","source":"a","target":"b","weight":3}},{"data":{"id":"e2","source":"a","target":"b","direction":"bidirectional"}},{"data":{"id":"loop","source":"b","target":"b","direction":"reverse"}}]}}"#,
                "elements.nodes[0]",
            ),
            (
                r#"{"elements":[{"data":{"source":"a","target":"b","weight":3}},{"data":{"id":"a","label":"First"}},{"data":{"id":"b"}},{"data":{"source":"a","target":"b","direction":"bidirectional"}},{"data":{"source":"b","target":"b","direction":"reverse"}}]}"#,
                "elements[1]",
            ),
            (
                r#"{"directed":true,"multigraph":true,"nodes":["a","b"],"edges":[["a","b",3],["a","b","two","bidirectional"],["b","b","loop","reverse"]]}"#,
                "nodes[0]",
            ),
            (
                r#"{"directed":true,"multigraph":true,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","weight":3},{"id":"b","direction":"bidirectional"}],[{"id":"b","direction":"reverse"}]]}"#,
                "nodes[0]",
            ),
            (
                r#"{"graph":{"type":"mixed_multigraph"},"nodes":{"a":"First","b":"Second"},"adjacency":{"a":[{"target":"b","weight":3},{"target":"b","direction":"bidirectional"}],"b":[{"target":"b","direction":"reverse"}]}}"#,
                "nodes.a",
            ),
        ];
        for (source, path) in cases {
            let expected = graph(source);
            assert_eq!(expected.nodes.len(), 2, "{source}");
            assert_eq!(expected.edges.len(), 3, "{source}");
            assert!(
                expected
                    .edges
                    .iter()
                    .any(|edge| edge.source == edge.target
                        && edge.direction == EdgeDirection::Reverse)
            );
            assert!(
                expected
                    .edges
                    .iter()
                    .any(|edge| edge.direction == EdgeDirection::Bidirectional)
            );
            assert!(expected.edges.iter().any(|edge| edge.label.contains('3')));
            assert_eq!(expected.nodes[0].path, path);
            let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
            for format in DataFormat::ALL {
                let text = serialize_node(&root, format, false).unwrap();
                let root = parse_data(&text, Some(format)).unwrap().0;
                let actual = super::super::try_build_relationship_graph(&root).unwrap();
                assert_eq!(actual.edges, expected.edges, "{format:?}: {text}");
                assert_eq!(actual.nodes.len(), expected.nodes.len());
                for (actual, expected) in actual.nodes.iter().zip(&expected.nodes) {
                    assert_eq!(actual.id, expected.id);
                    assert_eq!(actual.label, expected.label);
                    assert_eq!(actual.path, expected.path);
                    assert!(actual.search_paths.contains(&actual.path));
                }
            }
        }
    }

    #[test]
    fn native_yaml_and_toml_keep_keyed_edges_and_weights() {
        for (format, source) in [
            (
                DataFormat::Yaml,
                "graph:\n  type: mixed_multigraph\n  weight_unit: ms\nnodes:\n  a: Alpha\n  b: Beta\nlinks:\n  ab:\n    source: a\n    target: b\n    weight: 2\n    direction: bidirectional\n",
            ),
            (
                DataFormat::Toml,
                "[graph]\ntype = 'mixed_multigraph'\nweight_unit = 'ms'\n[nodes]\na = 'Alpha'\nb = 'Beta'\n[links.ab]\nsource = 'a'\ntarget = 'b'\nweight = 2\ndirection = 'bidirectional'\n",
            ),
        ] {
            let root = parse_data(source, Some(format)).unwrap().0;
            let actual = super::super::try_build_relationship_graph(&root).unwrap();
            assert_eq!(actual.nodes[0].label, "Alpha");
            assert_eq!(actual.edges[0].label, "2 ms · ab");
            assert_eq!(actual.edges[0].direction, EdgeDirection::Bidirectional);
        }
    }

    #[test]
    fn tuples_and_weighted_adjacency_keep_isolated_and_incoming_nodes() {
        let actual = graph(
            r#"{"directed":true,"nodes":["a","b","isolated"],"edges":[["a","b"],["b","b",{"weight":2,"direction":"reverse"}]]}"#,
        );
        assert_eq!(actual.nodes.len(), 3);
        assert_eq!(actual.edges[1].label, "2");
        assert_eq!(actual.edges[1].direction, EdgeDirection::Reverse);
        let actual = graph(
            r#"{"graph":{"type":"weighted_undirected"},"adjacency":{"a":{"b":2,"a":3},"isolated":[]}}"#,
        );
        assert_eq!(actual.nodes.len(), 3);
        assert_eq!(actual.edges.len(), 2);
        assert!(actual.nodes.iter().any(|node| node.id == "b"));
    }

    #[test]
    fn networkx_undirected_adjacency_deduplicates_mirrors_not_parallel_keys() {
        let actual = graph(
            r#"{"directed":false,"multigraph":true,"graph":{},"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"weight":2},{"id":"b","key":1,"weight":2},{"id":"a","key":0}],[{"id":"a","key":0,"weight":2},{"id":"a","key":1,"weight":2}]]}"#,
        );
        assert_eq!(actual.edges.len(), 3);
        assert_eq!(actual.edges[0].label, "2 · 0");
        assert_eq!(actual.edges[1].label, "2 · 1");
        let actual = graph(
            r#"{"directed":false,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b"}],[{"id":"a"}]]}"#,
        );
        assert_eq!(actual.edges.len(), 1);
    }

    #[test]
    fn malformed_graphs_report_errors_instead_of_losing_edges() {
        for source in [
            r#"{"directed":true,"nodes":["a","a"],"edges":[]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[["a","missing"]]}"#,
            r#"{"directed":true,"nodes":{"a":{"id":"b"}},"edges":[]}"#,
            r#"{"directed":true,"nodes":{"a":[]},"edges":[]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[["a"]]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[["a","a",1,"invalid"]]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","from":"a","target":"a"}]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[],"links":[]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[],"adjacency":[[]]}"#,
            r#"{"directed":"true","nodes":["a"],"edges":[]}"#,
            r#"{"directed":true,"multigraph":"true","nodes":["a"],"edges":[]}"#,
            r#"{"directed":true,"nodes":["a"],"adjacency":[]}"#,
            r#"{"directed":true,"nodes":["a"],"adjacency":[[{"id":"missing"}]]}"#,
            r#"{"directed":true,"nodes":["a"],"adjacency":{"a":["missing"]}}"#,
            r#"{"directed":true,"nodes":["a"],"adjacency":{"a":[{"target":"a","source":"other"}]}}"#,
            r#"{"directed":true,"nodes":["a"]}"#,
            r#"{"directed":true,"nodes":["a"],"adjacency":false}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","target":"a","directed":"true"}]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[{"source":"a","target":"a","weight":{}}]}"#,
            r#"{"directed":true,"nodes":["a"],"edges":{"e1":{"id":"e2","source":"a","target":"a"}}}"#,
            r#"{"directed":true,"nodes":["a"],"edges":[["a","a",{"direction":"reverse"},"bidirectional"]]}"#,
            r#"{"directed":false,"multigraph":true,"nodes":[{"id":"a"},{"id":"b"}],"adjacency":[[{"id":"b","key":0,"weight":2}],[{"id":"a","key":0,"weight":3}]]}"#,
            r#"{"graph":{"type":"directed"},"adjacency":{"a":false}}"#,
            r#"{"elements":[{"data":{"id":"a"}},{"data":{"source":"a","target":"missing"}}]}"#,
            r#"{"elements":[{"data":{"id":"a"}},{"data":"invalid"}]}"#,
        ] {
            let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
            assert!(
                super::super::try_build_relationship_graph(&root).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn ordinary_documents_are_not_claimed_by_the_adapter() {
        for source in [
            r#"{"nodes":[{"name":"chapter"}],"edges":["margin"],"links":["website"]}"#,
            r#"{"elements":["water","air"]}"#,
            r#"{"elements":{"nodes":[],"edges":[]}}"#,
            r#"{"nodes":[],"edges":[]}"#,
            r#"{"nodes":["chapter"],"links":["https://example.test"]}"#,
        ] {
            let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
            assert!(build(&root).unwrap().is_none(), "{source}");
            assert!(
                super::super::try_build_relationship_graph(&root).is_ok(),
                "{source}"
            );
        }
    }

    #[test]
    fn networkx_graph_attributes_are_not_nested_graph_config() {
        let actual = graph(
            r#"{"directed":false,"multigraph":false,"graph":{"type":"domain-specific","nodes":"attribute","direction":"north"},"nodes":[{"id":"a"},{"id":"b"}],"links":[{"source":"a","target":"b"}]}"#,
        );
        assert_eq!(actual.nodes.len(), 2);
        assert_eq!(actual.edges[0].direction, EdgeDirection::Undirected);
    }
}
