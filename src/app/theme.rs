//! Цветовая схема приложения.
//!
//! Задаёт цвета подсветки синтаксиса JSON и выделения результатов поиска.
//! Цвета подобраны так, чтобы оставаться читаемыми в тёмной и светлой темах.

use egui::Color32;

use crate::parser::JsonValueType;

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
/// Цвет для подсветки совпадений при поиске (жёлтый).
pub(super) const COLOR_MATCH: Color32 = Color32::from_rgb(229, 192, 73);
/// Цвет для активного совпадения при поиске (ярко-оранжевый).
pub(super) const COLOR_ACTIVE_MATCH: Color32 = Color32::from_rgb(255, 120, 50);
/// Цвет сообщений об ошибках (красный).
pub(super) const COLOR_ERROR: Color32 = Color32::from_rgb(220, 80, 80);
/// Цвет успешных уведомлений (зелёный).
pub(super) const COLOR_SUCCESS: Color32 = Color32::from_rgb(100, 200, 100);

/// Вернуть цвет, соответствующий типу значения JSON.
///
/// Объекты и массивы окрашиваются как ключи, поскольку их отображаемое
/// значение — служебная подпись вида `{3 поля}`.
pub(super) fn value_color(vtype: &JsonValueType) -> Color32 {
    match vtype {
        JsonValueType::String => COLOR_STRING,
        JsonValueType::DateTime => COLOR_STRING,
        JsonValueType::Number => COLOR_NUMBER,
        JsonValueType::Float => COLOR_NUMBER,
        JsonValueType::Bool => COLOR_BOOL,
        JsonValueType::Null => COLOR_NULL,
        JsonValueType::Object | JsonValueType::Array => COLOR_KEY,
    }
}
