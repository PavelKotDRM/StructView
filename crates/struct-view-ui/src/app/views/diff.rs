use super::super::state::PairDifferenceCache;
use super::*;
use struct_view_core::diff::{compare_pair_at_path, values_equal};

/// Отрисовать парный diff с выбором сравниваемых файлов.
pub(in crate::app) fn show_diff(
    ui: &mut egui::Ui,
    comparison: &mut ComparisonState,
    locale: Locale,
) {
    let colors = SyntaxColors::new(ui.visuals());
    let mut left_index = comparison.left_index;
    let mut right_index = comparison.right_index;
    let selector_width = (ui.available_width() / 2.0 - 100.0).clamp(120.0, 360.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(locale.text(TextKey::DiffLeft));
        egui::ComboBox::from_id_salt("diff_left_document")
            .width(selector_width)
            .selected_text(document_label(comparison, left_index))
            .show_ui(ui, |ui| {
                for (index, document) in comparison.documents.iter().enumerate() {
                    ui.selectable_value(
                        &mut left_index,
                        index,
                        document_label_for_path(&document.path, document.format),
                    );
                }
            });
        ui.label(locale.text(TextKey::DiffRight));
        egui::ComboBox::from_id_salt("diff_right_document")
            .width(selector_width)
            .selected_text(document_label(comparison, right_index))
            .show_ui(ui, |ui| {
                for (index, document) in comparison.documents.iter().enumerate() {
                    ui.selectable_value(
                        &mut right_index,
                        index,
                        document_label_for_path(&document.path, document.format),
                    );
                }
            });
    });
    comparison.left_index = left_index;
    comparison.right_index = right_index;

    let cache_matches = comparison
        .pair_cache
        .as_ref()
        .is_some_and(|cache| cache.left_index == left_index && cache.right_index == right_index);
    if !cache_matches {
        let differences = comparison
            .differences
            .iter()
            .flat_map(|difference| {
                let left_value = difference.values.get(left_index).and_then(Option::as_ref);
                let right_value = difference.values.get(right_index).and_then(Option::as_ref);
                let left_json = parse_json_structure_text(left_value);
                let right_json = parse_json_structure_text(right_value);
                let left = left_json.as_ref().or(left_value);
                let right = right_json.as_ref().or(right_value);
                compare_pair_at_path(&difference.path, left, right)
            })
            .collect();
        comparison.pair_cache = Some(PairDifferenceCache {
            left_index,
            right_index,
            differences,
        });
    }
    let pair_differences = comparison
        .pair_cache
        .as_ref()
        .map_or(&[][..], |cache| cache.differences.as_slice());
    if pair_differences.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(locale.text(TextKey::DiffNoDifferences))
                    .size(18.0)
                    .color(colors.success),
            );
        });
        return;
    }

    ui.label(format!(
        "{} {}",
        locale.text(TextKey::DiffCount),
        pair_differences.len()
    ));
    show_difference_legend(ui, locale, None);
    let column_count = 4;
    let column_width = comparison_column_width(ui.available_width(), column_count);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("pair_diff_grid")
                .num_columns(column_count)
                .striped(true)
                .min_col_width(column_width)
                .max_col_width(column_width)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    column_label(
                        ui,
                        column_width,
                        RichText::new(locale.text(TextKey::DiffPath)).strong(),
                    );
                    column_label(
                        ui,
                        column_width,
                        RichText::new(locale.text(TextKey::DiffChangeType)).strong(),
                    );
                    column_label(
                        ui,
                        column_width,
                        RichText::new(locale.text(TextKey::DiffBefore)).strong(),
                    )
                    .on_hover_text(document_label(comparison, left_index));
                    column_label(
                        ui,
                        column_width,
                        RichText::new(locale.text(TextKey::DiffAfter)).strong(),
                    )
                    .on_hover_text(document_label(comparison, right_index));
                    ui.end_row();

                    for difference in pair_differences {
                        let left = difference.values[0].as_ref();
                        let right = difference.values[1].as_ref();
                        let (change_type, left_color, right_color) = match pair_change(left, right)
                        {
                            Some(PairChange::Added) => {
                                (locale.text(TextKey::DiffAdded), colors.null, colors.success)
                            }
                            Some(PairChange::Removed) => {
                                (locale.text(TextKey::DiffRemoved), colors.error, colors.null)
                            }
                            Some(PairChange::Changed) => (
                                locale.text(TextKey::DiffChanged),
                                colors.matched,
                                colors.matched,
                            ),
                            None => continue,
                        };
                        column_label(
                            ui,
                            column_width,
                            RichText::new(single_line_text(&human_diff_path(
                                &difference.path,
                                locale,
                            )))
                            .color(colors.matched),
                        )
                        .on_hover_text(&difference.path);
                        column_label(ui, column_width, RichText::new(change_type))
                            .on_hover_text(change_type);
                        for (index, (value, color)) in [(left, left_color), (right, right_color)]
                            .into_iter()
                            .enumerate()
                        {
                            if let Some(value @ (Value::Array(_) | Value::Object(_))) = value {
                                ui.set_width(column_width);
                                ui.push_id((&difference.path, index), |ui| {
                                    egui::CollapsingHeader::new(
                                        RichText::new(display_diff_value(value, locale))
                                            .color(color),
                                    )
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        egui::ScrollArea::vertical().max_height(220.0).show(
                                            ui,
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format_value(Some(value)))
                                                            .monospace(),
                                                    )
                                                    .wrap(),
                                                );
                                            },
                                        );
                                    });
                                });
                            } else {
                                let text = value
                                    .map(|value| display_diff_value(value, locale))
                                    .unwrap_or_else(|| {
                                        locale.text(TextKey::MissingValue).to_string()
                                    });
                                column_label(
                                    ui,
                                    column_width,
                                    RichText::new(single_line_text(&text)).color(color),
                                )
                                .on_hover_text(value.map_or_else(
                                    || locale.text(TextKey::MissingValue).to_string(),
                                    |value| format_value(Some(value)),
                                ));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

fn parse_json_structure_text(value: Option<&Value>) -> Option<Value> {
    let Value::String(text) = value? else {
        return None;
    };
    let text = text.trim();
    if !text.starts_with(['{', '[']) {
        return None;
    }
    let value = serde_json::from_str::<Value>(text).ok()?;
    matches!(value, Value::Object(_) | Value::Array(_)).then_some(value)
}

pub(in crate::app) fn display_diff_value(value: &Value, locale: Locale) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(values) => format!(
            "{} ({} {})",
            locale.text(TextKey::TypeArray),
            values.len(),
            collection_count_word(locale, values.len(), true)
        ),
        Value::Object(values) => format!(
            "{} ({} {})",
            locale.text(TextKey::TypeObject),
            values.len(),
            collection_count_word(locale, values.len(), false)
        ),
    }
}

