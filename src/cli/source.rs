//! Источник JSON-данных для headless-команд.

use std::io::Read;
use std::path::PathBuf;

/// Источник JSON-данных для headless-команд.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Стандартный ввод (аргумент `-` или отсутствие пути).
    Stdin,
    /// Файл на диске.
    File(PathBuf),
}

impl Source {
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
                    .map_err(|e| format!("Ошибка чтения stdin: {}", e))?;
                Ok(buf)
            }
            Source::File(path) => std::fs::read_to_string(path)
                .map_err(|e| format!("Ошибка чтения файла {}: {}", path.display(), e)),
        }
    }
}
