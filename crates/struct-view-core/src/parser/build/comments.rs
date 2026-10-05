use super::super::node::{JsonNode, JsonValueType};
use super::format::DataFormat;

mod hash;
mod json5;

pub(super) fn extract_comments(input: &str, format: DataFormat) -> Vec<String> {
    match format {
        DataFormat::Json => Vec::new(),
        DataFormat::Json5 => json5::extract(input),
        DataFormat::Yaml | DataFormat::Toml => hash::extract_hash_comments(input, format),
        DataFormat::Dot | DataFormat::GraphMl | DataFormat::Gexf => Vec::new(),
    }
}

pub(super) fn add_comment_nodes(root: &mut JsonNode, comments: Vec<String>) {
    if comments.is_empty() {
        return;
    }

    let comment_nodes = comments
        .into_iter()
        .enumerate()
        .map(|(index, comment)| JsonNode {
            key: None,
            yaml_key: None,
            value_type: JsonValueType::Comment,
            display_value: comment,
            children: Vec::new(),
            expanded: false,
            path: format!("{}::comment[{index}]", root.path),
        })
        .collect::<Vec<_>>();
    root.children.splice(0..0, comment_nodes);
}

pub(super) fn collect_comments(node: &JsonNode) -> Vec<String> {
    let mut comments = Vec::new();
    collect_comment_nodes(node, &mut comments);
    comments
}

fn collect_comment_nodes(node: &JsonNode, comments: &mut Vec<String>) {
    if node.value_type == JsonValueType::Comment {
        comments.push(node.display_value.clone());
        return;
    }
    for child in &node.children {
        collect_comment_nodes(child, comments);
    }
}

pub fn comment_input(comment: &str) -> String {
    let comment = comment.trim();
    if let Some(body) = comment
        .strip_prefix("/*")
        .and_then(|comment| comment.strip_suffix("*/"))
    {
        return body.trim().to_string();
    }

    comment
        .lines()
        .map(|line| {
            let indentation = &line[..line.len() - line.trim_start().len()];
            let content = line.trim_start();
            let content = content
                .strip_prefix("//")
                .or_else(|| content.strip_prefix('#'))
                .map(|content| content.strip_prefix(' ').unwrap_or(content));
            match content {
                Some(content) => format!("{indentation}{content}"),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_comment_for_format(comment: &str, format: DataFormat) -> Result<String, String> {
    let marker = match format {
        DataFormat::Json5 => "//",
        DataFormat::Yaml | DataFormat::Toml => "#",
        DataFormat::Json => {
            return Err("JSON не поддерживает комментарии".to_string());
        }
        DataFormat::Dot | DataFormat::GraphMl | DataFormat::Gexf => {
            return Err(format!("{format} доступен только для чтения"));
        }
    };
    Ok(format_comment(comment, marker))
}

pub(super) fn format_comment(comment: &str, marker: &str) -> String {
    let body = comment_input(comment);
    if body.is_empty() {
        return marker.to_string();
    }
    body.lines()
        .map(|line| {
            if line.is_empty() {
                marker.to_string()
            } else {
                format!("{marker} {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
