use super::*;

/// Отрисовать таблицу и вернуть `true`, если был запрошен экспорт CSV.
pub(in crate::app) fn show_table(
    ui: &mut egui::Ui,
    table: &TableData,
    search: &SearchState,
    locale: Locale,
) -> bool {
    let export_clicked = ui
        .horizontal(|ui| {
            ui.label(locale.text(TextKey::TableDescription));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.button(locale.text(TextKey::ExportCsv)).clicked()
            })
            .inner
        })
        .inner;

    let visible_indices = table_visible_indices(table, search);
    let visible_row_count = visible_indices.as_ref().map_or(table.rows.len(), Vec::len);
    if visible_row_count == 0 {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::NotFound));
        });
        return export_clicked;
    }

    let width = ui.available_width().max(MIN_TABLE_WIDTH);
    let path_width = width * 0.43;
    let value_width = width * 0.39;
    let type_width = width - path_width - value_width;
    let row_height = ui.spacing().interact_size.y;

    ui.horizontal(|ui| {
        table_header(
            ui,
            path_width,
            locale.text(TextKey::ComparisonPath),
            row_height,
        );
        table_header(ui, value_width, locale.text(TextKey::Value), row_height);
        table_header(ui, type_width, locale.text(TextKey::SchemaType), row_height);
    });

    egui::ScrollArea::both().auto_shrink([false; 2]).show_rows(
        ui,
        row_height,
        visible_row_count,
        |ui, row_range| {
            for visible_index in row_range {
                let row_index = visible_indices
                    .as_ref()
                    .map_or(visible_index, |indices| indices[visible_index]);
                let row = &table.rows[row_index];
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [path_width, row_height],
                        egui::Label::new(RichText::new(&row.path).monospace()).truncate(),
                    )
                    .on_hover_text(&row.path);
                    ui.add_sized(
                        [value_width, row_height],
                        egui::Label::new(
                            RichText::new(&row.value)
                                .color(value_color(&row.value_type))
                                .monospace(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&row.value);
                    ui.add_sized(
                        [type_width, row_height],
                        egui::Label::new(value_type_label(locale, &row.value_type)),
                    );
                });
            }
        },
    );

    export_clicked
}

pub(super) fn table_header(ui: &mut egui::Ui, width: f32, label: &str, height: f32) {
    ui.add_sized(
        [width, height],
        egui::Label::new(RichText::new(label).strong()),
    );
}

fn value_type_label(locale: Locale, value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::Object => locale.text(TextKey::TypeObject),
        JsonValueType::Array => locale.text(TextKey::TypeArray),
        JsonValueType::String => locale.text(TextKey::TypeString),
        JsonValueType::DateTime => locale.text(TextKey::TypeDateTime),
        JsonValueType::Comment => locale.text(TextKey::TypeComment),
        JsonValueType::Metadata => locale.text(TextKey::TypeMetadata),
        JsonValueType::Number => locale.text(TextKey::TypeNumber),
        JsonValueType::Float => locale.text(TextKey::TypeFloat),
        JsonValueType::Bool => locale.text(TextKey::TypeBoolean),
        JsonValueType::Null => locale.text(TextKey::TypeNull),
    }
}

/// Формировать экспорт CSV после применения поиска, вызываемое из состояния.
pub(in crate::app) fn export_table_csv(table: &TableData, search: &SearchState) -> String {
    table_to_csv(table, search)
}
