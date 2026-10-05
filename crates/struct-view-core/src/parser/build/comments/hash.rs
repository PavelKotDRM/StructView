use super::super::format::DataFormat;

#[derive(Clone, Copy)]
enum HashQuote {
    Single,
    Double,
    MultilineSingle,
    MultilineDouble,
}

pub(super) fn extract_hash_comments(input: &str, format: DataFormat) -> Vec<String> {
    let mut comments = Vec::new();
    let mut quote = None;
    let mut yaml_block_indent = None;
    let mut yaml_flow_depth = 0usize;

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
        let mut yaml_scalar_start = quote.is_none();
        let mut yaml_block_indicator = None;
        while index < bytes.len() {
            if let Some(active_quote) = quote {
                match active_quote {
                    HashQuote::Single => {
                        if bytes[index] == b'\'' {
                            if bytes.get(index + 1) == Some(&b'\'') {
                                index += 2;
                            } else {
                                quote = None;
                                yaml_scalar_start = false;
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
                            yaml_scalar_start = false;
                            index += 1;
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineSingle => {
                        if bytes[index..].starts_with(b"'''") {
                            quote = None;
                            index += bytes[index..]
                                .iter()
                                .take_while(|byte| **byte == b'\'')
                                .count()
                                .min(5);
                        } else {
                            index += 1;
                        }
                    }
                    HashQuote::MultilineDouble => {
                        if bytes[index] == b'\\' {
                            index = (index + 2).min(bytes.len());
                        } else if bytes[index..].starts_with(b"\"\"\"") {
                            quote = None;
                            index += bytes[index..]
                                .iter()
                                .take_while(|byte| **byte == b'"')
                                .count()
                                .min(5);
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

            if format == DataFormat::Yaml {
                match bytes[index] {
                    b':' if bytes.get(index + 1).is_none_or(|byte| {
                        byte.is_ascii_whitespace() || matches!(byte, b'"' | b'\'' | b'[' | b'{')
                    }) =>
                    {
                        yaml_scalar_start = true;
                        index += 1;
                        continue;
                    }
                    b'[' | b'{' if yaml_scalar_start => {
                        yaml_flow_depth += 1;
                        yaml_scalar_start = true;
                        index += 1;
                        continue;
                    }
                    b']' | b'}' if yaml_flow_depth > 0 => {
                        yaml_flow_depth -= 1;
                        yaml_scalar_start = false;
                        index += 1;
                        continue;
                    }
                    b',' if yaml_flow_depth > 0 => {
                        yaml_scalar_start = true;
                        index += 1;
                        continue;
                    }
                    b'-' | b'?'
                        if yaml_scalar_start
                            && bytes.get(index + 1).is_some_and(u8::is_ascii_whitespace) =>
                    {
                        index += 1;
                        continue;
                    }
                    b'!' | b'&' if yaml_scalar_start => {
                        while index < bytes.len()
                            && !bytes[index].is_ascii_whitespace()
                            && !matches!(bytes[index], b',' | b'[' | b']' | b'{' | b'}')
                        {
                            index += 1;
                        }
                        continue;
                    }
                    b'"' | b'\'' if !yaml_scalar_start => {
                        index += 1;
                        continue;
                    }
                    b'|' | b'>' if yaml_scalar_start => {
                        yaml_block_indicator = Some(index);
                        yaml_scalar_start = false;
                    }
                    byte if byte.is_ascii_whitespace() => {}
                    _ => yaml_scalar_start = false,
                }
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

        if let Some(indicator) = yaml_block_indicator
            && yaml_has_block_scalar_indicator(&line[indicator..code_end])
        {
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
