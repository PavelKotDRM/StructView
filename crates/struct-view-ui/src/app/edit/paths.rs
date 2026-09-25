use struct_view_core::parser::{JsonNode, JsonValueType, build_path, plural_ru};

pub(in crate::app) fn find_node_mut<'a>(
    node: &'a mut JsonNode,
    path: &str,
) -> Option<&'a mut JsonNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, path))
}

pub(super) fn find_parent<'a>(node: &'a JsonNode, path: &str) -> Option<&'a JsonNode> {
    if node.children.iter().any(|child| child.path == path) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_parent(child, path))
}

pub(in crate::app) fn find_node<'a>(node: &'a JsonNode, path: &str) -> Option<&'a JsonNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, path))
}

pub(super) fn replace_node_at_path(
    node: &mut JsonNode,
    path: &str,
    replacement: &JsonNode,
    new_key: Option<&str>,
    parent_path: &str,
    is_index: bool,
) -> bool {
    if node.path == path {
        let mut updated = replacement.clone();
        updated.key = new_key
            .map(|key| key.trim().to_string())
            .or_else(|| node.key.clone());
        if matches!(
            (&node.value_type, &updated.value_type),
            (JsonValueType::Object, JsonValueType::Object)
                | (JsonValueType::Array, JsonValueType::Array)
        ) {
            updated.children = node.children.clone();
            updated.display_value = node.display_value.clone();
        }
        updated.expanded = node.expanded;
        update_paths(&mut updated, parent_path, is_index);
        *node = updated;
        return true;
    }

    let current_path = node.path.clone();
    let child_parent_path = if node.value_type == JsonValueType::Metadata {
        format!("{current_path}::metadata-value")
    } else {
        current_path.clone()
    };
    for child in &mut node.children {
        if replace_node_at_path(
            child,
            path,
            replacement,
            new_key,
            &child_parent_path,
            is_index,
        ) {
            update_container_label(node);
            return true;
        }
    }
    false
}

pub(super) fn update_paths(node: &mut JsonNode, parent_path: &str, is_index: bool) {
    node.path = build_path(parent_path, &node.key, is_index);
    update_child_paths(node);
}

pub(super) fn update_child_paths(node: &mut JsonNode) {
    let path = node.path.clone();
    let child_is_index = node.value_type == JsonValueType::Array;
    let mut data_index = 0;
    let mut comment_index = 0;
    for child in &mut node.children {
        if child.value_type == JsonValueType::Comment {
            child.key = None;
            child.path = format!("{path}::comment[{comment_index}]");
            comment_index += 1;
            continue;
        }
        if node.value_type == JsonValueType::Metadata {
            child.key = None;
            update_paths(child, &format!("{path}::metadata-value"), false);
            continue;
        }
        if child_is_index {
            child.key = Some(data_index.to_string());
            data_index += 1;
        }
        update_paths(child, &path, child_is_index);
    }
}

pub(super) fn update_container_label(node: &mut JsonNode) {
    let count = data_child_count(node);
    node.display_value = match node.value_type {
        JsonValueType::Object => format!(
            "{{{}}} {}",
            count,
            plural_ru(count, "поле", "поля", "полей")
        ),
        JsonValueType::Array => format!(
            "[{}] {}",
            count,
            plural_ru(count, "элемент", "элемента", "элементов")
        ),
        _ => return,
    };
}

pub(super) fn data_child_count(node: &JsonNode) -> usize {
    node.children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment)
        .count()
}
