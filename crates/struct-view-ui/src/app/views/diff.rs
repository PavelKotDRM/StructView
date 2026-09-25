use super::*;

/// Отрисовать парный diff с выбором сравниваемых файлов.
pub(in crate::app) fn show_diff(
    ui: &mut egui::Ui,
    comparison: &mut ComparisonState,
    locale: Locale,
) {
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

    let pair_differences = comparison
        .differences
        .iter()
        .filter(|difference| {
            difference.values.get(left_index) != difference.values.get(right_index)
        })
        .collect::<Vec<_>>();
    if pair_differences.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(locale.text(TextKey::DiffNoDifferences))
                    .size(18.0)
                    .color(COLOR_SUCCESS),
            );
        });
        return;
    }

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
                    ui.label(RichText::new(locale.text(TextKey::ComparisonPath)).strong());
                    ui.label(RichText::new(locale.text(TextKey::DiffChangeType)).strong());
                    ui.label(
                        RichText::new(document_label(comparison, left_index))
                            .strong()
                            .monospace(),
                    );
                    ui.label(
                        RichText::new(document_label(comparison, right_index))
                            .strong()
                            .monospace(),
                    );
                    ui.end_row();

                    for difference in pair_differences {
                        let left = difference.values.get(left_index).and_then(Option::as_ref);
                        let right = difference.values.get(right_index).and_then(Option::as_ref);
                        let (change_type, left_color, right_color) = match pair_change(left, right)
                        {
                            Some(PairChange::Added) => (
                                locale.text(TextKey::DiffAdded),
                                Color32::GRAY,
                                COLOR_SUCCESS,
                            ),
                            Some(PairChange::Removed) => (
                                locale.text(TextKey::DiffRemoved),
                                COLOR_ERROR,
                                Color32::GRAY,
                            ),
                            Some(PairChange::Changed) => {
                                (locale.text(TextKey::DiffChanged), COLOR_MATCH, COLOR_MATCH)
                            }
                            None => continue,
                        };
                        ui.label(
                            RichText::new(&difference.path)
                                .color(COLOR_MATCH)
                                .monospace(),
                        );
                        ui.label(change_type);
                        ui.label(
                            RichText::new(
                                left.map(|value| format_value(Some(value)))
                                    .unwrap_or_else(|| {
                                        locale.text(TextKey::MissingValue).to_string()
                                    }),
                            )
                            .color(left_color)
                            .monospace(),
                        );
                        ui.label(
                            RichText::new(
                                right
                                    .map(|value| format_value(Some(value)))
                                    .unwrap_or_else(|| {
                                        locale.text(TextKey::MissingValue).to_string()
                                    }),
                            )
                            .color(right_color)
                            .monospace(),
                        );
                        ui.end_row();
                    }
                });
        });
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
    ui.horizontal_wrapped(|ui| {
        if let Some(context) = context {
            ui.label(context);
        }
        ui.label(RichText::new(locale.text(TextKey::DiffAdded)).color(COLOR_SUCCESS));
        ui.label(RichText::new(locale.text(TextKey::DiffRemoved)).color(COLOR_ERROR));
        ui.label(RichText::new(locale.text(TextKey::DiffChanged)).color(COLOR_MATCH));
    });
}

pub(in crate::app) fn comparison_value_color(
    reference: Option<&Value>,
    value: Option<&Value>,
    unchanged_color: Color32,
) -> Color32 {
    match pair_change(reference, value) {
        Some(PairChange::Added) => COLOR_SUCCESS,
        Some(PairChange::Removed) => COLOR_ERROR,
        Some(PairChange::Changed) => COLOR_MATCH,
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
        (Some(left), Some(right)) if left != right => Some(PairChange::Changed),
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
