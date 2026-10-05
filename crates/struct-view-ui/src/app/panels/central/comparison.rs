use super::*;

impl StructViewApp {
    pub(super) fn show_comparison(&self, ui: &mut Ui) {
        let Some(comparison) = &self.comparison else {
            return;
        };
        let locale = self.locale;
        let colors = SyntaxColors::new(ui.visuals());

        if comparison.differences.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(locale.text(TextKey::ComparisonNoDifferences))
                        .size(18.0)
                        .color(colors.success),
                );
            });
            return;
        }

        show_difference_legend(
            ui,
            locale,
            Some(locale.text(TextKey::ComparisonColorLegend)),
        );
        let column_count = comparison.documents.len() + 1;
        let column_width = comparison_column_width(ui.available_width(), column_count);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("comparison_grid")
                    .num_columns(column_count)
                    .striped(true)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        column_label(
                            ui,
                            column_width,
                            RichText::new(locale.text(TextKey::ComparisonPath)).strong(),
                        );
                        for document in &comparison.documents {
                            let header = format!(
                                "{} ({}, {:.1} KB, {} ms)",
                                document.path.display(),
                                document.format,
                                document.size_bytes as f64 / 1024.0,
                                document.load_time_ms
                            );
                            column_label(
                                ui,
                                column_width,
                                RichText::new(single_line_text(&header))
                                    .strong()
                                    .monospace(),
                            )
                            .on_hover_text(&header);
                        }
                        ui.end_row();

                        for difference in &comparison.differences {
                            column_label(
                                ui,
                                column_width,
                                RichText::new(single_line_text(&difference.path))
                                    .color(colors.matched)
                                    .monospace(),
                            )
                            .on_hover_text(&difference.path);
                            let reference = difference.values.first().and_then(Option::as_ref);
                            for value in &difference.values {
                                let text = value
                                    .as_ref()
                                    .map(|value| format_value(Some(value)))
                                    .unwrap_or_else(|| {
                                        locale.text(TextKey::MissingValue).to_string()
                                    });
                                let color = comparison_value_color(
                                    reference,
                                    value.as_ref(),
                                    ui.visuals().text_color(),
                                    colors,
                                );
                                column_label(
                                    ui,
                                    column_width,
                                    RichText::new(single_line_text(&text))
                                        .color(color)
                                        .monospace(),
                                )
                                .on_hover_text(&text);
                            }
                            ui.end_row();
                        }
                    });
            });
    }
}
