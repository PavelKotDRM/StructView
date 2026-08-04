//! Описание команд и разбор аргументов командной строки.

use std::path::PathBuf;

use super::source::Source;

/// Разобранная команда командной строки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Запустить графический интерфейс, опционально открыв файл.
    Gui {
        /// Файл, который нужно открыть при старте.
        file: Option<PathBuf>,
    },
    /// Отформатировать JSON.
    Format {
        /// Источник данных.
        input: Source,
        /// Файл для записи результата; `None` — вывод в stdout.
        output: Option<PathBuf>,
        /// Компактный вывод без отступов.
        minify: bool,
    },
    /// Проверить синтаксис JSON.
    Validate {
        /// Источник данных.
        input: Source,
    },
    /// Найти узлы, ключ или значение которых содержит подстроку.
    Find {
        /// Поисковый запрос (регистронезависимый).
        query: String,
        /// Источник данных.
        input: Source,
    },
    /// Вывести справку.
    Help,
    /// Вывести версию.
    Version,
}

/// Разобрать аргументы командной строки (без имени программы).
///
/// # Errors
///
/// Возвращает описание проблемы, если аргументы некорректны:
/// неизвестный флаг, отсутствующее значение опции или лишний позиционный аргумент.
///
/// # Examples
///
/// ```
/// use json_viewer::cli::{parse_args, Command};
///
/// let cmd = parse_args(["validate".to_string(), "a.json".to_string()]).unwrap();
/// assert!(matches!(cmd, Command::Validate { .. }));
/// ```
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let args: Vec<String> = args.into_iter().collect();

    let Some(first) = args.first() else {
        return Ok(Command::Gui { file: None });
    };

    match first.as_str() {
        "-h" | "--help" | "help" => return Ok(Command::Help),
        "-V" | "--version" | "version" => return Ok(Command::Version),
        "format" => return parse_format(&args[1..]),
        "validate" => return parse_validate(&args[1..]),
        "find" => return parse_find(&args[1..]),
        _ => {}
    }

    if first.starts_with('-') {
        return Err(format!("Неизвестная опция: {}", first));
    }
    if args.len() > 1 {
        return Err("GUI принимает не более одного файла".to_string());
    }
    Ok(Command::Gui {
        file: Some(PathBuf::from(first)),
    })
}

/// Разобрать аргументы подкоманды `format`.
fn parse_format(args: &[String]) -> Result<Command, String> {
    let mut input: Option<Source> = None;
    let mut output: Option<PathBuf> = None;
    let mut minify = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-m" | "--minify" => minify = true,
            "-o" | "--output" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "Опция --output требует путь к файлу".to_string())?;
                output = Some(PathBuf::from(value));
            }
            other => input = Some(take_positional(input, other, "format")?),
        }
        i += 1;
    }

    Ok(Command::Format {
        input: input.unwrap_or(Source::Stdin),
        output,
        minify,
    })
}

/// Разобрать аргументы подкоманды `validate`.
fn parse_validate(args: &[String]) -> Result<Command, String> {
    let mut input: Option<Source> = None;
    for arg in args {
        input = Some(take_positional(input, arg, "validate")?);
    }
    Ok(Command::Validate {
        input: input.unwrap_or(Source::Stdin),
    })
}

/// Разобрать аргументы подкоманды `find`.
fn parse_find(args: &[String]) -> Result<Command, String> {
    let mut query: Option<String> = None;
    let mut input: Option<Source> = None;

    for arg in args {
        if query.is_none() {
            query = Some(arg.clone());
        } else {
            input = Some(take_positional(input, arg, "find")?);
        }
    }

    let query = query.ok_or_else(|| "Подкоманда find требует поисковый запрос".to_string())?;
    Ok(Command::Find {
        query,
        input: input.unwrap_or(Source::Stdin),
    })
}

/// Принять позиционный аргумент-источник, отвергнув неизвестные флаги и дубликаты.
fn take_positional(current: Option<Source>, arg: &str, command: &str) -> Result<Source, String> {
    if current.is_some() {
        return Err(format!("Подкоманда {} принимает только один файл", command));
    }
    if arg == "-" {
        return Ok(Source::Stdin);
    }
    if arg.starts_with('-') {
        return Err(format!("Неизвестная опция для {}: {}", command, arg));
    }
    Ok(Source::File(PathBuf::from(arg)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_starts_gui() {
        assert_eq!(
            parse_args(Vec::<String>::new()).unwrap(),
            Command::Gui { file: None }
        );
    }

    #[test]
    fn positional_file_starts_gui() {
        let cmd = parse_args(["a.json".to_string()]).unwrap();
        assert_eq!(
            cmd,
            Command::Gui {
                file: Some(PathBuf::from("a.json"))
            }
        );
    }

    #[test]
    fn format_parses_options() {
        let cmd = parse_args(
            ["format", "a.json", "--minify", "-o", "b.json"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            cmd,
            Command::Format {
                input: Source::File(PathBuf::from("a.json")),
                output: Some(PathBuf::from("b.json")),
                minify: true,
            }
        );
    }

    #[test]
    fn dash_means_stdin() {
        let cmd = parse_args(["validate", "-"].map(String::from).to_vec()).unwrap();
        assert_eq!(
            cmd,
            Command::Validate {
                input: Source::Stdin
            }
        );
    }

    #[test]
    fn find_requires_query() {
        assert!(parse_args(["find".to_string()]).is_err());
    }

    #[test]
    fn unknown_option_is_error() {
        assert!(parse_args(["--nope".to_string()]).is_err());
    }
}
