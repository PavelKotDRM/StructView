//! Цветовая схема приложения.
//!
//! Задаёт цвета подсветки типов значений и выделения результатов поиска.
//! Цвета подобраны так, чтобы оставаться читаемыми в тёмной и светлой темах.

use egui::Color32;

use struct_view_core::parser::JsonValueType;

/// Цвет для строковых значений (зелёный).
pub(super) const COLOR_STRING: Color32 = Color32::from_rgb(106, 177, 112);
/// Цвет для числовых значений (голубой).
pub(super) const COLOR_NUMBER: Color32 = Color32::from_rgb(100, 163, 220);
/// Цвет для булевых значений (оранжевый).
pub(super) const COLOR_BOOL: Color32 = Color32::from_rgb(209, 154, 102);
/// Цвет для значений null (серый).
pub(super) const COLOR_NULL: Color32 = Color32::from_rgb(150, 150, 150);
/// Цвет для ключей объектов (белый/светлый).
pub(super) const COLOR_KEY: Color32 = Color32::from_rgb(224, 224, 224);
/// Цвет для YAML-тегов (фиолетовый).
pub(super) const COLOR_METADATA: Color32 = Color32::from_rgb(183, 116, 202);
/// Цвет для комментариев (сине-зелёный).
pub(super) const COLOR_COMMENT: Color32 = Color32::from_rgb(100, 157, 169);
/// Цвет для подсветки совпадений при поиске (жёлтый).
pub(super) const COLOR_MATCH: Color32 = Color32::from_rgb(229, 192, 73);
/// Цвет для активного совпадения при поиске (ярко-оранжевый).
pub(super) const COLOR_ACTIVE_MATCH: Color32 = Color32::from_rgb(255, 120, 50);
/// Цвет сообщений об ошибках (красный).
pub(super) const COLOR_ERROR: Color32 = Color32::from_rgb(236, 94, 94);
/// Цвет успешных уведомлений (зелёный).
pub(super) const COLOR_SUCCESS: Color32 = Color32::from_rgb(100, 200, 100);

/// Вернуть цвет, соответствующий типу значения JSON.
///
/// Объекты и массивы окрашиваются как ключи, поскольку их отображаемое
/// значение — служебная подпись вида `{3 поля}`.
#[derive(Debug, Clone, Copy)]
pub(super) struct SyntaxColors {
    pub(super) key: Color32,
    pub(super) string: Color32,
    pub(super) number: Color32,
    pub(super) boolean: Color32,
    pub(super) null: Color32,
    pub(super) metadata: Color32,
    pub(super) comment: Color32,
    pub(super) matched: Color32,
    pub(super) active_match: Color32,
    pub(super) error: Color32,
    pub(super) success: Color32,
}

impl SyntaxColors {
    pub(super) fn new(visuals: &egui::Visuals) -> Self {
        if visuals.dark_mode {
            Self {
                key: COLOR_KEY,
                string: COLOR_STRING,
                number: COLOR_NUMBER,
                boolean: COLOR_BOOL,
                null: COLOR_NULL,
                metadata: COLOR_METADATA,
                comment: COLOR_COMMENT,
                matched: COLOR_MATCH,
                active_match: COLOR_ACTIVE_MATCH,
                error: COLOR_ERROR,
                success: COLOR_SUCCESS,
            }
        } else {
            Self {
                key: Color32::from_rgb(20, 80, 140),
                string: Color32::from_rgb(32, 105, 48),
                number: Color32::from_rgb(24, 83, 150),
                boolean: Color32::from_rgb(137, 69, 14),
                null: Color32::from_gray(95),
                metadata: Color32::from_rgb(125, 50, 154),
                comment: Color32::from_rgb(37, 102, 112),
                matched: Color32::from_rgb(125, 80, 0),
                active_match: Color32::from_rgb(170, 52, 12),
                error: Color32::from_rgb(180, 35, 35),
                success: Color32::from_rgb(25, 112, 43),
            }
        }
    }

    pub(super) fn value_color(self, vtype: &JsonValueType) -> Color32 {
        match vtype {
            JsonValueType::String | JsonValueType::DateTime => self.string,
            JsonValueType::Number | JsonValueType::Float => self.number,
            JsonValueType::Bool => self.boolean,
            JsonValueType::Null => self.null,
            JsonValueType::Object | JsonValueType::Array => self.key,
            JsonValueType::Metadata => self.metadata,
            JsonValueType::Comment => self.comment,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{COLOR_COMMENT, COLOR_METADATA, SyntaxColors};
    use struct_view_core::parser::JsonValueType;

    #[test]
    fn comments_and_metadata_have_distinct_colors() {
        let visuals = egui::Visuals::dark();
        let colors = SyntaxColors::new(&visuals);
        let comment_color = colors.value_color(&JsonValueType::Comment);
        let metadata_color = colors.value_color(&JsonValueType::Metadata);

        assert_eq!(comment_color, COLOR_COMMENT);
        assert_eq!(metadata_color, COLOR_METADATA);
        assert_ne!(comment_color, metadata_color);
        for existing_type in [
            JsonValueType::String,
            JsonValueType::Number,
            JsonValueType::Bool,
            JsonValueType::Null,
            JsonValueType::Object,
            JsonValueType::Array,
        ] {
            let existing_color = colors.value_color(&existing_type);
            assert_ne!(comment_color, existing_color);
            assert_ne!(metadata_color, existing_color);
        }
    }

    fn luminance(color: egui::Color32) -> f32 {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    #[test]
    fn both_palettes_have_readable_contrast_on_their_panel_backgrounds() {
        for visuals in [egui::Visuals::dark(), egui::Visuals::light()] {
            let colors = SyntaxColors::new(&visuals);
            let background = luminance(visuals.panel_fill);
            for color in [
                colors.key,
                colors.string,
                colors.number,
                colors.boolean,
                colors.null,
                colors.metadata,
                colors.comment,
                colors.matched,
                colors.active_match,
                colors.error,
                colors.success,
            ] {
                let foreground = luminance(color);
                let contrast =
                    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "{color:?}: contrast {contrast}, dark={}",
                    visuals.dark_mode
                );
            }
        }
    }
}
