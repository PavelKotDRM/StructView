//! # Модуль парсинга структурированных данных
//!
//! Содержит структуры и функции для разбора JSON, YAML, TOML и JSON5 в дерево узлов,
//! пригодное для отображения в дереве графического интерфейса.
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
//! //! | `build::format` | Определение поддерживаемого формата |
//! | `build::parse` | Разбор документов и построение исходного дерева |
//! | `build::serialize` | Сериализация деревьев и преобразование в значения |
//! | `build::comments` | Извлечение и сохранение комментариев |
//! | `build::tree` | Построение узлов для JSON, YAML и TOML |
//! | `build::paths` | Формирование путей и вспомогательные функции |

mod build;
mod node;

pub use build::{
    DataFormat, build_path, comment_input, format_comment_for_format, node_to_value, parse_data,
    parse_json, plural_ru, serialize_data, serialize_node,
};
pub use node::{JsonNode, JsonValueType, ParseError, set_expanded_all};
