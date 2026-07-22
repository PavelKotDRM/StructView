//! # Модуль парсинга JSON
//!
//! Содержит структуры и функции для разбора JSON-файлов в дерево узлов,
//! пригодное для отображения в древовидном представлении [`egui`].
//!
//! ## Основные типы
//! - [`JsonNode`] — узел дерева, хранящий тип, ключ, значение и дочерние узлы.
//! - [`ParseError`] — ошибка парсинга с указанием строки и позиции.
//! - [`parse_json`] — функция преобразования строки в [`JsonNode`].

use serde_json::Value;

/// Тип значения JSON-узла.
///
/// Используется для цветовой маркировки узлов дерева.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValueType {
    /// Объект `{…}`
    Object,
    /// Массив `[…]`
    Array,
    /// Строковое значение
    String,
    /// Числовое значение
    Number,
    /// Логическое значение (`true` / `false`)
    Bool,
    /// Значение `null`
    Null,
}

/// Узел JSON-дерева.
///
/// Каждый узел представляет один элемент JSON: объект, массив, строку,
/// число, булево или null. Объекты и массивы содержат дочерние узлы.
#[derive(Debug, Clone)]
pub struct JsonNode {
    /// Ключ (имя поля) или индекс элемента массива. `None` для корневого узла.
    pub key: Option<String>,
    /// Тип значения этого узла.
    pub value_type: JsonValueType,
    /// Текстовое представление значения для конечных узлов (строки, числа, bool, null).
    /// Для объектов и массивов содержит количество дочерних элементов в формате `{N}` / `[N]`.
    pub display_value: String,
    /// Дочерние узлы (для объектов и массивов).
    pub children: Vec<JsonNode>,
    /// Состояние раскрытия узла в дереве.
    pub expanded: bool,
    /// Абсолютный путь к узлу (например, `store.book[2].author`).
    pub path: String,
}

/// Ошибка разбора JSON.
///
/// Содержит человекочитаемое описание, а также (если доступно) строку и столбец
/// в исходном тексте, где обнаружена ошибка.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Описание ошибки.
    pub message: String,
    /// Номер строки (1-based), если известен.
    pub line: Option<usize>,
    /// Номер столбца (1-based), если известен.
    pub column: Option<usize>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "{} (строка {}, позиция {})", self.message, l, c),
            (Some(l), None) => write!(f, "{} (строка {})", self.message, l),
            _ => write!(f, "{}", self.message),
        }
    }
}

/// Разобрать строку с JSON-содержимым в дерево [`JsonNode`].
///
/// Возвращает корневой узел дерева или [`ParseError`] с указанием места ошибки.
///
/// # Errors
///
/// Возвращает [`ParseError`], если входная строка содержит синтаксическую
/// ошибку JSON. Поля `line` и `column` заполняются из информации `serde_json`.
///
/// # Examples
///
/// ```
/// use json_viewer::parser::parse_json;
///
/// let node = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
/// assert_eq!(node.children.len(), 2);
/// ```
pub fn parse_json(input: &str) -> Result<JsonNode, ParseError> {
    let value: Value = serde_json::from_str(input).map_err(|e| ParseError {
        message: e.to_string(),
        line: Some(e.line()),
        column: Some(e.column()),
    })?;
    let root = build_node(None, &value, String::new());
    Ok(root)
}

/// Рекурсивно построить [`JsonNode`] из [`serde_json::Value`].
///
/// # Arguments
///
/// * `key` — ключ текущего узла (имя поля или индекс массива).
/// * `value` — разобранное значение JSON.
/// * `parent_path` — путь родительского узла.
fn build_node(key: Option<String>, value: &Value, parent_path: String) -> JsonNode {
    let path = build_path(&parent_path, &key);
    match value {
        Value::Object(map) => {
            let children = map
                .iter()
                .map(|(k, v)| build_node(Some(k.clone()), v, path.clone()))
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Object,
                display_value: format!(
                    "{{{}}} {}",
                    count,
                    plural_ru(count, "поле", "поля", "полей")
                ),
                children,
                expanded: true,
                path,
            }
        }
        Value::Array(arr) => {
            let children = arr
                .iter()
                .enumerate()
                .map(|(i, v)| build_node(Some(i.to_string()), v, path.clone()))
                .collect::<Vec<_>>();
            let count = children.len();
            JsonNode {
                key,
                value_type: JsonValueType::Array,
                display_value: format!(
                    "[{}] {}",
                    count,
                    plural_ru(count, "элемент", "элемента", "элементов")
                ),
                children,
                expanded: true,
                path,
            }
        }
        Value::String(s) => JsonNode {
            key,
            value_type: JsonValueType::String,
            display_value: format!("\"{}\"", s),
            children: vec![],
            expanded: false,
            path,
        },
        Value::Number(n) => JsonNode {
            key,
            value_type: JsonValueType::Number,
            display_value: n.to_string(),
            children: vec![],
            expanded: false,
            path,
        },
        Value::Bool(b) => JsonNode {
            key,
            value_type: JsonValueType::Bool,
            display_value: b.to_string(),
            children: vec![],
            expanded: false,
            path,
        },
        Value::Null => JsonNode {
            key,
            value_type: JsonValueType::Null,
            display_value: "null".to_string(),
            children: vec![],
            expanded: false,
            path,
        },
    }
}

/// Построить путь к узлу из пути родителя и ключа текущего узла.
///
/// Индексы массивов оборачиваются в квадратные скобки: `arr[0]`.
/// Ключи объектов разделяются точкой: `obj.field`.
///
/// # Examples
///
/// ```
/// use json_viewer::parser::build_path;
///
/// assert_eq!(build_path("store.book", &Some("2".to_string())), "store.book[2]");
/// assert_eq!(build_path("store", &Some("title".to_string())), "store.title");
/// assert_eq!(build_path("", &Some("root".to_string())), "root");
/// assert_eq!(build_path("root", &None), "root");
/// ```
pub fn build_path(parent: &str, key: &Option<String>) -> String {
    match key {
        None => parent.to_string(),
        Some(k) => {
            // Если ключ — число, считаем его индексом массива
            let is_index = k.parse::<usize>().is_ok();
            if parent.is_empty() {
                k.clone()
            } else if is_index {
                format!("{}[{}]", parent, k)
            } else {
                format!("{}.{}", parent, k)
            }
        }
    }
}

/// Вспомогательная функция для русских форм множественного числа.
///
/// Возвращает нужную форму слова в зависимости от числа `n`.
///
/// # Examples
///
/// ```
/// use json_viewer::parser::plural_ru;
///
/// assert_eq!(plural_ru(1, "поле", "поля", "полей"), "поле");
/// assert_eq!(plural_ru(3, "поле", "поля", "полей"), "поля");
/// assert_eq!(plural_ru(11, "поле", "поля", "полей"), "полей");
/// ```
pub fn plural_ru<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let rem100 = n % 100;
    let rem10 = n % 10;
    if (11..=19).contains(&rem100) {
        many
    } else if rem10 == 1 {
        one
    } else if (2..=4).contains(&rem10) {
        few
    } else {
        many
    }
}

/// Установить состояние `expanded` рекурсивно для всего дерева.
///
/// # Arguments
///
/// * `node` — корневой узел поддерева.
/// * `expanded` — `true` — развернуть, `false` — свернуть.
pub fn set_expanded_all(node: &mut JsonNode, expanded: bool) {
    node.expanded = expanded;
    for child in &mut node.children {
        set_expanded_all(child, expanded);
    }
}
