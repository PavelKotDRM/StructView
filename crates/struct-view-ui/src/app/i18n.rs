//! Локализация пользовательского интерфейса.

mod catalog;
mod locale;
#[cfg(test)]
mod tests;
mod text_key;

pub(super) use locale::Locale;
pub(super) use text_key::TextKey;

impl Locale {
    pub(super) fn value_type_label(
        self,
        value_type: &struct_view_core::parser::JsonValueType,
    ) -> &'static str {
        use struct_view_core::parser::JsonValueType;
        self.text(match value_type {
            JsonValueType::String => TextKey::TypeString,
            JsonValueType::DateTime => TextKey::TypeDateTime,
            JsonValueType::Comment => TextKey::TypeComment,
            JsonValueType::Metadata => TextKey::TypeMetadata,
            JsonValueType::Number => TextKey::TypeNumber,
            JsonValueType::Float => TextKey::TypeFloat,
            JsonValueType::Bool => TextKey::TypeBoolean,
            JsonValueType::Null => TextKey::TypeNull,
            JsonValueType::Object => TextKey::TypeObject,
            JsonValueType::Array => TextKey::TypeArray,
        })
    }
}
