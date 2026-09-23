use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde_json::Value;

use crate::parser::{JsonNode, JsonValueType, build_path, node_to_value};
use crate::search::SearchState;

/// Представление, выбранное для открытого документа или сравнения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisualizationMode {
    #[default]
    Tree,
    Graph,
    Table,
    Schema,
    Comparison,
    Diff,
}

/// Кэш вычисляемых представлений документа.
#[derive(Debug, Default)]
pub(super) struct VisualizationCache {
    pub(super) table: Option<TableData>,
    pub(super) graph: Option<RelationshipGraph>,
    pub(super) schema: Option<Result<SchemaDiagram, String>>,
}

/// Строка развёрнутой таблицы значений.
#[derive(Debug, Clone)]
pub(super) struct TableRow {
    pub(super) path: String,
    pub(super) value: String,
    pub(super) value_type: JsonValueType,
}

/// Данные таблицы вместе с индексом для быстрого применения поиска.
#[derive(Debug, Default)]
pub(super) struct TableData {
    pub(super) rows: Vec<TableRow>,
    path_indices: HashMap<String, usize>,
}

/// Построить строки таблицы, включая контейнеры и пустые значения.
pub(super) fn build_table(root: &JsonNode) -> TableData {
    let mut table = TableData::default();
    collect_table_rows(root, None, &mut table);
    table
}

fn collect_table_rows(
    node: &JsonNode,
    parent: Option<(&str, &JsonValueType)>,
    table: &mut TableData,
) {
    let path = match parent {
        None => "$".to_string(),
        Some((parent_path, parent_type)) => {
            let is_index = *parent_type == JsonValueType::Array;
            build_path(parent_path, &node.key, is_index)
        }
    };

    let index = table.rows.len();
    table.path_indices.insert(node.path.clone(), index);
    table.rows.push(TableRow {
        path: path.clone(),
        value: node.display_value.clone(),
        value_type: node.value_type.clone(),
    });

    for child in &node.children {
        collect_table_rows(child, Some((&path, &node.value_type)), table);
    }
}

/// Вернуть индексы строк, совпавших с результатами поиска по дереву.
pub(super) fn table_visible_indices(table: &TableData, search: &SearchState) -> Option<Vec<usize>> {
    if search.query.is_empty() {
        return None;
    }

    Some(
        search
            .matches
            .iter()
            .filter_map(|path| table.path_indices.get(path).copied())
            .collect(),
    )
}

/// Экспортировать текущие строки таблицы в CSV с учётом активного поиска.
pub(super) fn table_to_csv(table: &TableData, search: &SearchState) -> String {
    let mut output = String::from("path,value,type\r\n");
    if let Some(indices) = table_visible_indices(table, search) {
        for index in indices {
            append_csv_row(&mut output, &table.rows[index]);
        }
    } else {
        for row in &table.rows {
            append_csv_row(&mut output, row);
        }
    }
    output
}

fn append_csv_row(output: &mut String, row: &TableRow) {
    output.push_str(&csv_field(&row.path));
    output.push(',');
    output.push_str(&csv_field(&row.value));
    output.push(',');
    output.push_str(&csv_field(value_type_name(&row.value_type)));
    output.push_str("\r\n");
}

fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\r' | '\n'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn value_type_name(value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::Object => "object",
        JsonValueType::Array => "array",
        JsonValueType::String => "string",
        JsonValueType::DateTime => "datetime",
        JsonValueType::Number => "number",
        JsonValueType::Float => "float",
        JsonValueType::Bool => "boolean",
        JsonValueType::Null => "null",
    }
}

/// Узел графа сущностей, найденный по идентификатору или определению схемы.
#[derive(Debug, Clone)]
pub(super) struct GraphNode {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) path: String,
    pub(super) search_paths: Vec<String>,
}

/// Направленное ребро от сущности, содержащей ссылку, к её целевой сущности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphEdge {
    pub(super) source: usize,
    pub(super) target: usize,
    pub(super) label: String,
}

/// Граф идентифицированных сущностей и распознанных ссылок между ними.
#[derive(Debug, Default)]
pub(super) struct RelationshipGraph {
    pub(super) nodes: Vec<GraphNode>,
    pub(super) edges: Vec<GraphEdge>,
}

