use std::borrow::Cow;

use egui::{Response, RichText, Ui};

pub(super) fn single_line_text(text: &str) -> Cow<'_, str> {
    let needs_escape =
        |character: char| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}');
    if !text.chars().any(needs_escape) {
        return Cow::Borrowed(text);
    }
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if needs_escape(character) {
            escaped.extend(character.escape_debug());
        } else {
            escaped.push(character);
        }
    }
    Cow::Owned(escaped)
}

pub(super) struct Column<'a> {
    pub(super) width: f32,
    pub(super) header: &'a str,
}

pub(super) fn column_label(ui: &mut Ui, width: f32, text: RichText) -> Response {
    ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::Label::new(text).truncate(),
    )
}

pub(super) fn show_virtualized_columns(
    ui: &mut Ui,
    id_salt: &str,
    columns: &[Column<'_>],
    row_count: usize,
    mut show_row: impl FnMut(&mut Ui, usize),
) {
    let row_height = ui.spacing().interact_size.y;
    let width = columns.iter().map(|column| column.width).sum::<f32>()
        + ui.spacing().item_spacing.x * columns.len().saturating_sub(1) as f32;
    egui::ScrollArea::horizontal()
        .id_salt(id_salt)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                for column in columns {
                    column_label(ui, column.width, RichText::new(column.header).strong())
                        .on_hover_text(column.header);
                }
            });
            egui::ScrollArea::vertical()
                .id_salt((id_salt, "rows"))
                .auto_shrink([false; 2])
                .show_rows(ui, row_height, row_count, |ui, row_range| {
                    for index in row_range {
                        ui.push_id(index, |ui| show_row(ui, index));
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_row_breaks_without_changing_unicode_or_plain_text() {
        assert!(matches!(single_line_text("plain text"), Cow::Borrowed(_)));
        assert_eq!(single_line_text("line\n\tbreak\r\0"), r"line\n\tbreak\r\0");
        assert_eq!(single_line_text("ключ\nvalue"), "ключ\\nvalue");
    }
}
