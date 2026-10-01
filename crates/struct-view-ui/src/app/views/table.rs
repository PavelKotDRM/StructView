use super::*;

/// Отрисовать таблицу и вернуть `true`, если был запрошен экспорт CSV.
pub(in crate::app) fn show_table(
    ui: &mut egui::Ui,
    table: &TableData,
    search: &SearchState,
    locale: Locale,
) -> bool {
    let toolbar_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let export_clicked = ui
        .allocate_ui_with_layout(
            toolbar_size,
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let clicked = ui.button(locale.text(TextKey::ExportCsv)).clicked();
                ui.add(egui::Label::new(locale.text(TextKey::TableDescription)).truncate())
                    .on_hover_text(locale.text(TextKey::TableDescription));
                clicked
            },
        )
        .inner;

    let visible_indices = table_visible_indices(table, search);
    let visible_row_count = visible_indices.as_ref().map_or(table.rows.len(), Vec::len);
    if visible_row_count == 0 {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::NotFound));
        });
        return export_clicked;
    }

    let colors = SyntaxColors::new(ui.visuals());
    let width =
        (ui.available_width().max(MIN_TABLE_WIDTH) - 2.0 * ui.spacing().item_spacing.x).max(1.0);
    let path_width = width * 0.43;
    let value_width = width * 0.39;
    let type_width = width - path_width - value_width;
    let columns = [
        Column {
            width: path_width,
            header: locale.text(TextKey::ComparisonPath),
        },
        Column {
            width: value_width,
            header: locale.text(TextKey::Value),
        },
        Column {
            width: type_width,
            header: locale.text(TextKey::SchemaType),
        },
    ];
    show_virtualized_columns(
        ui,
        "table_horizontal",
        &columns,
        visible_row_count,
        |ui, visible_index| {
            let row_index = visible_indices
                .as_ref()
                .map_or(visible_index, |indices| indices[visible_index]);
            let row = &table.rows[row_index];
            let value = match row.value_type {
                JsonValueType::Object => locale.object_count(row.child_count),
                JsonValueType::Array => locale.array_count(row.child_count),
                _ => row.value.clone(),
            };
            ui.horizontal(|ui| {
                column_label(
                    ui,
                    path_width,
                    RichText::new(single_line_text(&row.path)).monospace(),
                )
                .on_hover_text(&row.path);
                column_label(
                    ui,
                    value_width,
                    RichText::new(single_line_text(&value))
                        .color(colors.value_color(&row.value_type))
                        .monospace(),
                )
                .on_hover_text(&value);
                column_label(
                    ui,
                    type_width,
                    RichText::new(locale.value_type_label(&row.value_type)),
                )
                .on_hover_text(locale.value_type_label(&row.value_type));
            });
        },
    );

    export_clicked
}

/// Формировать экспорт CSV после применения поиска, вызываемое из состояния.
pub(in crate::app) fn export_table_csv(table: &TableData, search: &SearchState) -> String {
    table_to_csv(table, search)
}
