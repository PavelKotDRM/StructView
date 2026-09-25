//! Источник структурированных данных для headless-команд.

use std::io::Read;
use std::path::PathBuf;

use struct_view_core::parser::DataFormat;

/// Источник JSON-данных для headless-команд.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Стандартный ввод (аргумент `-` или отсутствие пути).
    Stdin,
    /// Файл на диске.
    File(PathBuf),
}

impl Source {
    /// Получить подсказку формата из расширения файла.
    pub fn format_hint(&self) -> Option<DataFormat> {
        match self {
            Source::Stdin => None,
            Source::File(path) => DataFormat::from_path(path),
        }
    }

    /// Прочитать содержимое источника в строку.
    ///
    /// # Errors
    ///
    /// Возвращает описание ошибки ввода-вывода, если файл недоступен
    /// или stdin содержит не-UTF-8 данные.
    pub fn read(&self) -> Result<String, String> {
        match self {
            Source::Stdin => {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| format!("Error reading stdin: {}", e))?;
                Ok(buf)
            }
            Source::File(path) => std::fs::read_to_string(path)
                .map_err(|e| format!("Error reading file {}: {}", path.display(), e)),
        }
    }
}
