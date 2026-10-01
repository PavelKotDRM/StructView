//! Описание команд и разбор аргументов командной строки.

use std::path::PathBuf;

use struct_view_core::search::SearchOptions;

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
/// use struct_view_cli::cli::{parse_args, Command};
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
        "--" => return parse_gui_files(&args[1..], true),
        _ => {}
    }

    if first.starts_with('-') {
        return Err(format!("Unknown option: {}", first));
    }
    parse_gui_files(&args, false)
}

fn parse_gui_files(args: &[String], literal: bool) -> Result<Command, String> {
    if !literal && args.iter().any(|arg| arg.starts_with('-')) {
        return Err("GUI accepts file paths only".to_string());
    }
    match args {
        [] => Ok(Command::Gui { file: None }),
        [file] => Ok(Command::Gui {
            file: Some(PathBuf::from(file)),
        }),
        files => Ok(Command::GuiCompare {
            files: files.iter().map(PathBuf::from).collect(),
        }),
    }
}

/// Разобрать аргументы подкоманды `diff`.
fn parse_diff(args: &[String]) -> Result<Command, String> {
    let mut inputs = Vec::with_capacity(args.len());
    let mut stdin_seen = false;
    let mut literal = false;
    for arg in args {
        if arg == "--" && !literal {
            literal = true;
            continue;
        }
        if arg == "-" {
            if stdin_seen {
                return Err("The diff command accepts at most one stdin source".to_string());
            }
            stdin_seen = true;
            inputs.push(Source::Stdin);
        } else if !literal && arg.starts_with('-') {
            return Err(format!("Unknown option for diff: {}", arg));
        } else {
            inputs.push(Source::File(PathBuf::from(arg)));
        }
    }

    if inputs.len() < 2 {
        return Err("The diff command requires at least two files".to_string());
    }
    Ok(Command::Diff { inputs })
}

/// Разобрать аргументы подкоманды `format`.
fn parse_format(args: &[String]) -> Result<Command, String> {
    let mut input: Option<Source> = None;
    let mut output: Option<PathBuf> = None;
    let mut minify = false;
    let mut literal = false;

    let mut i = 0;
    while i < args.len() {
        if literal {
            input = Some(take_positional(input, &args[i], "format", true)?);
            i += 1;
            continue;
        }
        match args[i].as_str() {
            "--" => literal = true,
            "-m" | "--minify" => minify = true,
            "-o" | "--output" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--output requires a file path".to_string())?;
                if value.starts_with('-') {
                    return Err("--output requires a file path, not an option".to_string());
                }
                output = Some(PathBuf::from(value));
            }
            other => input = Some(take_positional(input, other, "format", false)?),
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
    let mut literal = false;
    for arg in args {
        if arg == "--" && !literal {
            literal = true;
        } else {
            input = Some(take_positional(input, arg, "validate", literal)?);
        }
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
    let mut literal = false;

    for arg in args {
        if literal {
            if query.is_none() {
                query = Some(arg.clone());
            } else {
                input = Some(take_positional(input, arg, "find", true)?);
            }
            continue;
        }
        match arg.as_str() {
            "--" => literal = true,
            "--keys" => {
                if !scope_selected {
                    options.search_keys = false;
                    options.search_values = false;
                    options.search_paths = false;
                    scope_selected = true;
                }
                options.search_keys = true;
            }
            "--values" => {
                if !scope_selected {
                    options.search_keys = false;
                    options.search_values = false;
                    options.search_paths = false;
                    scope_selected = true;
                }
                options.search_values = true;
            }
            "--paths" => {
                if !scope_selected {
                    options.search_keys = false;
                    options.search_values = false;
                    options.search_paths = false;
                    scope_selected = true;
                }
                options.search_paths = true;
            }
            "--case-sensitive" => options.case_sensitive = true,
            "--exact" => options.exact_match = true,
            "--whole-word" => options.whole_word = true,
            "--regex" => options.use_regex = true,
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("Unknown option for find: {arg}"));
            }
            _ if query.is_none() => query = Some(arg.clone()),
            _ => input = Some(take_positional(input, arg, "find", false)?),
        }
    }

    if options.use_regex && (options.exact_match || options.whole_word) {
        return Err("--regex cannot be combined with --exact or --whole-word".to_string());
    }
    if options.exact_match && options.whole_word {
        return Err("--exact and --whole-word cannot be used together".to_string());
    }

    let query = query.ok_or_else(|| "The find command requires a search query".to_string())?;
    Ok(Command::Find {
        query,
        input: input.unwrap_or(Source::Stdin),
        options,
    })
}

/// Принять позиционный аргумент-источник, отвергнув неизвестные флаги и дубликаты.
fn take_positional(
    current: Option<Source>,
    arg: &str,
    command: &str,
    literal: bool,
) -> Result<Source, String> {
    if current.is_some() {
        return Err(format!("The {} command accepts only one file", command));
    }
    if arg == "-" {
        return Ok(Source::Stdin);
    }
    if !literal && arg.starts_with('-') {
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
                    search_paths: false,
                    case_sensitive: true,
                    exact_match: true,
                    whole_word: false,
                    use_regex: false,
                },
            }
        );
    }

    #[test]
    fn find_parses_regex_and_rejects_incompatible_match_options() {
        let command = parse_args(
            ["find", "--paths", "--regex", r"^user_[0-9]+$", "data.json"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            command,
            Command::Find {
                query: r"^user_[0-9]+$".to_string(),
                input: Source::File(PathBuf::from("data.json")),
                options: SearchOptions {
                    search_keys: false,
                    search_values: false,
                    search_paths: true,
                    use_regex: true,
                    ..Default::default()
                },
            }
        );

        assert!(
            parse_args(
                ["find", "--regex", "--exact", "user", "data.json"]
                    .map(String::from)
                    .to_vec()
            )
            .is_err()
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

    #[test]
    fn unknown_find_option_is_not_used_as_a_query() {
        assert!(parse_args(["find", "--typo", "data.json"].map(String::from)).is_err());
    }

    #[test]
    fn option_terminator_allows_queries_and_paths_starting_with_a_dash() {
        assert_eq!(
            parse_args(["find", "--keys", "--", "--literal", "-data.json"].map(String::from))
                .unwrap(),
            Command::Find {
                query: "--literal".to_string(),
                input: Source::File(PathBuf::from("-data.json")),
                options: SearchOptions {
                    search_keys: true,
                    search_values: false,
                    search_paths: false,
                    ..Default::default()
                },
            }
        );
        assert_eq!(
            parse_args(["--", "-data.json"].map(String::from)).unwrap(),
            Command::Gui {
                file: Some(PathBuf::from("-data.json"))
            }
        );
    }

    #[test]
    fn an_output_option_requires_a_path_not_another_option() {
        assert!(parse_args(["format", "--output", "--minify"].map(String::from)).is_err());
    }
}
