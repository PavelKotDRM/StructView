//! # Модуль командной строки
//!
//! Разбирает аргументы командной строки и выполняет headless-команды
//! (без запуска GUI): форматирование, валидацию и поиск по JSON.
//!
//! ## Поддерживаемые вызовы
//!
//! ```text
//! json_viewer                      # открыть GUI
//! json_viewer data.json            # открыть GUI с файлом
//! json_viewer format data.json     # pretty-print в stdout
//! json_viewer validate data.json   # проверить синтаксис
//! json_viewer find name data.json  # вывести пути совпадений
//! ```

use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::Value;

use crate::parser::parse_json;
use crate::search::SearchState;

/// Текст справки, выводимый по `--help`.
pub const HELP: &str = concat!(
    "JSON Viewer ",
    env!("CARGO_PKG_VERSION"),
    " — просмотр и обработка JSON\n",
    "\n",
    "ИСПОЛЬЗОВАНИЕ:\n",
    "    json_viewer [ФАЙЛ]                     запустить GUI (опционально с файлом)\n",
    "    json_viewer format [ФАЙЛ] [ОПЦИИ]      форматировать JSON\n",
    "    json_viewer validate [ФАЙЛ]            проверить синтаксис JSON\n",
    "    json_viewer find <ЗАПРОС> [ФАЙЛ]       найти пути узлов по подстроке\n",
    "\n",
    "ОПЦИИ format:\n",
    "    -o, --output <ФАЙЛ>    записать результат в файл вместо stdout\n",
    "    -m, --minify           вывести компактный JSON без отступов\n",
    "\n",
    "ОБЩИЕ ОПЦИИ:\n",
    "    -h, --help             показать эту справку\n",
    "    -V, --version          показать версию\n",
    "\n",
    "Вместо ФАЙЛ можно указать `-`, чтобы читать JSON из stdin.\n",
    "\n",
    "КОДЫ ВОЗВРАТА:\n",
    "    0  успех\n",
    "    1  ошибка (некорректный JSON, нет совпадений, ошибка ввода-вывода)\n",
    "    2  ошибка разбора аргументов командной строки\n",
);

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

/// Выполнить headless-команду.
///
/// Возвращает `true`, если команда завершилась успешно, и `false` при
/// логической неудаче (невалидный JSON, отсутствие совпадений).
///
/// # Errors
///
/// Возвращает описание ошибки ввода-вывода или сериализации.
///
/// # Panics
///
/// Паникует, если передана [`Command::Gui`] — GUI запускается в `main`.
pub fn run(command: &Command) -> Result<bool, String> {
    match command {
        Command::Help => {
            print!("{}", HELP);
            Ok(true)
        }
        Command::Version => {
            println!("json_viewer {}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        Command::Format {
            input,
            output,
            minify,
        } => run_format(input, output.as_deref(), *minify),
        Command::Validate { input } => run_validate(input),
        Command::Find { query, input } => run_find(query, input),
        Command::Gui { .. } => panic!("Command::Gui не выполняется в headless-режиме"),
    }
}

/// Отформатировать JSON и записать результат в файл или stdout.
fn run_format(input: &Source, output: Option<&std::path::Path>, minify: bool) -> Result<bool, String> {
    let content = input.read()?;
    let value: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Ошибка JSON: {} (строка {}, позиция {})", e, e.line(), e.column());
            return Ok(false);
        }
    };

    let formatted = if minify {
        serde_json::to_string(&value)
    } else {
        serde_json::to_string_pretty(&value)
    }
    .map_err(|e| format!("Ошибка сериализации: {}", e))?;

    match output {
        Some(path) => std::fs::write(path, formatted)
            .map_err(|e| format!("Ошибка записи {}: {}", path.display(), e))?,
        None => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            writeln!(lock, "{}", formatted).map_err(|e| format!("Ошибка вывода: {}", e))?;
        }
    }
    Ok(true)
}

/// Проверить синтаксис JSON.
fn run_validate(input: &Source) -> Result<bool, String> {
    let content = input.read()?;
    match parse_json(&content) {
        Ok(_) => {
            println!("JSON корректен");
            Ok(true)
        }
        Err(e) => {
            eprintln!("Ошибка JSON: {}", e);
            Ok(false)
        }
    }
}

/// Найти узлы по подстроке и вывести их пути.
fn run_find(query: &str, input: &Source) -> Result<bool, String> {
    let content = input.read()?;
    let root = match parse_json(&content) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Ошибка JSON: {}", e);
            return Ok(false);
        }
    };

    let mut state = SearchState::default();
    state.search(&root, query);

    if state.matches.is_empty() {
        eprintln!("Совпадений не найдено");
        return Ok(false);
    }

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for path in &state.matches {
        // Корневой узел имеет пустой путь — показываем его как `$`.
        let shown = if path.is_empty() { "$" } else { path.as_str() };
        writeln!(lock, "{}", shown).map_err(|e| format!("Ошибка вывода: {}", e))?;
    }
    Ok(true)
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
