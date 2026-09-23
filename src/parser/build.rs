//! Построение [`JsonNode`]-дерева из структурированных текстовых форматов.

use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::node::{JsonNode, JsonValueType, ParseError};

/// Поддерживаемый формат структурированных данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// JavaScript Object Notation.
    Json,
    /// YAML Ain't Markup Language.
    Yaml,
    /// Tom's Obvious Minimal Language.
    Toml,
    /// Расширенный синтаксис JSON с комментариями и trailing commas.
    Json5,
}

impl DataFormat {
    /// Все форматы, доступные для преобразования и сохранения.
    pub const ALL: [Self; 4] = [Self::Json, Self::Yaml, Self::Toml, Self::Json5];

    /// Определить формат по расширению пути.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "json5" => Some(Self::Json5),
            _ => None,
        }
    }

    /// Основное расширение файла без точки.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json5 => "json5",
        }
    }

    /// Расширения, принимаемые диалогом выбора файла для этого формата.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Json => &["json"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Json5 => &["json5"],
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Json5 => "JSON5",
        })
    }
}

/// Разобрать данные указанного формата или определить формат автоматически.
///
/// При успешном автодетекте возвращает фактически использованный формат.
///
/// # Errors
///
/// Возвращает [`ParseError`], если содержимое не соответствует указанному
/// формату или ни одному из поддерживаемых форматов при автодетекте.
pub fn parse_data(
    input: &str,
    format: Option<DataFormat>,
) -> Result<(JsonNode, DataFormat), ParseError> {
    if let Some(format) = format {
        let value = parse_value(input, format)?;
        return Ok((build_node(None, false, &value, String::new()), format));
    }

    let mut errors = Vec::new();
    for candidate in [
        DataFormat::Json,
        DataFormat::Toml,
        DataFormat::Json5,
        DataFormat::Yaml,
    ] {
        match parse_value(input, candidate) {
            Ok(value) => return Ok((build_node(None, false, &value, String::new()), candidate)),
            Err(error) => errors.push(format!("{}: {}", candidate, error.message)),
        }
    }

    Err(ParseError {
        message: format!(
            "Не удалось определить формат. Поддерживаются JSON, YAML, TOML и JSON5.\n{}",
            errors.join("\n")
        ),
        line: None,
        column: None,
    })
}

/// Сериализовать значение в выбранном формате.
///
/// # Errors
///
/// Возвращает ошибку, если значение нельзя представить в выбранном формате.
pub fn serialize_data(value: &Value, format: DataFormat, compact: bool) -> Result<String, String> {
    match format {
        DataFormat::Json | DataFormat::Json5 => if compact {
            serde_json::to_string(value)
        } else {
            serde_json::to_string_pretty(value)
        }
        .map_err(|error| format!("Ошибка сериализации {}: {}", format, error)),
        DataFormat::Yaml => serde_yaml_ng::to_string(value)
            .map_err(|error| format!("Ошибка сериализации YAML: {}", error)),
        DataFormat::Toml => if compact {
            toml::to_string(value)
        } else {
            toml::to_string_pretty(value)
        }
        .map_err(|error| format!("Ошибка сериализации TOML: {}", error)),
    }
}

fn parse_value(input: &str, format: DataFormat) -> Result<Value, ParseError> {
    match format {
        DataFormat::Json => serde_json::from_str(input).map_err(|error| ParseError {
            message: error.to_string(),
            line: Some(error.line()),
            column: Some(error.column()),
        }),
        DataFormat::Yaml => parse_yaml_documents(input),
        DataFormat::Toml => toml::from_str(input).map_err(|error| ParseError {
            message: error.to_string(),
            line: None,
            column: None,
        }),
        DataFormat::Json5 => json5::from_str(input).map_err(|error| ParseError {
            message: error.to_string(),
            line: None,
            column: None,
        }),
    }
}

/// Разобрать YAML-поток из одного или нескольких документов.
fn parse_yaml_documents(input: &str) -> Result<Value, ParseError> {
    let documents = serde_yaml_ng::Deserializer::from_str(input)
        .map(serde_yaml_ng::Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            let location = error.location();
            ParseError {
                message: error.to_string(),
                line: location.as_ref().map(serde_yaml_ng::Location::line),
                column: location.as_ref().map(serde_yaml_ng::Location::column),
            }
        })?;

    match documents.as_slice() {
        [] => Ok(Value::Null),
        [document] => Ok(yaml_value_to_json(document.clone())),
        _ => Ok(Value::Array(
            documents.into_iter().map(yaml_value_to_json).collect(),
        )),
    }
}