fn collection_count_word(locale: Locale, count: usize, is_array: bool) -> &'static str {
    match locale {
        Locale::Russian if is_array => {
            struct_view_core::parser::plural_ru(count, "элемент", "элемента", "элементов")
        }
        Locale::Russian => struct_view_core::parser::plural_ru(count, "поле", "поля", "полей"),
        Locale::English if is_array && count == 1 => "item",
        Locale::English if is_array => "items",
        Locale::English if count == 1 => "field",
        Locale::English => "fields",
    }
}

pub(in crate::app) fn human_diff_path(path: &str, locale: Locale) -> String {
    let mut chars = path.char_indices().peekable();
    let mut segments = Vec::new();
    if chars.next().is_none_or(|(_, character)| character != '$') {
        return path.to_string();
    }

    while let Some((_, character)) = chars.next() {
        match character {
            '.' => {
                let mut key = String::new();
                while let Some(&(_, next)) = chars.peek() {
                    if matches!(next, '.' | '[') {
                        break;
                    }
                    chars.next();
                    key.push(next);
                }
                if !key.is_empty() {
                    segments.push(key);
                }
            }
            '[' => {
                let Some(&(_, first)) = chars.peek() else {
                    return path.to_string();
                };
                if first == '"' {
                    chars.next();
                    let mut quoted = String::from("\"");
                    let mut escaped = false;
                    let mut closed = false;
                    for (_, next) in chars.by_ref() {
                        quoted.push(next);
                        if next == '"' && !escaped {
                            closed = true;
                            break;
                        }
                        if next == '\\' && !escaped {
                            escaped = true;
                        } else {
                            escaped = false;
                        }
                    }
                    if !closed || chars.next().is_none_or(|(_, next)| next != ']') {
                        return path.to_string();
                    }
                    match serde_json::from_str::<String>(&quoted) {
                        Ok(key) => segments.push(key),
                        Err(_) => return path.to_string(),
                    }
                } else {
                    let mut index = String::new();
                    let mut closed = false;
                    for (_, next) in chars.by_ref() {
                        if next == ']' {
                            closed = true;
                            break;
                        }
                        index.push(next);
                    }
                    if !closed {
                        return path.to_string();
                    }
                    let Some(index) = index.parse::<usize>().ok() else {
                        return path.to_string();
                    };
                    segments.push(format!(
                        "{} {}",
                        locale.text(TextKey::DiffArrayItem),
                        index + 1
                    ));
                }
            }
            _ => return path.to_string(),
        }
    }

    if segments.is_empty() {
        locale.text(TextKey::DiffRoot).to_string()
    } else {
        segments.join(" → ")
    }
}

