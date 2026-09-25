use std::collections::{BTreeMap, BTreeSet, HashMap};

use struct_view_core::parser::{JsonNode, JsonValueType, build_path};

use super::{SchemaDiagram, SchemaRow, SchemaSource};

#[derive(Default)]
struct InferredRow {
    types: BTreeSet<String>,
    occurrences: usize,
    parent_path: Option<String>,
}

pub(super) fn inferred_schema(root: &JsonNode) -> SchemaDiagram {
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
    if node.value_type == JsonValueType::Comment {
        return;
    }
    if node.value_type == JsonValueType::Metadata {
        for child in &node.children {
            if child.value_type != JsonValueType::Comment {
                collect_inferred_rows(child, path, parent_path, rows, object_counts);
            }
        }
        return;
    }

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
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
                let child_path = build_path(path, &child.key, false);
                collect_inferred_rows(child, &child_path, Some(path), rows, object_counts);
            }
        }
        JsonValueType::Array => {
            for child in &node.children {
                if child.value_type == JsonValueType::Comment {
                    continue;
                }
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
        JsonValueType::Comment | JsonValueType::Metadata => {}
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
        JsonValueType::Comment | JsonValueType::Metadata => "any",
    }
}
