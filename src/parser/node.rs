//! Типы JSON-дерева и ошибки разбора.

/// Тип значения узла структурированных данных.
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
    /// Дата или время TOML, сохраняемое без кавычек при сериализации TOML.
    DateTime,
    /// Комментарий формата, отображаемый отдельным узлом дерева.
    Comment,
    /// YAML-тег, оборачивающий значение в дочернем узле.
    Metadata,
    /// Числовое значение
    Number,
    /// Вещественное числовое значение (сохраняет тип TOML Float).
    Float,
    /// Логическое значение (`true` / `false`)
    Bool,
    /// Значение `null`
    Null,
}

/// Узел JSON-дерева.
///
/// Каждый узел представляет элемент структурированного документа. Объекты и
/// массивы содержат дочерние узлы; TOML-даты и время хранят отдельный тип,
/// а YAML-теги и поддерживаемые форматы комментариев сохраняются как отдельные
/// узлы, чтобы оставаться доступными при отображении и сериализации.
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

/// Ошибка разбора структурированных данных.
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

/// Установить состояние `expanded` рекурсивно для всего дерева.
///
/// # Arguments
///
/// * `node` — корневой узел поддерева.
/// * `expanded` — `true` — развернуть, `false` — свернуть.
///
/// # Examples
///
/// ```
/// use struct_view::parser::{parse_json, set_expanded_all};
///
/// let mut root = parse_json(r#"{"a": {"b": 1}}"#).unwrap();
/// set_expanded_all(&mut root, false);
/// assert!(!root.expanded);
/// assert!(!root.children[0].expanded);
/// ```
pub fn set_expanded_all(node: &mut JsonNode, expanded: bool) {
    node.expanded = expanded;
    for child in &mut node.children {
        set_expanded_all(child, expanded);
    }
}