/// Построить граф по полям `id`, `_id`, `$id`, `$ref` и именам зависимостей.
pub(super) fn build_relationship_graph(root: &JsonNode) -> RelationshipGraph {
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
        .and_then(|child| match child.value_type {
            JsonValueType::String => serde_json::from_str::<String>(&child.display_value).ok(),
            JsonValueType::Number | JsonValueType::Float | JsonValueType::Bool => {
                Some(child.display_value.clone())
            }
            JsonValueType::DateTime
            | JsonValueType::Object
            | JsonValueType::Array
            | JsonValueType::Null => None,
        })
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

/// Источник сведений, показанный в диаграмме схемы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SchemaSource {
    JsonSchema,
    OpenApi,
    Inferred,
}

/// Описание одного поля в ожидаемой или выведенной структуре.
#[derive(Debug, Clone)]
pub(super) struct SchemaRow {
    pub(super) path: String,
    pub(super) type_name: String,
    pub(super) required: Option<bool>,
    pub(super) constraints: String,
    pub(super) reference: Option<String>,
}

/// Строковая схема, построенная из JSON Schema/OpenAPI или примера данных.
#[derive(Debug, Clone)]
pub(super) struct SchemaDiagram {
    pub(super) source: SchemaSource,
    pub(super) title: Option<String>,
    pub(super) rows: Vec<SchemaRow>,
}

/// Построить явную JSON Schema/OpenAPI схему либо вывести структуру из данных.
pub(super) fn build_schema_diagram(root: &JsonNode) -> Result<SchemaDiagram, String> {
    let value = node_to_value(root)?;
    Ok(schema_diagram(&value).unwrap_or_else(|| inferred_schema(root)))
}

fn schema_diagram(value: &Value) -> Option<SchemaDiagram> {
    if looks_like_json_schema(value) {
        let mut rows = Vec::new();
        collect_schema_rows(value, "$".to_string(), None, &mut rows);
        return Some(SchemaDiagram {
            source: SchemaSource::JsonSchema,
            title: value
                .get("title")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            rows,
        });
    }

    if let Some(diagram) = openapi_schema_diagram(value) {
        return Some(diagram);
    }

    None
}

fn looks_like_json_schema(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("$schema") || object.contains_key("$ref") {
        return true;
    }

    let has_schema_members = [
        "required",
        "additionalProperties",
        "patternProperties",
        "prefixItems",
        "items",
        "enum",
        "const",
        "format",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "contains",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "pattern",
        "uniqueItems",
        "minProperties",
        "maxProperties",
        "minContains",
        "maxContains",
        "propertyNames",
        "unevaluatedItems",
        "unevaluatedProperties",
        "dependentRequired",
        "dependentSchemas",
        "contentEncoding",
        "contentMediaType",
        "contentSchema",
        "readOnly",
        "writeOnly",
        "deprecated",
        "discriminator",
        "nullable",
        "xml",
        "oneOf",
        "anyOf",
        "allOf",
        "not",
        "if",
        "then",
        "else",
    ]
    .iter()
    .any(|key| object.contains_key(*key));
    let property_schemas = object.get("properties").and_then(Value::as_object);
    let has_properties = property_schemas.is_some();
    let properties_look_like_schema = property_schemas
        .is_some_and(|properties| properties.values().all(looks_like_schema_fragment));
    let has_known_type = object.get("type").is_some_and(|type_name| match type_name {
        Value::String(type_name) => matches!(
            type_name.as_str(),
            "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
        ),
        Value::Array(types) => types.iter().any(|type_name| {
            type_name.as_str().is_some_and(|type_name| {
                matches!(
                    type_name,
                    "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
                )
            })
        }),
        _ => false,
    });
    let has_definitions = ["definitions", "$defs"].iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_object)
            .is_some_and(|definitions| definitions.values().any(looks_like_schema_fragment))
    });
    let has_only_schema_keywords = object.keys().all(|key| {
        [
            "$schema",
            "$id",
            "$ref",
            "$defs",
            "definitions",
            "type",
            "title",
            "description",
            "default",
            "examples",
            "enum",
            "const",
            "format",
            "properties",
            "required",
            "additionalProperties",
            "patternProperties",
            "items",
            "prefixItems",
            "allOf",
            "anyOf",
            "oneOf",
            "not",
            "if",
            "then",
            "else",
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "multipleOf",
            "minLength",
            "maxLength",
            "pattern",
            "minItems",
            "maxItems",
            "uniqueItems",
            "minProperties",
            "maxProperties",
            "contains",
            "minContains",
            "maxContains",
            "propertyNames",
            "unevaluatedItems",
            "unevaluatedProperties",
            "dependentRequired",
            "dependentSchemas",
            "contentEncoding",
            "contentMediaType",
            "contentSchema",
            "readOnly",
            "writeOnly",
            "deprecated",
            "discriminator",
            "nullable",
            "xml",
        ]
        .contains(&key.as_str())
    });

    (has_properties && (has_schema_members || has_known_type || properties_look_like_schema))
        || (has_only_schema_keywords && (has_known_type || has_schema_members))
        || has_definitions
}