/// Преобразовать YAML-значение в представление дерева.
///
/// JSON требует строковые ключи объектов, а YAML допускает значения любой
/// структуры. Составные YAML-ключи представляются компактной JSON-строкой и
/// остаются читаемыми в дереве, например `["name","age"]`.
fn yaml_value_to_json(value: serde_yaml_ng::Value) -> Value {
    match value {
        serde_yaml_ng::Value::Null => Value::Null,
        serde_yaml_ng::Value::Bool(value) => Value::Bool(value),
        serde_yaml_ng::Value::Number(value) => Value::Number(yaml_number_to_json(value)),
        serde_yaml_ng::Value::String(value) => Value::String(value),
        serde_yaml_ng::Value::Sequence(values) => {
            Value::Array(values.into_iter().map(yaml_value_to_json).collect())
        }
        serde_yaml_ng::Value::Mapping(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (yaml_key_to_string(key), yaml_value_to_json(value)))
                .collect(),
        ),
        serde_yaml_ng::Value::Tagged(tagged) => yaml_value_to_json(tagged.value),
    }
}

/// Создать отображаемое имя YAML-ключа, включая составные ключи.
fn yaml_key_to_string(key: serde_yaml_ng::Value) -> String {
    match key {
        serde_yaml_ng::Value::String(value) => value,
        other => serde_json::to_string(&yaml_value_to_json(other))
            .unwrap_or_else(|_| "<неподдерживаемый ключ YAML>".to_string()),
    }
}

/// Сохранить целочисленное YAML-число без промежуточного `f64`.
fn yaml_number_to_json(value: serde_yaml_ng::Number) -> serde_json::Number {
    if let Some(value) = value.as_i64() {
        serde_json::Number::from(value)
    } else if let Some(value) = value.as_u64() {
        serde_json::Number::from(value)
    } else {
        serde_json::Number::from_f64(value.as_f64().unwrap_or_default())
            .unwrap_or_else(|| serde_json::Number::from(0))
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
    parse_data(input, Some(DataFormat::Json)).map(|(node, _)| node)
}

/// Рекурсивно построить [`JsonNode`] из [`serde_json::Value`].
///
/// # Arguments
///
/// * `key` — ключ текущего узла (имя поля или индекс массива).
/// * `is_index` — `true`, если `key` — индекс родительского массива, а не
///   имя поля родительского объекта.
/// * `value` — разобранное значение JSON.
/// * `parent_path` — путь родительского узла.
fn build_node(key: Option<String>, is_index: bool, value: &Value, parent_path: String) -> JsonNode {
    let path = build_path(&parent_path, &key, is_index);
    match value {
        Value::Object(map) => {
            let children = map
                .iter()
                .map(|(k, v)| build_node(Some(k.clone()), false, v, path.clone()))
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
                expanded: false,
                path,
            }
        }
        Value::Array(arr) => {
            let children = arr
                .iter()
                .enumerate()
                .map(|(i, v)| build_node(Some(i.to_string()), true, v, path.clone()))
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
                expanded: false,
                path,
            }
        }
        Value::String(s) => leaf(
            key,
            JsonValueType::String,
            serde_json::Value::String(s.clone()).to_string(),
            path,
        ),
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
/// Индексы массивов оборачиваются в квадратные скобки: `arr[0]`. Ключи
/// объектов, являющиеся простым идентификатором, разделяются точкой:
/// `obj.field`. Остальные ключи объектов (пустая строка, число, содержащие
/// `.`/`[`/`]` и т. п.) оборачиваются в квадратные скобки с JSON-строкой:
/// `obj["0"]`, `obj["a.b"]`. Это исключает совпадение пути поля объекта
/// с путём индекса массива или с путём вложенного объекта.
///
/// `is_index` указывает, что `key` — индекс контейнера-массива, а не имя
/// поля объекта; вызывающий код определяет это по типу родительского узла,
/// а не по содержимому строки ключа.
///
/// # Examples
///
/// ```
/// use json_viewer::parser::build_path;
///
/// assert_eq!(build_path("store.book", &Some("2".to_string()), true), "store.book[2]");
/// assert_eq!(build_path("store", &Some("title".to_string()), false), "store.title");
/// assert_eq!(build_path("", &Some("root".to_string()), false), "root");
/// assert_eq!(build_path("root", &None, false), "root");
/// // Ключ объекта, совпадающий по написанию с индексом массива, экранируется.
/// assert_eq!(build_path("store", &Some("0".to_string()), false), "store[\"0\"]");
/// ```
pub fn build_path(parent: &str, key: &Option<String>, is_index: bool) -> String {
    match key {
        None => parent.to_string(),
        Some(k) => {
            if is_index {
                if parent.is_empty() {
                    k.clone()
                } else {
                    format!("{}[{}]", parent, k)
                }
            } else {
                object_path_segment(parent, k)
            }
        }
    }
}

/// Построить путь к полю объекта, экранируя ключи, которые иначе были бы
/// неотличимы от индекса массива или от разделителя вложенности.
fn object_path_segment(parent: &str, key: &str) -> String {
    if is_plain_identifier(key) {
        if parent.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", parent, key)
        }
    } else {
        let quoted = serde_json::to_string(key)
            .unwrap_or_else(|error| format!("\"<key serialization error: {error}>\""));
        if parent.is_empty() {
            format!("[{}]", quoted)
        } else {
            format!("{}[{}]", parent, quoted)
        }
    }
}

