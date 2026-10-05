//! Источник структурированных данных для headless-команд.

use std::io::Read;
use std::path::PathBuf;

use struct_view_core::parser::DataFormat;

/// Источник структурированных данных для headless-команд.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Стандартный ввод (аргумент `-` или отсутствие пути).
    Stdin,
    /// Файл на диске.
    File(PathBuf),
}

impl Source {
    /// Prevent exports from replacing a read-only imported graph source.
    pub(super) fn check_output(
        &self,
        output: Option<&std::path::Path>,
        format: DataFormat,
    ) -> Result<(), String> {
        if format.is_serializable() {
            return Ok(());
        }
        if let (Self::File(input), Some(output)) = (self, output)
            && output.exists()
        {
            let resolve = |path: &std::path::Path| {
                path.canonicalize()
                    .map_err(|error| format!("Cannot resolve {}: {error}", path.display()))
            };
            if resolve(input)? == resolve(output)? {
                return Err(
                    "Imported graph files are read-only and cannot be overwritten".to_string(),
                );
            }
        }
        Ok(())
    }

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