fn looks_like_schema_fragment(value: &Value) -> bool {
    if value.is_boolean() {
        return true;
    }
    value.as_object().is_some_and(|object| {
        object.is_empty()
            || [
                "$ref",
                "type",
                "properties",
                "items",
                "enum",
                "const",
                "allOf",
                "oneOf",
                "anyOf",
            ]
            .iter()
            .any(|key| object.contains_key(*key))
    })
}

fn openapi_schema_diagram(value: &Value) -> Option<SchemaDiagram> {
    let object = value.as_object()?;
    if !object.contains_key("openapi") && !object.contains_key("swagger") {
        return None;
    }

    let mut rows = Vec::new();
    if let Some(schemas) = value
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
    {
        for (name, schema) in schemas {
            let path = build_path("$.components.schemas", &Some(name.clone()), false);
            collect_schema_rows(schema, path, None, &mut rows);
        }
    }
    if let Some(paths) = value.get("paths") {
        collect_openapi_inline_schemas(paths, "$.paths", &mut rows);
    }

    Some(SchemaDiagram {
        source: SchemaSource::OpenApi,
        title: value
            .pointer("/info/title")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        rows,
    })
}

fn collect_openapi_inline_schemas(value: &Value, path: &str, rows: &mut Vec<SchemaRow>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = build_path(path, &Some(key.clone()), false);
                if key == "schema" && looks_like_schema_fragment(child) {
                    collect_schema_rows(child, child_path, None, rows);
                } else {
                    collect_openapi_inline_schemas(child, &child_path, rows);
                }
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_openapi_inline_schemas(child, &format!("{path}[{index}]"), rows);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn collect_schema_rows(
    schema: &Value,
    path: String,
    required: Option<bool>,
    rows: &mut Vec<SchemaRow>,
) {
    let object = schema.as_object();
    let type_name = if schema == &Value::Bool(false) {
        "never".to_string()
    } else {
        object
            .and_then(|object| object.get("type"))
            .map(schema_type_name)
            .unwrap_or_else(|| {
                if object.is_some_and(|object| object.contains_key("properties")) {
                    "object".to_string()
                } else if object.is_some_and(|object| object.contains_key("items")) {
                    "array".to_string()
                } else {
                    "any".to_string()
                }
            })
    };
    let constraints = object
        .map(|object| {
            [
                "required",
                "additionalProperties",
                "patternProperties",
                "prefixItems",
                "format",
                "enum",
                "const",
                "default",
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "multipleOf",
                "minLength",
                "maxLength",
                "pattern",
                "minItems",
                "maxItems",
                "uniqueItems",
                "minProperties",
                "maxProperties",
                "contains",
                "minContains",
                "maxContains",
                "propertyNames",
                "unevaluatedItems",
                "unevaluatedProperties",
                "dependentRequired",
                "dependentSchemas",
                "contentEncoding",
                "contentMediaType",
                "contentSchema",
                "allOf",
                "anyOf",
                "oneOf",
                "not",
                "if",
                "then",
                "else",
                "readOnly",
                "writeOnly",
                "deprecated",
                "discriminator",
                "nullable",
                "xml",
                "externalDocs",
                "description",
            ]
            .iter()
            .filter_map(|key| object.get(*key).map(|value| format!("{key}={value}")))
            .collect::<Vec<_>>()
            .join("; ")
        })
        .unwrap_or_default();
    let reference = object
        .and_then(|object| object.get("$ref"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    rows.push(SchemaRow {
        path: path.clone(),
        type_name,
        required,
        constraints,
        reference,
    });

    let Some(object) = object else {
        return;
    };
    let required_fields = object
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();

    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, property_schema) in properties {
            let property_path = build_path(&path, &Some(name.clone()), false);
            collect_schema_rows(
                property_schema,
                property_path,
                Some(required_fields.contains(name.as_str())),
                rows,
            );
        }
    }

    if let Some(items) = object.get("items") {
        if let Some(tuple_items) = items.as_array() {
            for (index, item_schema) in tuple_items.iter().enumerate() {
                collect_schema_rows(item_schema, format!("{path}[{index}]"), None, rows);
            }
        } else {
            collect_schema_rows(items, format!("{path}[*]"), None, rows);
        }
    }

    if let Some(prefix_items) = object.get("prefixItems").and_then(Value::as_array) {
        for (index, item_schema) in prefix_items.iter().enumerate() {
            collect_schema_rows(item_schema, format!("{path}[{index}]"), None, rows);
        }
    }

    if let Some(additional) = object
        .get("additionalProperties")
        .filter(|value| value.is_object())
    {
        collect_schema_rows(additional, format!("{path}{{*}}"), None, rows);
    }

    if let Some(property_names) = object.get("propertyNames") {
        collect_schema_rows(property_names, format!("{path}{{key}}"), None, rows);
    }

    if let Some(patterns) = object.get("patternProperties").and_then(Value::as_object) {
        for (pattern, pattern_schema) in patterns {
            collect_schema_rows(
                pattern_schema,
                format!("{path}[pattern={pattern}]"),
                None,
                rows,
            );
        }
    }

    if let Some(contains) = object.get("contains") {
        collect_schema_rows(contains, format!("{path}[*]"), None, rows);
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(schemas) = object.get(keyword).and_then(Value::as_array) {
            for (index, child_schema) in schemas.iter().enumerate() {
                collect_schema_rows(
                    child_schema,
                    format!("{path}.{keyword}[{index}]"),
                    None,
                    rows,
                );
            }
        }
    }

    for keyword in ["not", "if", "then", "else"] {
        if let Some(child_schema) = object.get(keyword) {
            collect_schema_rows(child_schema, format!("{path}.{keyword}"), None, rows);
        }
    }

    if let Some(dependent_schemas) = object.get("dependentSchemas").and_then(Value::as_object) {
        for (name, child_schema) in dependent_schemas {
            let child_path = build_path(
                &build_path(&path, &Some("dependentSchemas".to_string()), false),
                &Some(name.clone()),
                false,
            );
            collect_schema_rows(child_schema, child_path, None, rows);
        }
    }

    for keyword in ["definitions", "$defs"] {
        if let Some(definitions) = object.get(keyword).and_then(Value::as_object) {
            for (name, definition) in definitions {
                let definition_path = build_path(
                    &build_path(&path, &Some(keyword.to_string()), false),
                    &Some(name.clone()),
                    false,
                );
                collect_schema_rows(definition, definition_path, None, rows);
            }
        }
    }
}

fn schema_type_name(value: &Value) -> String {
    match value {
        Value::String(type_name) => type_name.clone(),
        Value::Array(types) => types
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "any".to_string(),
    }
}

#[derive(Default)]
struct InferredRow {
    types: BTreeSet<String>,
    occurrences: usize,
    parent_path: Option<String>,
}

fn inferred_schema(root: &JsonNode) -> SchemaDiagram {
    let mut rows = BTreeMap::new();
    let mut object_counts = HashMap::new();
    collect_inferred_rows(root, "$", None, &mut rows, &mut object_counts);

    let rows = rows
        .into_iter()
        .map(|(path, row)| {
            let required = row.parent_path.as_ref().and_then(|parent| {
                object_counts
                    .get(parent)
                    .map(|count| row.occurrences == *count)
            });
            let constraints = row
                .parent_path
                .as_ref()
                .and_then(|parent| object_counts.get(parent))
                .map(|count| format!("observed in {}/{} sample object(s)", row.occurrences, count))
                .unwrap_or_default();
            SchemaRow {
                path,
                type_name: row.types.into_iter().collect::<Vec<_>>().join(" | "),
                required,
                constraints,
                reference: None,
            }
        })
        .collect();

    SchemaDiagram {
        source: SchemaSource::Inferred,
        title: None,
        rows,
    }
}

fn collect_inferred_rows(
    node: &JsonNode,
    path: &str,
    parent_path: Option<&str>,
    rows: &mut BTreeMap<String, InferredRow>,
    object_counts: &mut HashMap<String, usize>,
) {
    let row = rows.entry(path.to_string()).or_default();
    row.types
        .insert(schema_node_type(&node.value_type).to_string());
    if let Some(parent_path) = parent_path {
        row.occurrences += 1;
        row.parent_path = Some(parent_path.to_string());
    }

    match node.value_type {
        JsonValueType::Object => {
            *object_counts.entry(path.to_string()).or_default() += 1;
            for child in &node.children {
                let child_path = build_path(path, &child.key, false);
                collect_inferred_rows(child, &child_path, Some(path), rows, object_counts);
            }
        }
        JsonValueType::Array => {
            for child in &node.children {
                let child_path = format!("{path}[*]");
                collect_inferred_rows(child, &child_path, None, rows, object_counts);
            }
        }
        JsonValueType::String
        | JsonValueType::DateTime
        | JsonValueType::Number
        | JsonValueType::Float
        | JsonValueType::Bool
        | JsonValueType::Null => {}
    }
}

fn schema_node_type(value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::Object => "object",
        JsonValueType::Array => "array",
        JsonValueType::String => "string",
        JsonValueType::DateTime => "date-time",
        JsonValueType::Number => "number",
        JsonValueType::Float => "number",
        JsonValueType::Bool => "boolean",
        JsonValueType::Null => "null",
    }
}

