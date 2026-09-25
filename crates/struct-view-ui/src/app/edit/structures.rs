use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use crate::clipboard::ClipboardEntry;
use struct_view_core::parser::{DataFormat, JsonNode, JsonValueType, node_to_value, parse_data};

use super::paths::{
    data_child_count, find_node_mut, update_child_paths, update_container_label, update_paths,
};

/// Собрать выбранные узлы для копирования, не дублируя вложенные выборы.
///
/// Если одновременно выбраны контейнер и его потомок, в буфер попадает
/// только контейнер: его значение уже содержит всю вложенную иерархию.
pub(in crate::app) fn selected_structures(
    root: &JsonNode,
    selected_paths: &BTreeSet<String>,
) -> Result<Vec<ClipboardEntry>, String> {
    let mut entries = Vec::new();
    collect_selected_structures(root, selected_paths, &mut entries)?;
    if entries.is_empty() {
        return Err("Не выбрано ни одной структуры".to_string());
    }
    Ok(entries)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum DeleteError {
    EmptySelection,
    RootSelected,
    SelectionNotFound,
}

/// Удалить выбранные узлы дерева, не удаляя потомков выбранного узла повторно.
pub(in crate::app) fn delete_selected_structures(
    root: &mut JsonNode,
    selected_paths: &BTreeSet<String>,
) -> Result<usize, DeleteError> {
    if selected_paths.is_empty() {
        return Err(DeleteError::EmptySelection);
    }
    if selected_paths.contains(&root.path) {
        return Err(DeleteError::RootSelected);
    }

    let mut targets = BTreeSet::new();
    collect_selected_paths(root, selected_paths, &mut targets);
    if targets.is_empty() {
        return Err(DeleteError::SelectionNotFound);
    }

    Ok(remove_selected_children(root, &targets))
}

fn collect_selected_paths(
    node: &JsonNode,
    selected_paths: &BTreeSet<String>,
    targets: &mut BTreeSet<String>,
) {
    for child in &node.children {
        if selected_paths.contains(&child.path) {
            targets.insert(child.path.clone());
        } else {
            collect_selected_paths(child, selected_paths, targets);
        }
    }
}

fn remove_selected_children(node: &mut JsonNode, selected_paths: &BTreeSet<String>) -> usize {
    let mut removed = 0;
    let mut retained = Vec::with_capacity(node.children.len());

    for mut child in std::mem::take(&mut node.children) {
        if selected_paths.contains(&child.path) {
            removed += 1;
        } else {
            removed += remove_selected_children(&mut child, selected_paths);
            retained.push(child);
        }
    }

    node.children = retained;
    if removed > 0 {
        update_child_paths(node);
        update_container_label(node);
    }
    removed
}

fn collect_selected_structures(
    node: &JsonNode,
    selected_paths: &BTreeSet<String>,
    entries: &mut Vec<ClipboardEntry>,
) -> Result<(), String> {
    if selected_paths.contains(&node.path) {
        entries.push(ClipboardEntry {
            key: node.key.clone(),
            value: node_to_value(node)?,
        });
        return Ok(());
    }

    for child in &node.children {
        collect_selected_structures(child, selected_paths, entries)?;
    }
    Ok(())
}

/// Вставить структуры в объект или массив по пути контейнера.
///
/// Ключи полей объектов сохраняются, а элементы массивов добавляются в конец
/// с новыми индексами. Неключевой объект (например, скопированный корень)
/// разворачивается в целевой объект своими полями; неключевой массив
/// разворачивается в целевой массив своими элементами.
pub(in crate::app) fn paste_structures_at_path(
    root: &mut JsonNode,
    target_path: &str,
    entries: &[ClipboardEntry],
) -> Result<usize, String> {
    if entries.is_empty() {
        return Err("Буфер структур пуст".to_string());
    }

    let parent = find_node_mut(root, target_path)
        .ok_or_else(|| "Не удалось найти контейнер для вставки данных".to_string())?;
    match parent.value_type {
        JsonValueType::Object => paste_into_object(parent, entries),
        JsonValueType::Array => paste_into_array(parent, entries),
        _ => Err("Вставлять структуры можно только в объект или массив".to_string()),
    }
}

fn paste_into_object(parent: &mut JsonNode, entries: &[ClipboardEntry]) -> Result<usize, String> {
    let mut candidates = Vec::new();
    for entry in entries {
        match (&entry.key, &entry.value) {
            (Some(key), value) => candidates.push((key.clone(), value.clone())),
            (None, Value::Object(fields)) => {
                candidates.extend(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
            }
            (None, _) => {
                return Err(
                    "Для вставки значения без ключа в объект скопируйте поле объекта".to_string(),
                );
            }
        }
    }
    if candidates.is_empty() {
        return Err("В буфере нет полей для вставки в объект".to_string());
    }

    let mut known_keys = parent
        .children
        .iter()
        .filter_map(|child| child.key.as_deref())
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    for (key, _) in &candidates {
        if !known_keys.insert(key.clone()) {
            return Err(format!("Поле «{}» уже существует", key));
        }
    }

    let parent_path = parent.path.clone();
    let nodes = candidates
        .into_iter()
        .map(|(key, value)| value_to_node(Some(key), false, &value, &parent_path))
        .collect::<Result<Vec<_>, _>>()?;
    let count = nodes.len();
    parent.children.extend(nodes);
    update_container_label(parent);
    Ok(count)
}

fn paste_into_array(parent: &mut JsonNode, entries: &[ClipboardEntry]) -> Result<usize, String> {
    let mut values = Vec::new();
    for entry in entries {
        if entry.key.is_none()
            && let Value::Array(items) = &entry.value
        {
            values.extend(items.iter().cloned());
            continue;
        }
        values.push(entry.value.clone());
    }
    if values.is_empty() {
        return Err("В буфере нет элементов для вставки в массив".to_string());
    }

    let parent_path = parent.path.clone();
    let first_index = data_child_count(parent);
    let nodes = values
        .into_iter()
        .enumerate()
        .map(|(offset, value)| {
            value_to_node(
                Some((first_index + offset).to_string()),
                true,
                &value,
                &parent_path,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = nodes.len();
    parent.children.extend(nodes);
    update_container_label(parent);
    Ok(count)
}

fn value_to_node(
    key: Option<String>,
    is_index: bool,
    value: &Value,
    parent_path: &str,
) -> Result<JsonNode, String> {
    let source = serde_json::to_string(value)
        .map_err(|error| format!("Ошибка подготовки структуры к вставке: {}", error))?;
    let (mut node, _) = parse_data(&source, Some(DataFormat::Json))
        .map_err(|error| format!("Не удалось подготовить структуру к вставке: {}", error))?;
    node.key = key;
    update_paths(&mut node, parent_path, is_index);
    Ok(node)
}
