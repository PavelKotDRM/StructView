//! # Модуль буфера обмена
//!
//! Предоставляет вспомогательные функции для копирования текста и
//! иерархических структур в системный буфер обмена через библиотеку
//! [`arboard`].

use arboard::Clipboard;
use serde_json::{Value, json};

/// Один узел, сохранённый в буфере структур.
///
/// Ключ хранится отдельно от значения, чтобы при вставке в другой объект
/// можно было восстановить имя поля. В корне документа ключ отсутствует.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardEntry {
    /// Имя поля или индекс элемента исходного родителя.
    pub key: Option<String>,
    /// Структурированное значение узла.
    pub value: Value,
}

const CLIPBOARD_MARKER: &str = "$json_viewer_selection";
const CLIPBOARD_VERSION: u64 = 1;

/// Скопировать строку `text` в системный буфер обмена.
///
/// Открывает соединение с буфером обмена, устанавливает текст и немедленно
/// закрывает соединение. Ошибки подавляются и возвращаются как `Err(String)`.
///
/// # Errors
///
/// Возвращает `Err` с описанием, если не удалось открыть буфер обмена или
/// записать текст (например, при отсутствии дисплея в среде без GUI).
///
/// # Examples
///
/// ```no_run
/// use json_viewer::clipboard::copy_to_clipboard;
///
/// copy_to_clipboard("Hello, clipboard!").expect("clipboard unavailable");
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

/// Прочитать текст из системного буфера обмена.
///
/// # Errors
///
/// Возвращает ошибку, если системный буфер обмена недоступен или не содержит
/// текста.
pub fn read_from_clipboard() -> Result<String, String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.get_text().map_err(|e| e.to_string())
}

/// Сериализовать выбранные узлы в JSON-конверт для системного буфера обмена.
///
/// Конверт остаётся обычным JSON-текстом, но содержит ключи каждого
/// выбранного узла. Поэтому несколько полей объекта и вложенные структуры
/// можно восстановить независимо от формата открытого файла.
///
/// # Errors
///
/// Возвращает ошибку для пустого списка или при сериализации.
pub fn encode_structures(entries: &[ClipboardEntry]) -> Result<String, String> {
    if entries.is_empty() {
        return Err("Не выбрано ни одной структуры".to_string());
    }

    let nodes = entries
        .iter()
        .map(|entry| json!({ "key": entry.key, "value": entry.value }))
        .collect::<Vec<_>>();
    let envelope = json!({
        CLIPBOARD_MARKER: CLIPBOARD_VERSION,
        "nodes": nodes,
    });

    serde_json::to_string_pretty(&envelope)
        .map_err(|error| format!("Ошибка сериализации буфера: {}", error))
}

/// Разобрать конверт структур или обычное JSON-значение из буфера обмена.
///
/// Обычный JSON поддерживается для вставки данных, скопированных из других
/// приложений. У такого значения нет ключа, поэтому объект вставляется в
/// объект-назначение своими полями, а остальные значения требуют массива.
///
/// # Errors
///
/// Возвращает ошибку, если текст не является JSON или конверт имеет неверную
/// структуру.
pub fn decode_structures(text: &str) -> Result<Vec<ClipboardEntry>, String> {
    let value = serde_json::from_str::<Value>(text.trim())
        .map_err(|error| format!("Буфер не содержит корректный JSON: {}", error))?;

    let Some(object) = value.as_object() else {
        return Ok(vec![ClipboardEntry { key: None, value }]);
    };

    let Some(version_value) = object.get(CLIPBOARD_MARKER) else {
        return Ok(vec![ClipboardEntry { key: None, value }]);
    };
    let version = version_value
        .as_u64()
        .ok_or_else(|| "Некорректная версия буфера структур".to_string())?;
    if version != CLIPBOARD_VERSION {
        return Err(format!(
            "Неподдерживаемая версия буфера структур: {}",
            version
        ));
    }

    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "Буфер структур не содержит список узлов".to_string())?;
    if nodes.is_empty() {
        return Err("Буфер структур не содержит узлов".to_string());
    }

    let mut entries = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let node = node
            .as_object()
            .ok_or_else(|| format!("Узел {} в буфере имеет неверный формат", index + 1))?;
        let key = match node.get("key") {
            None | Some(Value::Null) => None,
            Some(Value::String(key)) => Some(key.clone()),
            Some(_) => {
                return Err(format!(
                    "Ключ узла {} в буфере должен быть строкой или null",
                    index + 1
                ));
            }
        };
        let value = node
            .get("value")
            .cloned()
            .ok_or_else(|| format!("Узел {} в буфере не содержит значения", index + 1))?;
        entries.push(ClipboardEntry { key, value });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_clipboard_roundtrip_preserves_keys_and_values() {
        let entries = vec![
            ClipboardEntry {
                key: Some("profile".to_string()),
                value: serde_json::json!({"name": "Ada", "roles": ["admin"]}),
            },
            ClipboardEntry {
                key: Some("enabled".to_string()),
                value: Value::Bool(true),
            },
        ];

        let encoded = encode_structures(&entries).unwrap();
        assert_eq!(decode_structures(&encoded).unwrap(), entries);
    }

    #[test]
    fn plain_json_is_decoded_as_an_unkeyed_entry() {
        let entries = decode_structures(r#"{"name":"Ada"}"#).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, None);
        assert_eq!(entries[0].value, serde_json::json!({"name": "Ada"}));
    }

    #[test]
    fn invalid_structure_envelope_is_rejected() {
        let error = decode_structures(r#"{"$json_viewer_selection":1,"nodes":[{}]}"#).unwrap_err();

        assert!(error.contains("значения"));
    }
}
