//! Выполнение headless-команд: format, validate, find, diff.

use std::io::Write;
use std::path::Path;

use crate::diff::{compare_values, format_value};
use crate::parser::{DataFormat, parse_data, serialize_data};
use crate::search::SearchState;

use super::args::Command;
use super::source::Source;

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
            print!("{}", super::HELP);
            Ok(true)
        }
        Command::Version => {
            print!("{}", crate::build_info::detailed());
            Ok(true)
        }
        Command::Format {
            input,
            output,
            minify,
        } => run_format(input, output.as_deref(), *minify),
        Command::Validate { input } => run_validate(input),
        Command::Find {
            query,
            input,
            options,
        } => run_find(query, input, *options),
        Command::Diff { inputs } => run_diff(inputs),
        Command::Gui { .. } | Command::GuiCompare { .. } => {
            panic!("GUI commands cannot run in headless mode")
        }
    }
}

/// Сравнить несколько источников и вывести отличающиеся пути.
fn run_diff(inputs: &[Source]) -> Result<bool, String> {
    if inputs.len() < 2 {
        return Err("The diff command requires at least two files".to_string());
    }

    let mut values = Vec::with_capacity(inputs.len());
    for input in inputs {
        let content = input.read()?;
        let (root, _) = match parse_data(&content, input.format_hint()) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("Parse error in {}: {}", source_name(input), error);
                return Ok(false);
            }
        };
        let value = match crate::app::node_to_value(&root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Conversion error in {}: {}", source_name(input), error);
                return Ok(false);
            }
        };
        values.push(value);
    }

    let differences = compare_values(&values);
    if differences.is_empty() {
        println!("Files are identical");
        return Ok(true);
    }

    println!("{} difference(s) found", differences.len());
    for difference in differences {
        println!("{}:", difference.path);
        for (input, value) in inputs.iter().zip(difference.values.iter()) {
            println!("  {}: {}", source_name(input), format_value(value.as_ref()));
        }
    }
    Ok(false)
}

fn source_name(source: &Source) -> String {
    match source {
        Source::Stdin => "<stdin>".to_string(),
        Source::File(path) => path.display().to_string(),
    }
}

/// Отформатировать JSON и записать результат в файл или stdout.
fn run_format(input: &Source, output: Option<&Path>, minify: bool) -> Result<bool, String> {
    let content = input.read()?;
    let (root, input_format) = match parse_data(&content, input.format_hint()) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("Parse error: {}", error);
            return Ok(false);
        }
    };
    let value = crate::app::node_to_value(&root)?;
    let output_format = output
        .and_then(DataFormat::from_path)
        .unwrap_or(input_format);

    let formatted = serialize_data(&value, output_format, minify)?;

    match output {
        Some(path) => std::fs::write(path, formatted)
            .map_err(|e| format!("Write error for {}: {}", path.display(), e))?,
        None => write_lines(std::iter::once(formatted.as_str()))?,
    }
    Ok(true)
}

/// Проверить синтаксис JSON.
fn run_validate(input: &Source) -> Result<bool, String> {
    let content = input.read()?;
    match parse_data(&content, input.format_hint()) {
        Ok((_, format)) => {
            println!("{} is valid", format);
            Ok(true)
        }
        Err(error) => {
            eprintln!("Parse error: {}", error);
            Ok(false)
        }
    }
}

/// Найти узлы по подстроке и вывести их пути.
fn run_find(
    query: &str,
    input: &Source,
    options: crate::search::SearchOptions,
) -> Result<bool, String> {
    let content = input.read()?;
    let root = match parse_data(&content, input.format_hint()) {
        Ok((node, _)) => node,
        Err(error) => {
            eprintln!("Parse error: {}", error);
            return Ok(false);
        }
    };

    let mut state = SearchState::default();
    state.search_with_options(&root, query, options);

    if state.matches.is_empty() {
        eprintln!("No matches found");
        return Ok(false);
    }

    // Корневой узел имеет пустой путь — показываем его как `$`.
    write_lines(state.matches.iter().map(
        |path| {
            if path.is_empty() { "$" } else { path.as_str() }
        },
    ))?;
    Ok(true)
}

/// Записать строки в stdout под одной блокировкой.
fn write_lines<'a, I: IntoIterator<Item = &'a str>>(lines: I) -> Result<(), String> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for line in lines {
        writeln!(lock, "{}", line).map_err(|e| format!("Output error: {}", e))?;
    }
    Ok(())
}
