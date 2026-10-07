pub(super) fn extract(input: &str) -> Vec<String> {
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
                while index < bytes.len()
                    && !matches!(bytes[index], b'\n' | b'\r')
                    && !bytes[index..].starts_with("\u{2028}".as_bytes())
                    && !bytes[index..].starts_with("\u{2029}".as_bytes())
                {
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
