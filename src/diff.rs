//! Сравнение нескольких нормализованных структурированных документов.

use std::collections::BTreeSet;

use serde_json::Value;

/// Одно отличие между документами.
///
/// Значения идут в том же порядке, что и входной список документов.
/// `None` означает, что путь отсутствует в соответствующем документе.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    /// JSON-путь изменившегося значения (`$`, `$.user.name`, `$[0]` и т. п.).
    pub path: String,
    /// Значение по пути в каждом сравниваемом документе.
    pub values: Vec<Option<Value>>,
}

/// Найти все отличия между двумя или более JSON-значениями.
///
/// Объекты и массивы сравниваются рекурсивно, поэтому в результате указывается
/// наиболее глубокий путь, на котором значения расходятся. Если тип узла
/// различается (например, объект в одном документе и число в другом),
/// отличие фиксируется на самом узле.
pub fn compare_values(values: &[Value]) -> Vec<Difference> {
    if values.len() < 2 {
        return Vec::new();
    }

    let references = values.iter().map(Some).collect();
    let mut differences = Vec::new();
    collect_differences("$".to_string(), references, &mut differences);
    differences
}

/// Преобразовать значение отличия в компактное человекочитаемое представление.
///
/// Отсутствующий путь отображается отдельно от JSON-значения `null`.
pub fn format_value(value: Option<&Value>) -> String {
    match value {
        None => "<missing>".to_string(),
        Some(value) => serde_json::to_string(value)
            .unwrap_or_else(|error| format!("<serialization error: {error}>")),
    }
}

fn collect_differences(
    path: String,
    values: Vec<Option<&Value>>,
    differences: &mut Vec<Difference>,
) {
    if values.iter().all(|value| *value == values[0]) {
        return;
    }

    if values
        .iter()
        .all(|value| value.is_some_and(Value::is_object))
    {
        let keys = values
            .iter()
            .filter_map(|value| value.and_then(Value::as_object))
            .flat_map(|object| object.keys().cloned())
            .collect::<BTreeSet<_>>();

        for key in keys {
            let child_values = values
                .iter()
                .map(|value| value.and_then(|value| value.as_object()?.get(&key)))
                .collect();
            collect_differences(object_path(&path, &key), child_values, differences);
        }
        return;
    }

    if values
        .iter()
        .all(|value| value.is_some_and(Value::is_array))
    {
        let length = values
            .iter()
            .filter_map(|value| value.and_then(Value::as_array))
            .map(Vec::len)
            .max()
            .unwrap_or(0);

        for index in 0..length {
            let child_values = values
                .iter()
                .map(|value| value.and_then(|value| value.as_array()?.get(index)))
                .collect();
            collect_differences(format!("{path}[{index}]"), child_values, differences);
        }
        return;
    }

    differences.push(Difference {
        path,
        values: values.into_iter().map(|value| value.cloned()).collect(),
    });
}

fn object_path(parent: &str, key: &str) -> String {
    if is_identifier(key) {
        format!("{parent}.{key}")
    } else {
        let encoded = serde_json::to_string(key)
            .unwrap_or_else(|error| format!("\"<key serialization error: {error}>\""));
        format!("{parent}[{encoded}]")
    }
}

fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_nested_objects_and_missing_fields() {
        let values = [
            json!({"name": "Alice", "profile": {"age": 30}}),
            json!({"name": "Bob", "profile": {}}),
            json!({"name": "Alice", "profile": {"age": 31}}),
        ];

        let differences = compare_values(&values);

        assert_eq!(
            differences,
            [
                Difference {
                    path: "$.name".to_string(),
                    values: vec![
                        Some(json!("Alice")),
                        Some(json!("Bob")),
                        Some(json!("Alice"))
                    ],
                },
                Difference {
                    path: "$.profile.age".to_string(),
                    values: vec![Some(json!(30)), None, Some(json!(31))],
                },
            ]
        );
    }

    #[test]
    fn compares_arrays_and_type_changes() {
        let values = [json!([1, 2]), json!([1, 3, 4]), json!({"items": true})];

        let differences = compare_values(&values);

        assert_eq!(differences.len(), 1);
        assert_eq!(differences[0].path, "$");
        assert_eq!(
            differences[0].values,
            vec![
                Some(json!([1, 2])),
                Some(json!([1, 3, 4])),
                Some(json!({"items": true})),
            ]
        );
    }

    #[test]
    fn equal_values_have_no_differences() {
        let values = [json!({"value": 1}), json!({"value": 1})];
        assert!(compare_values(&values).is_empty());
        assert!(compare_values(&[json!(1)]).is_empty());
    }

    #[test]
    fn formats_missing_and_null_differently() {
        assert_eq!(format_value(None), "<missing>");
        assert_eq!(format_value(Some(&Value::Null)), "null");
    }

    #[test]
    fn escapes_non_identifier_object_keys() {
        let values = [json!({"a-b": 1}), json!({"a-b": 2})];
        assert_eq!(compare_values(&values)[0].path, "$[\"a-b\"]");
    }
}
