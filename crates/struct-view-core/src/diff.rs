//! Сравнение нескольких нормализованных структурированных документов.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::numbers;
use crate::parser::build_path;

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

/// Compare a selected pair at an existing difference path.
///
/// A non-empty container replaced by a scalar is expanded into child changes
/// plus a change for the scalar at the container path.
pub fn compare_pair_at_path(
    path: &str,
    left: Option<&Value>,
    right: Option<&Value>,
) -> Vec<Difference> {
    let mut differences = Vec::new();
    match (left, right) {
        (Some(left), Some(right)) if has_container_children(left) && !is_container(right) => {
            collect_differences(path.to_string(), vec![Some(left), None], &mut differences);
            differences.push(Difference {
                path: path.to_string(),
                values: vec![None, Some(right.clone())],
            });
        }
        (Some(left), Some(right)) if !is_container(left) && has_container_children(right) => {
            collect_differences(path.to_string(), vec![None, Some(right)], &mut differences);
            differences.push(Difference {
                path: path.to_string(),
                values: vec![Some(left.clone()), None],
            });
        }
        _ => collect_differences(path.to_string(), vec![left, right], &mut differences),
    }
    differences
}

fn is_container(value: &Value) -> bool {
    value.is_array() || value.is_object()
}

fn has_container_children(value: &Value) -> bool {
    match value {
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        _ => false,
    }
}

/// Compare normalized values, retaining integer/float distinctions but not decimal spelling.
pub fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => numbers::equivalent(left, right),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| values_equal(left, right))
                })
        }
        _ => left == right,
    }
}

/// Преобразовать значение отличия в форматированный JSON.
///
/// Отсутствующий путь отображается отдельно от JSON-значения `null`.
pub fn format_value(value: Option<&Value>) -> String {
    match value {
        None => "<missing>".to_string(),
        Some(value) => serde_json::to_string_pretty(value)
            .unwrap_or_else(|error| format!("<serialization error: {error}>")),
    }
}

