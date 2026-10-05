//! Window-free access to the same document operations and models used by the GUI.
//!
//! Image rendering uses an in-memory egui context, not a native window or display.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use struct_view_core::parser::{DataFormat, JsonNode, JsonValueType};
use struct_view_core::search::SearchState;

use super::{edit, visualization};
use crate::clipboard::{decode_structures, encode_structures};

/// Resolve a path emitted by search, accepting `$` as the root.
pub fn node_at_path<'a>(root: &'a JsonNode, path: &str) -> Result<&'a JsonNode, String> {
    let normalized = normalize_path(path);
    if let Some(node) = edit::find_node(root, normalized) {
        return Ok(node);
    }
    // Root array indexes in the tree omit the brackets used by the table.
    if root.value_type == JsonValueType::Array
        && let Some(index) = normalized.strip_prefix('[')
        && let Some((index, rest)) = index.split_once(']')
        && index.chars().all(|ch| ch.is_ascii_digit())
        && let Some(node) = edit::find_node(root, &format!("{index}{rest}"))
    {
        return Ok(node);
    }
    Err(format!("Path not found: {path}"))
}

fn normalize_path(path: &str) -> &str {
    if path == "$" {
        ""
    } else if let Some(path) = path.strip_prefix("$.") {
        path
    } else if path.starts_with("$[") || path.starts_with("$::") {
        &path[1..]
    } else {
        path
    }
}

/// Add a typed field, array element, or comment using the GUI constructor.
pub fn add(
    root: &mut JsonNode,
    path: &str,
    key: &str,
    kind: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    let path = node_at_path(root, path)?.path.clone();
    edit::add_typed_child_at_path(root, &path, key, kind, value, format)
}

/// Change a node's type/value, optionally renaming an object field.
///
/// As in the GUI, selecting the current container type retains its children.
pub fn set(
    root: &mut JsonNode,
    path: &str,
    key: Option<&str>,
    kind: &JsonValueType,
    value: &str,
    format: DataFormat,
) -> Result<(), String> {
    let path = node_at_path(root, path)?.path.clone();
    edit::edit_child_at_path(root, &path, key, kind, value, format)
}

/// Rename without reconstructing the value, preserving native metadata.
pub fn rename(root: &mut JsonNode, path: &str, key: &str) -> Result<(), String> {
    let path = node_at_path(root, path)?.path.clone();
    edit::rename_at_path(root, &path, key)
}

fn selected_paths(root: &JsonNode, paths: &[String]) -> Result<BTreeSet<String>, String> {
    paths
        .iter()
        .map(|path| node_at_path(root, path).map(|node| node.path.clone()))
        .collect()
}

/// Delete selected structures, removing nested selections only once.
pub fn delete(root: &mut JsonNode, paths: &[String]) -> Result<(), String> {
    let paths = selected_paths(root, paths)?;
    edit::delete_selected_structures(root, &paths)
        .map(|_| ())
        .map_err(|error| format!("Cannot delete selection: {error:?}"))
}

/// Export selected structures in the GUI-compatible clipboard envelope.
pub fn copy(root: &JsonNode, paths: &[String]) -> Result<String, String> {
    encode_structures(&edit::selected_structures(
        root,
        &selected_paths(root, paths)?,
    )?)
}

/// Paste a clipboard envelope or plain JSON into a container.
pub fn paste(root: &mut JsonNode, path: &str, text: &str) -> Result<(), String> {
    let path = node_at_path(root, path)?.path.clone();
    edit::paste_structures_at_path(root, &path, &decode_structures(text)?).map(|_| ())
}

/// Export the flattened table with the active search filter.
pub fn table(root: &JsonNode, search: &SearchState) -> String {
    visualization::table_to_csv(&visualization::build_table(root), search)
}

/// Export schema diagram rows, clearly marking sample-derived schemas.
pub fn schema(root: &JsonNode, search: &SearchState) -> Result<Value, String> {
    let schema = visualization::build_schema_diagram(root)?;
    let indices = visualization::schema_visible_indices(&schema, search);
    let rows = schema.rows.iter().enumerate()
        .filter(|(index, _)| indices.as_ref().is_none_or(|indices| indices.contains(index)))
        .map(|(_, row)| json!({
            "key": row.key, "path": row.path, "type": row.type_name,
            "required": row.required, "constraints": row.constraints, "reference": row.reference,
        })).collect::<Vec<_>>();
    Ok(json!({
        "source": match schema.source {
            visualization::SchemaSource::JsonSchema => "json-schema",
            visualization::SchemaSource::OpenApi => "openapi",
            visualization::SchemaSource::Inferred => "inferred",
        },
        "title": schema.title, "rows": rows,
    }))
}

/// Export recognized entities and links, retaining parallel edges and partitions.
pub fn graph(root: &JsonNode) -> Result<Value, String> {
    let graph = visualization::try_build_relationship_graph(root)?;
    if graph.nodes.is_empty() {
        return Err("No graph entities found".to_string());
    }
    Ok(json!({
        "directed": graph.directed, "direction": graph.direction_name(), "partitions": graph.partition_names,
        "nodes": graph.nodes.iter().map(|node| json!({
            "id": node.id, "label": node.label, "path": node.path, "partition": node.partition,
        })).collect::<Vec<_>>(),
        "edges": graph.edges.iter().map(|edge| json!({
            "source": edge.source, "target": edge.target, "label": edge.label,
            "direction": edge.direction.as_str(),
        })).collect::<Vec<_>>(),
    }))
}

/// Render the full graph to SVG or PNG with the GUI's layout and image limits.
pub fn graph_image(root: &JsonNode, png: bool, dark: bool) -> Result<Vec<u8>, String> {
    super::views::graph_image_for_headless(root, png, dark)
}
