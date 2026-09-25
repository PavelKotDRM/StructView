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
/// use struct_view_core::parser::build_path;
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
/// use struct_view_core::parser::plural_ru;
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
