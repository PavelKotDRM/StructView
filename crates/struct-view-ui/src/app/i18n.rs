//! Локализация пользовательского интерфейса.

mod catalog;
mod locale;
#[cfg(test)]
mod tests;
mod text_key;

pub(super) use locale::Locale;
pub(super) use text_key::TextKey;