fn collect_differences(
    path: String,
    values: Vec<Option<&Value>>,
    differences: &mut Vec<Difference>,
) {
    if values.iter().all(|value| match (*value, values[0]) {
        (Some(left), Some(right)) => values_equal(left, right),
        (None, None) => true,
        _ => false,
    }) {
        return;
    }

    let has_objects = values
        .iter()
        .any(|value| value.is_some_and(Value::is_object));
    let has_arrays = values
        .iter()
        .any(|value| value.is_some_and(Value::is_array));
    let has_scalars = values
        .iter()
        .any(|value| value.is_some_and(|value| !value.is_object() && !value.is_array()));
    if has_scalars && (has_objects || has_arrays) {
        differences.push(Difference {
            path,
            values: values.into_iter().map(|value| value.cloned()).collect(),
        });
        return;
    }

    let has_container_type_mismatch = has_objects && has_arrays;

    if has_container_type_mismatch {
        let objects = values
            .iter()
            .map(|value| value.filter(|value| value.is_object()))
            .collect::<Vec<_>>();
        if objects.iter().any(Option::is_some) {
            collect_differences(path.clone(), objects, differences);
        }

        let arrays = values
            .iter()
            .map(|value| value.filter(|value| value.is_array()))
            .collect::<Vec<_>>();
        if arrays.iter().any(Option::is_some) {
            collect_differences(path.clone(), arrays, differences);
        }

        let scalars = values
            .iter()
            .map(|value| value.filter(|value| !value.is_object() && !value.is_array()))
            .collect::<Vec<_>>();
        if scalars.iter().any(Option::is_some) {
            collect_differences(path, scalars, differences);
        }
        return;
    }

    if values.iter().any(Option::is_some)
        && values
            .iter()
            .all(|value| value.is_none_or(Value::is_object))
    {
        let keys = values
            .iter()
            .filter_map(|value| value.and_then(Value::as_object))
            .flat_map(|object| object.keys().cloned())
            .collect::<BTreeSet<_>>();

        if keys.is_empty() {
            differences.push(Difference {
                path,
                values: values.into_iter().map(|value| value.cloned()).collect(),
            });
            return;
        }

        for key in keys {
            let child_values = values
                .iter()
                .map(|value| value.and_then(|value| value.as_object()?.get(&key)))
                .collect();
            collect_differences(
                build_path(&path, &Some(key), false),
                child_values,
                differences,
            );
        }
        return;
    }

    if values.iter().any(Option::is_some)
        && values.iter().all(|value| value.is_none_or(Value::is_array))
    {
        let length = values
            .iter()
            .filter_map(|value| value.and_then(Value::as_array))
            .map(Vec::len)
            .max()
            .unwrap_or(0);

        if length == 0 {
            differences.push(Difference {
                path,
                values: values.into_iter().map(|value| value.cloned()).collect(),
            });
            return;
        }

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
    fn compares_added_or_removed_object_arrays_by_nested_fields() {
        let values = [
            json!({"edges": [
                {"id": "e1", "relation": "supplies", "source": "a", "target": "b"},
                {"id": "e2", "relation": "supports", "source": "a", "target": "b"}
            ]}),
            json!({}),
        ];

        let differences = compare_values(&values);

        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.as_str())
                .collect::<Vec<_>>(),
            [
                "$.edges[0].id",
                "$.edges[0].relation",
                "$.edges[0].source",
                "$.edges[0].target",
                "$.edges[1].id",
                "$.edges[1].relation",
                "$.edges[1].source",
                "$.edges[1].target",
            ]
        );
        assert!(differences.iter().all(|difference| {
            difference.values[0].is_some() && difference.values[1].is_none()
        }));
    }

    #[test]
    fn compares_objects_replaced_by_arrays_as_removed_and_added_fields() {
        let before = json!({
            "entities": {
                "account": "Account",
                "customer": "Customer",
                "order": "Order"
            }
        });
        let after = json!({
            "entities": [
                {"id": "customer", "name": "Customer"},
                {"id": "order", "name": "Order"}
            ]
        });

        let differences = compare_values(&[before, after]);

        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.as_str())
                .collect::<Vec<_>>(),
            [
                "$.entities.account",
                "$.entities.customer",
                "$.entities.order",
                "$.entities[0].id",
                "$.entities[0].name",
                "$.entities[1].id",
                "$.entities[1].name",
            ]
        );
        assert!(differences[..3].iter().all(|difference| {
            difference.values[0].is_some() && difference.values[1].is_none()
        }));
        assert!(differences[3..].iter().all(|difference| {
            difference.values[0].is_none() && difference.values[1].is_some()
        }));
    }

    #[test]
    fn reports_container_replaced_by_scalar_at_container_path() {
        let before = json!({"settings": {"enabled": true}});
        let after = json!({"settings": false});

        assert_eq!(
            compare_values(&[before, after]),
            [Difference {
                path: "$.settings".to_string(),
                values: vec![Some(json!({"enabled": true})), Some(json!(false)),],
            }]
        );
    }

    #[test]
    fn empty_containers_still_report_a_difference_when_missing() {
        for value in [json!({}), json!([])] {
            let differences = compare_pair_at_path("$", Some(&value), None);
            assert_eq!(differences.len(), 1);
            assert_eq!(differences[0].path, "$");
            assert_eq!(differences[0].values[0], Some(value));
        }

        let object = json!({});
        let array = json!([]);
        assert!(compare_pair_at_path("$", Some(&object), Some(&object)).is_empty());
        assert!(compare_pair_at_path("$", Some(&array), Some(&array)).is_empty());
    }

    #[test]
    fn pair_comparison_expands_non_empty_containers_replaced_by_scalars() {
        let object = json!({"items": [1, 2], "unchanged": true});
        let scalar = Value::Null;

        assert_eq!(
            compare_pair_at_path("$", Some(&object), Some(&scalar)),
            [
                Difference {
                    path: "$.items[0]".to_string(),
                    values: vec![Some(json!(1)), None],
                },
                Difference {
                    path: "$.items[1]".to_string(),
                    values: vec![Some(json!(2)), None],
                },
                Difference {
                    path: "$.unchanged".to_string(),
                    values: vec![Some(json!(true)), None],
                },
                Difference {
                    path: "$".to_string(),
                    values: vec![None, Some(Value::Null)],
                },
            ]
        );
        assert_eq!(
            compare_pair_at_path("$", Some(&scalar), Some(&object)),
            [
                Difference {
                    path: "$.items[0]".to_string(),
                    values: vec![None, Some(json!(1))],
                },
                Difference {
                    path: "$.items[1]".to_string(),
                    values: vec![None, Some(json!(2))],
                },
                Difference {
                    path: "$.unchanged".to_string(),
                    values: vec![None, Some(json!(true))],
                },
                Difference {
                    path: "$".to_string(),
                    values: vec![Some(Value::Null), None],
                },
            ]
        );

        let empty_object = json!({});
        assert_eq!(
            compare_pair_at_path("$", Some(&empty_object), Some(&scalar)),
            [Difference {
                path: "$".to_string(),
                values: vec![Some(empty_object), Some(Value::Null)],
            }]
        );
    }

    #[test]
    fn compares_arrays_and_replaced_container_types_recursively() {
        let values = [json!([1, 2]), json!([1, 3, 4]), json!({"items": true})];

        let differences = compare_values(&values);

        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.as_str())
                .collect::<Vec<_>>(),
            ["$.items", "$[0]", "$[1]", "$[2]"]
        );
        assert_eq!(differences[0].values, vec![None, None, Some(json!(true))]);
        assert_eq!(
            differences[1].values,
            vec![Some(json!(1)), Some(json!(1)), None]
        );
        assert_eq!(
            differences[2].values,
            vec![Some(json!(2)), Some(json!(3)), None]
        );
        assert_eq!(differences[3].values, vec![None, Some(json!(4)), None]);
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
    fn formats_nested_values_with_indentation() {
        let value = json!({"profile": {"age": 30}});

        assert_eq!(
            format_value(Some(&value)),
            "{\n  \"profile\": {\n    \"age\": 30\n  }\n}"
        );
    }

    #[test]
    fn escapes_non_identifier_object_keys() {
        let values = [json!({"a-b": 1}), json!({"a-b": 2})];
        assert_eq!(compare_values(&values)[0].path, "$[\"a-b\"]");
    }
}