/// Вернуть строки схемы, подходящие под текущий поисковый запрос.
pub(super) fn schema_visible_indices(
    diagram: &SchemaDiagram,
    search: &SearchState,
) -> Option<Vec<usize>> {
    if search.query.is_empty() {
        return None;
    }

    Some(
        diagram
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let key_match = search.options.search_keys
                    && text_matches(
                        &row.path,
                        &search.query,
                        search.options.case_sensitive,
                        search.options.exact_match,
                    );
                let value_match = search.options.search_values
                    && [
                        row.type_name.as_str(),
                        row.constraints.as_str(),
                        row.reference.as_deref().unwrap_or_default(),
                    ]
                    .iter()
                    .any(|text| {
                        text_matches(
                            text,
                            &search.query,
                            search.options.case_sensitive,
                            search.options.exact_match,
                        )
                    });
                (key_match || value_match).then_some(index)
            })
            .collect(),
    )
}

fn text_matches(text: &str, query: &str, case_sensitive: bool, exact_match: bool) -> bool {
    let (text, query) = if case_sensitive {
        (text.to_string(), query.to_string())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    if exact_match {
        text == query
    } else {
        text.contains(&query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parser::{DataFormat, parse_data};

    #[test]
    fn table_paths_and_csv_preserve_nested_fields() {
        let root = parse_data(
            r#"["line, one",{"quoted\"key":true}]"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let table = build_table(&root);

        assert_eq!(
            table
                .rows
                .iter()
                .map(|row| row.path.as_str())
                .collect::<Vec<_>>(),
            ["$", "$[0]", "$[1]", "$[1][\"quoted\\\"key\"]"]
        );
        let csv = table_to_csv(&table, &SearchState::default());
        assert!(csv.starts_with("path,value,type\r\n"));
        assert!(csv.contains("\"line, one\""));
    }

    #[test]
    fn table_filter_uses_tree_search_matches() {
        let root = parse_data(r#"{"keep":1,"drop":2}"#, Some(DataFormat::Json))
            .unwrap()
            .0;
        let table = build_table(&root);
        let mut search = SearchState::default();
        search.search(&root, "keep");

        let visible = table_visible_indices(&table, &search);
        assert_eq!(
            visible
                .unwrap()
                .iter()
                .map(|index| table.rows[*index].path.as_str())
                .collect::<Vec<_>>(),
            ["$.keep"]
        );
    }

    #[test]
    fn csv_escapes_quotes_and_line_breaks() {
        assert_eq!(csv_field("a,\"b\"\nline"), "\"a,\"\"b\"\"\nline\"");
    }

    #[test]
    fn graph_resolves_entity_ids_and_dependency_fields() {
        let root = parse_data(
            r#"{"services":[{"id":"api","name":"API","depends_on":"db"},{"id":"db","name":"Database"}]}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let graph = build_relationship_graph(&root);

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(
            graph.edges,
            [GraphEdge {
                source: 0,
                target: 1,
                label: "depends_on".to_string(),
            }]
        );
    }

    #[test]
    fn graph_resolves_json_schema_references_and_skips_ambiguous_ids() {
        let root = parse_data(
            r##"{"$defs":{"User":{"type":"object"},"Pet":{"$ref":"#/$defs/User"}}}"##,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let graph = build_relationship_graph(&root);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].label, "$ref");

        let duplicate_ids = parse_data(
            r#"{"items":[{"id":"same"},{"id":"same"},{"id":"consumer","ref":"same"}]}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        assert!(build_relationship_graph(&duplicate_ids).edges.is_empty());
    }

    #[test]
    fn schema_view_reads_json_schema_requirements_and_constraints() {
        let root = parse_data(
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["name"],"properties":{"name":{"type":"string","minLength":1},"age":{"type":"integer"}}}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let diagram = build_schema_diagram(&root).unwrap();

        assert_eq!(diagram.source, SchemaSource::JsonSchema);
        let name = diagram
            .rows
            .iter()
            .find(|row| row.path == "$.name")
            .unwrap();
        assert_eq!(name.type_name, "string");
        assert_eq!(name.required, Some(true));
        assert!(name.constraints.contains("minLength=1"));
        let age = diagram.rows.iter().find(|row| row.path == "$.age").unwrap();
        assert_eq!(age.required, Some(false));
    }

    #[test]
    fn schema_view_recognizes_scalar_json_schemas_and_toml_date_types() {
        let scalar_schema = parse_data(r#"{"type":"string"}"#, Some(DataFormat::Json))
            .unwrap()
            .0;
        let schema = build_schema_diagram(&scalar_schema).unwrap();
        assert_eq!(schema.source, SchemaSource::JsonSchema);
        assert_eq!(schema.rows[0].type_name, "string");

        let toml_document = parse_data("created = 1979-05-27T07:32:00Z", Some(DataFormat::Toml))
            .unwrap()
            .0;
        let inferred = build_schema_diagram(&toml_document).unwrap();
        assert_eq!(inferred.source, SchemaSource::Inferred);
        let created = inferred
            .rows
            .iter()
            .find(|row| row.path == "$.created")
            .unwrap();
        assert_eq!(created.type_name, "date-time");
    }

    #[test]
    fn schema_view_recognizes_openapi_components_and_infers_sample_presence() {
        let openapi = parse_data(
            r##"{"openapi":"3.0.0","components":{"schemas":{"Pet":{"type":"object","properties":{"owner":{"$ref":"#/components/schemas/User"}}},"User":{"type":"object"}}},"paths":{"/pets":{"post":{"requestBody":{"content":{"application/json":{"schema":{"type":"object","required":["name"],"properties":{"name":{"type":"string"}}}}}}}}}}"##,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let diagram = build_schema_diagram(&openapi).unwrap();
        assert_eq!(diagram.source, SchemaSource::OpenApi);
        assert!(diagram.rows.iter().any(|row| {
            row.path == "$.components.schemas.Pet.owner"
                && row.reference.as_deref() == Some("#/components/schemas/User")
        }));
        assert!(diagram.rows.iter().any(|row| {
            row.path.contains("requestBody")
                && row.path.ends_with(".name")
                && row.required == Some(true)
        }));

        let samples = parse_data(
            r#"{"users":[{"id":1,"name":"Ada"},{"id":2}]}"#,
            Some(DataFormat::Json),
        )
        .unwrap()
        .0;
        let inferred = build_schema_diagram(&samples).unwrap();
        assert_eq!(inferred.source, SchemaSource::Inferred);
        let name = inferred
            .rows
            .iter()
            .find(|row| row.path == "$.users[*].name")
            .unwrap();
        assert_eq!(name.required, Some(false));
    }
}
