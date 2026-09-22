//! Описание команд и разбор аргументов командной строки.

use std::path::PathBuf;

use crate::search::SearchOptions;

use super::source::Source;

/// Разобранная команда командной строки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Запустить графический интерфейс, опционально открыв файл.
    Gui {
        /// Файл, который нужно открыть при старте.
        file: Option<PathBuf>,
    },
    /// Запустить графический интерфейс и сравнить несколько файлов.
    GuiCompare {
        /// Файлы, которые нужно загрузить в режим сравнения.
        files: Vec<PathBuf>,
    },
    /// Отформатировать структурированные данные.
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
    /// Найти узлы по запросу и параметрам поиска.
    Find {
        /// Поисковый запрос.
        query: String,
        /// Источник данных.
        input: Source,
        /// Параметры поиска.
        options: SearchOptions,
    },
    /// Сравнить два или более источника данных.
    Diff {
        /// Источники данных в порядке отображения результата.
        inputs: Vec<Source>,
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
        "diff" | "compare" => return parse_diff(&args[1..]),
        _ => {}
    }

    if first.starts_with('-') {
        return Err(format!("Unknown option: {}", first));
    }
    if args.len() > 1 {
        if args.iter().any(|arg| arg.starts_with('-')) {
            return Err("GUI accepts file paths only".to_string());
        }
        return Ok(Command::GuiCompare {
            files: args.into_iter().map(PathBuf::from).collect(),
        });
    }
    Ok(Command::Gui {
        file: Some(PathBuf::from(first)),
    })
}

/// Разобрать аргументы подкоманды `diff`.
fn parse_diff(args: &[String]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("The diff command requires at least two files".to_string());
    }

    let mut inputs = Vec::with_capacity(args.len());
    let mut stdin_seen = false;
    for arg in args {
        if arg == "-" {
            if stdin_seen {
                return Err("The diff command accepts at most one stdin source".to_string());
            }
            stdin_seen = true;
            inputs.push(Source::Stdin);
        } else if arg.starts_with('-') {
            return Err(format!("Unknown option for diff: {}", arg));
        } else {
            inputs.push(Source::File(PathBuf::from(arg)));
        }
    }

    Ok(Command::Diff { inputs })
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
                    .ok_or_else(|| "--output requires a file path".to_string())?;
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
    let mut options = SearchOptions::default();
    let mut scope_selected = false;

    for arg in args {
        match arg.as_str() {
            "--keys" => {
                if !scope_selected {
                    options.search_keys = false;
                    options.search_values = false;
                    scope_selected = true;
                }
                options.search_keys = true;
            }
            "--values" => {
                if !scope_selected {
                    options.search_keys = false;
                    options.search_values = false;
                    scope_selected = true;
                }
                options.search_values = true;
            }
            "--case-sensitive" => options.case_sensitive = true,
            "--exact" => options.exact_match = true,
            _ if query.is_none() => query = Some(arg.clone()),
            _ => input = Some(take_positional(input, arg, "find")?),
        }
    }

    let query = query.ok_or_else(|| "The find command requires a search query".to_string())?;
    Ok(Command::Find {
        query,
        input: input.unwrap_or(Source::Stdin),
        options,
    })
}

/// Принять позиционный аргумент-источник, отвергнув неизвестные флаги и дубликаты.
fn take_positional(current: Option<Source>, arg: &str, command: &str) -> Result<Source, String> {
    if current.is_some() {
        return Err(format!("The {} command accepts only one file", command));
    }
    if arg == "-" {
        return Ok(Source::Stdin);
    }
    if arg.starts_with('-') {
        return Err(format!("Unknown option for {}: {}", command, arg));
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
    fn multiple_positional_files_start_gui_comparison() {
        let cmd = parse_args(["a.json".to_string(), "b.yaml".to_string()]).unwrap();
        assert_eq!(
            cmd,
            Command::GuiCompare {
                files: [PathBuf::from("a.json"), PathBuf::from("b.yaml")].to_vec(),
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
    fn find_parses_search_options() {
        let command = parse_args(
            [
                "find",
                "--keys",
                "--case-sensitive",
                "--exact",
                "Name",
                "a.json",
            ]
            .map(String::from)
            .to_vec(),
        )
        .unwrap();
        assert_eq!(
            command,
            Command::Find {
                query: "Name".to_string(),
                input: Source::File(PathBuf::from("a.json")),
                options: SearchOptions {
                    search_keys: true,
                    search_values: false,
                    case_sensitive: true,
                    exact_match: true,
                },
            }
        );
    }

    #[test]
    fn diff_requires_at_least_two_files() {
        assert!(parse_args(["diff", "a.json"].map(String::from).to_vec()).is_err());
    }

    #[test]
    fn diff_parses_multiple_sources() {
        let command =
            parse_args(["diff", "a.json", "-", "c.yaml"].map(String::from).to_vec()).unwrap();
        assert_eq!(
            command,
            Command::Diff {
                inputs: vec![
                    Source::File(PathBuf::from("a.json")),
                    Source::Stdin,
                    Source::File(PathBuf::from("c.yaml")),
                ],
            }
        );
    }

    #[test]
    fn unknown_option_is_error() {
        assert!(parse_args(["--nope".to_string()]).is_err());
    }
}
