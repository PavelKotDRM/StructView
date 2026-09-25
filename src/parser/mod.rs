//! # Модуль парсинга структурированных данных
//!
//! Содержит структуры и функции для разбора JSON, YAML, TOML и JSON5 в дерево узлов,
//! пригодное для отображения в древовидном представлении [`egui`].
//!
//! ## Основные типы
//! - [`JsonNode`] — узел дерева, хранящий тип, ключ, значение и дочерние узлы.
//! - [`ParseError`] — ошибка парсинга с указанием строки и позиции.
//! - [`parse_data`] — разбор поддерживаемых форматов с автодетектом.
//! - [`parse_json`] — совместимый API для строгого разбора JSON.
//!
//! ## Состав подмодулей
//!
//! | Подмодуль | Назначение |
//! |-----------|-----------|
//! | `node` | Типы дерева: [`JsonNode`], [`JsonValueType`], [`ParseError`] |
//! | `build` | Построение дерева из [`serde_json::Value`] и формирование путей |

mod build;
mod node;

pub use build::{
    DataFormat, build_path, parse_data, parse_json, plural_ru, serialize_data, serialize_node,
};
pub(crate) use build::{comment_input, format_comment_for_format, node_to_value};
pub use node::{JsonNode, JsonValueType, ParseError, set_expanded_all};
