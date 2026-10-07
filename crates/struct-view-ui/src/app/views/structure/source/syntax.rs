use super::*;
use crate::app::theme::SyntaxColors;
use egui::text::{LayoutJob, TextFormat};

impl StructureView {
    pub(super) fn source_layouter(
        format: DataFormat,
    ) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> {
        move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let colors = SyntaxColors::new(ui.visuals());
            let mut job = Self::source_syntax_job(
                text.as_str(),
                format,
                colors,
                ui.style().text_styles[&egui::TextStyle::Monospace].size,
            );
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
    }

    pub(in crate::app::views::structure) fn source_syntax_job(
        text: &str,
        format: DataFormat,
        colors: SyntaxColors,
        font_size: f32,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = egui::FontId::monospace(font_size);
        let mut index = 0;

        while let Some(character) = text[index..].chars().next() {
            let start = index;
            let color = if is_comment_start(text, index, format) {
                index = comment_end(text, index, format);
                colors.comment
            } else if character == '"' || (character == '\'' && format != DataFormat::Json) {
                index = quoted_end(text, index, character, format);
                if is_key_suffix(&text[index..], format)
                    || (format == DataFormat::Yaml && text[index..].trim_start().starts_with(':'))
                {
                    colors.key
                } else {
                    colors.string
                }
            } else if character.is_ascii_digit()
                || (matches!(character, '-' | '+')
                    && text[index + character.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(|next| next.is_ascii_digit()))
            {
                index = consume_while(text, index, |character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '+' | '-')
                });
                if is_key_suffix(&text[index..], format) {
                    colors.key
                } else {
                    colors.number
                }
            } else if is_word_start(character) {
                index = consume_while(text, index, is_word_continue);
                let word = &text[start..index];
                if is_key_suffix(&text[index..], format) {
                    colors.key
                } else {
                    match word {
                        "true" | "false" => colors.boolean,
                        "True" | "TRUE" | "False" | "FALSE" if format == DataFormat::Yaml => {
                            colors.boolean
                        }
                        "null"
                            if matches!(
                                format,
                                DataFormat::Json | DataFormat::Json5 | DataFormat::Yaml
                            ) =>
                        {
                            colors.null
                        }
                        "Null" | "NULL" | "~" if format == DataFormat::Yaml => colors.null,
                        _ => colors.string,
                    }
                }
            } else {
                index += character.len_utf8();
                colors.key
            };

            job.append(
                &text[start..index],
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                },
            );
        }

        job
    }
}

fn is_key_suffix(remaining: &str, format: DataFormat) -> bool {
    let next = remaining.trim_start();
    if format == DataFormat::Toml {
        next.starts_with('=')
    } else if format == DataFormat::Yaml {
        next.strip_prefix(':')
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(char::is_whitespace))
    } else {
        next.starts_with(':')
    }
}

fn is_comment_start(text: &str, index: usize, format: DataFormat) -> bool {
    let remaining = &text[index..];
    match format {
        DataFormat::Yaml => {
            remaining.starts_with('#')
                && (index == 0
                    || text[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace))
        }
        DataFormat::Toml => remaining.starts_with('#'),
        DataFormat::Json5 => remaining.starts_with("//") || remaining.starts_with("/*"),
        _ => false,
    }
}

fn comment_end(text: &str, start: usize, format: DataFormat) -> usize {
    let remaining = &text[start..];
    if format == DataFormat::Json5 && remaining.starts_with("/*") {
        return remaining
            .find("*/")
            .map_or(text.len(), |offset| start + offset + 2);
    }
    remaining
        .find(|character| {
            character == '\n'
                || (format == DataFormat::Json5
                    && matches!(character, '\r' | '\u{2028}' | '\u{2029}'))
        })
        .map_or(text.len(), |offset| start + offset)
}

fn quoted_end(text: &str, start: usize, quote: char, format: DataFormat) -> usize {
    let triple = format == DataFormat::Toml
        && text[start..].starts_with(if quote == '"' { "\"\"\"" } else { "'''" });
    let delimiter_len = if triple { 3 } else { 1 };
    let escapes = quote == '"' || format == DataFormat::Json5;
    let mut index = start + delimiter_len;
    while let Some(character) = text[index..].chars().next() {
        if escapes && character == '\\' {
            index += 1;
            if let Some(escaped) = text[index..].chars().next() {
                index += escaped.len_utf8();
            }
        } else if character == quote {
            if triple {
                let count = text[index..].chars().take_while(|&ch| ch == quote).count();
                if count >= 3 {
                    return index + count.min(5);
                }
                index += count;
            } else if format == DataFormat::Yaml
                && quote == '\''
                && text[index + 1..].starts_with('\'')
            {
                index += 2;
            } else {
                return index + 1;
            }
        } else {
            index += character.len_utf8();
        }
    }
    text.len()
}

fn consume_while(text: &str, start: usize, predicate: impl Fn(char) -> bool) -> usize {
    text[start..]
        .char_indices()
        .take_while(|(_, character)| predicate(*character))
        .last()
        .map_or(start, |(offset, character)| {
            start + offset + character.len_utf8()
        })
}

fn is_word_start(character: char) -> bool {
    character.is_alphabetic() || matches!(character, '_' | '$' | '~')
}

fn is_word_continue(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '$' | '~')
}