pub(in crate::app) fn comparison_column_width(available_width: f32, column_count: usize) -> f32 {
    let column_count = column_count.max(1);
    let spacing = 12.0 * column_count.saturating_sub(1) as f32;
    ((available_width - spacing) / column_count as f32).max(96.0)
}

pub(in crate::app) fn show_difference_legend(
    ui: &mut egui::Ui,
    locale: Locale,
    context: Option<&str>,
) {
    let colors = SyntaxColors::new(ui.visuals());
    ui.horizontal_wrapped(|ui| {
        if let Some(context) = context {
            ui.label(context);
        }
        ui.label(RichText::new(locale.text(TextKey::DiffAdded)).color(colors.success));
        ui.label(RichText::new(locale.text(TextKey::DiffRemoved)).color(colors.error));
        ui.label(RichText::new(locale.text(TextKey::DiffChanged)).color(colors.matched));
    });
}

pub(in crate::app) fn comparison_value_color(
    reference: Option<&Value>,
    value: Option<&Value>,
    unchanged_color: Color32,
    colors: SyntaxColors,
) -> Color32 {
    match pair_change(reference, value) {
        Some(PairChange::Added) => colors.success,
        Some(PairChange::Removed) => colors.error,
        Some(PairChange::Changed) => colors.matched,
        None => unchanged_color,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PairChange {
    Added,
    Removed,
    Changed,
}

pub(super) fn pair_change(left: Option<&Value>, right: Option<&Value>) -> Option<PairChange> {
    match (left, right) {
        (None, Some(_)) => Some(PairChange::Added),
        (Some(_), None) => Some(PairChange::Removed),
        (Some(left), Some(right)) if !values_equal(left, right) => Some(PairChange::Changed),
        (None, None) | (Some(_), Some(_)) => None,
    }
}

fn document_label(comparison: &ComparisonState, index: usize) -> String {
    comparison
        .documents
        .get(index)
        .map(|document| document_label_for_path(&document.path, document.format))
        .unwrap_or_default()
}

pub(super) fn document_label_for_path(path: &std::path::Path, format: DataFormat) -> String {
    format!("{} ({format})", path.display())
}
