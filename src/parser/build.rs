//! Построение [`JsonNode`]-дерева из текста JSON.

use serde_json::Value;

use super::node::{JsonNode, JsonValueType, ParseError};

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
        Value::String(s) => leaf(key, JsonValueType::String, format!("\"{}\"", s), path),
        Value::Number(n) => leaf(key, JsonValueType::Number, n.to_string(), path),
        Value::Bool(b) => leaf(key, JsonValueType::Bool, b.to_string(), path),
        Value::Null => leaf(key, JsonValueType::Null, "null".to_string(), path),
    }
}

/// Создать листовой (бездетный) узел дерева.
fn leaf(
    key: Option<String>,
    value_type: JsonValueType,
    display_value: String,
    path: String,
) -> JsonNode {
    JsonNode {
        key,
        value_type,
        display_value,
        children: vec![],
        expanded: false,
        path,
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