/// Проверить, что ключ можно безопасно записать через точку без экранирования.
///
/// Ключ должен начинаться с буквы или `_` и состоять только из букв, цифр и
/// `_`. Это, в частности, отсекает ключи, которые целиком состоят из цифр
/// (совпадают по написанию с индексом массива) и ключи с `.`, `[`, `]`.
fn is_plain_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml_array() {
        let (root, format) = parse_data("- one\n- two\n", Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(root.value_type, JsonValueType::Array);
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn parsed_containers_are_collapsed_by_default() {
        let root = parse_json(r#"{"object":{"value":1},"array":[{"value":2}]}"#).unwrap();

        assert!(!root.expanded);
        assert!(!root.children[0].expanded);
        assert!(!root.children[1].expanded);
        assert!(!root.children[1].children[0].expanded);
    }

    #[test]
    fn parses_toml_table() {
        let (root, format) = parse_data("[server]\nport = 8080\n", None).unwrap();
        assert_eq!(format, DataFormat::Toml);
        assert_eq!(root.children[0].key.as_deref(), Some("server"));
    }

    #[test]
    fn parses_json5_comments_and_trailing_comma() {
        let (root, format) = parse_data("{ // comment\n key: true,\n}", None).unwrap();
        assert_eq!(format, DataFormat::Json5);
        assert_eq!(root.children[0].key.as_deref(), Some("key"));
    }

    #[test]
    fn object_keys_that_look_like_array_indices_are_escaped_in_the_path() {
        // Объект с числовым ключом "0" не должен получить тот же path, что и
        // первый элемент массива (`arr[0]`), иначе find_node/selected_paths
        // не смогут различить эти узлы.
        let root = parse_json(r#"{"0": "object field"}"#).unwrap();
        assert_eq!(root.children[0].key.as_deref(), Some("0"));
        assert_eq!(root.children[0].path, "[\"0\"]");

        let array_root = parse_json(r#"["array element"]"#).unwrap();
        assert_eq!(array_root.children[0].path, "0");
        assert_ne!(root.children[0].path, array_root.children[0].path);
    }

    #[test]
    fn object_keys_with_dots_or_brackets_are_escaped_in_the_path() {
        let root = parse_json(r#"{"a": {"b.c": 1, "d[e]": 2}}"#).unwrap();
        let nested = &root.children[0];
        assert_eq!(nested.path, "a");
        let b_c = nested
            .children
            .iter()
            .find(|c| c.key.as_deref() == Some("b.c"))
            .unwrap();
        assert_eq!(b_c.path, "a[\"b.c\"]");
        let d_e = nested
            .children
            .iter()
            .find(|c| c.key.as_deref() == Some("d[e]"))
            .unwrap();
        assert_eq!(d_e.path, "a[\"d[e]\"]");
    }

    #[test]
    fn reports_invalid_yaml_location() {
        let error = parse_data("key: [broken", Some(DataFormat::Yaml)).unwrap_err();
        assert!(error.line.is_some());
    }

    #[test]
    fn parses_yaml_sequence_key() {
        let source = "- [name, age]: [Rae Smith, 4]";
        let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(
            root.children[0].children[0].key.as_deref(),
            Some("[\"name\",\"age\"]")
        );
        assert_eq!(
            root.children[0].children[0].children[0].display_value,
            "\"Rae Smith\""
        );
    }

    #[test]
    fn parses_yaml_stream_with_sequence_key() {
        let source = r#"--- # The Smiths
- {name: John Smith, age: 33}
- name: Mary Smith
  age: 27
- [name, age]: [Rae Smith, 4]
--- # People, by gender
men: [John Smith, Bill Jones]
women:
  - Mary Smith
  - Susan Williams"#;
        let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
        assert_eq!(format, DataFormat::Yaml);
        assert_eq!(root.value_type, JsonValueType::Array);
        assert_eq!(root.children.len(), 2);
        assert_eq!(
            root.children[0].children[2].children[0].key.as_deref(),
            Some("[\"name\",\"age\"]")
        );
        assert_eq!(root.children[1].children[0].key.as_deref(), Some("men"));
    }

    #[test]
    fn serializes_toml_table() {
        let value = serde_json::json!({"server": {"port": 8080}});
        let output = serialize_data(&value, DataFormat::Toml, false).unwrap();
        assert!(output.contains("[server]"));
        assert!(output.contains("port = 8080"));
    }
}
