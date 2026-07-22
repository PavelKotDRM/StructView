//! # Модуль буфера обмена
//!
//! Предоставляет вспомогательные функции для копирования текста
//! в системный буфер обмена через библиотеку [`arboard`].

use arboard::Clipboard;

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
