use super::super::node::{JsonNode, JsonValueType};
use super::format::DataFormat;

pub(super) fn extract_comments(input: &str, format: DataFormat) -> Vec<String> {
    match format {
        DataFormat::Json => Vec::new(),
        DataFormat::Json5 => extract_json5_comments(input),
        DataFormat::Yaml | DataFormat::Toml => extract_hash_comments(input, format),
    }
}

fn extract_json5_comments(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut comments = Vec::new();
    let mut quote = None;
    let mut index = 0;

    while index < bytes.len() {
        if let Some(quote_byte) = quote {
            if bytes[index] == b'\\' {
                index = (index + 2).min(bytes.len());
            } else if bytes[index] == quote_byte {
                quote = None;
                index += 1;
            } else {
                index += 1;
            }
            continue;
        }

        match bytes[index] {
            b'"' | b'\'' => {
                quote = Some(bytes[index]);
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let start = index;
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                comments.push(input[start..index].trim_end().to_string());
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let start = index;
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                if index + 1 < bytes.len() {
                    index += 2;
                    comments.push(input[start..index].to_string());
                }
            }
            _ => index += 1,
        }
    }

    comments
}

#[derive(Clone, Copy)]
enum HashQuote {
    Single,
    Double,
    MultilineSingle,
    MultilineDouble,
}

fn extract_hash_comments(input: &str, format: DataFormat) -> Vec<String> {
    let mut comments = Vec::new();
    let mut quote = None;
    let mut yaml_block_indent = None;

    for line in input.lines() {
        let bytes = line.as_bytes();
        let indent = bytes.iter().take_while(|byte| **byte == b' ').count();
        if format == DataFormat::Yaml
            && let Some(block_indent) = yaml_block_indent
        {
            if line.trim().is_empty() {
                continue;
            }
            if indent > block_indent {
                continue;
            }
            yaml_block_indent = None;
        }

        let mut index = 0;
        let mut code_end = bytes.len();
        while index < bytes.len() {
            if let Some(active_quote) = quote {
                match active_quote {
                    HashQuote::Single => {
                        if bytes[index] == b'\'' {
                            if bytes.get(index + 1) == Some(&b'\'') {
                                index += 2;
                            } else {
                                quote = None;
                                index += 1;
                            }
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::Double => {
                        if bytes[index] == b'\\' {
                            index = (index + 2).min(bytes.len());
                        } else if bytes[index] == b'"' {
                            quote = None;
                            index += 1;
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineSingle => {
                        if bytes[index..].starts_with(b"'''") {
                            quote = None;
                            index += 3;
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineDouble => {
                        if bytes[index] == b'\\' {
                            index = (index + 2).min(bytes.len());
                        } else if bytes[index..].starts_with(b"\"\"\"") {
                            quote = None;
                            index += 3;
                        } else {
                            index += 1;
                        }
                    }
                }
                continue;
            }

            if bytes[index] == b'#'
                && (format == DataFormat::Toml
                    || index == 0
                    || bytes[index - 1].is_ascii_whitespace())
            {
                comments.push(line[index..].trim_end().to_string());
                code_end = index;
                break;
            }

            if format == DataFormat::Toml && bytes[index..].starts_with(b"'''") {
                quote = Some(HashQuote::MultilineSingle);
                index += 3;
            } else if format == DataFormat::Toml && bytes[index..].starts_with(b"\"\"\"") {
                quote = Some(HashQuote::MultilineDouble);
                index += 3;
            } else {
                match bytes[index] {
                    b'\'' => {
                        quote = Some(HashQuote::Single);
                        index += 1;
                    }
                    b'"' => {
                        quote = Some(HashQuote::Double);
                        index += 1;
                    }
                    _ => index += 1,
                }
            }
        }

        if format == DataFormat::Yaml && yaml_has_block_scalar_indicator(&line[..code_end]) {
            yaml_block_indent = Some(indent);
        }
    }

    comments
}

fn yaml_has_block_scalar_indicator(line: &str) -> bool {
    let line = line.trim_end();
    let bytes = line.as_bytes();
    for index in (0..bytes.len()).rev() {
        if !matches!(bytes[index], b'|' | b'>')
            || (index > 0 && !bytes[index - 1].is_ascii_whitespace())
        {
            continue;
        }
        if bytes[index + 1..]
            .iter()
            .all(|byte| matches!(byte, b'+' | b'-' | b'1'..=b'9'))
        {
            return true;
        }
    }
    false
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
