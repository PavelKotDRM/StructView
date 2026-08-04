//! Выполнение headless-команд: format, validate, find.

use std::io::Write;
use std::path::Path;

use serde_json::Value;

use crate::parser::parse_json;
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
        Command::Find { query, input } => run_find(query, input),
        Command::Gui { .. } => panic!("Command::Gui не выполняется в headless-режиме"),
    }
}

/// Отформатировать JSON и записать результат в файл или stdout.
fn run_format(input: &Source, output: Option<&Path>, minify: bool) -> Result<bool, String> {
    let content = input.read()?;
    let value: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Ошибка JSON: {} (строка {}, позиция {})",
                e,
                e.line(),
                e.column()
            );
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
        None => write_lines(std::iter::once(formatted.as_str()))?,
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
        writeln!(lock, "{}", line).map_err(|e| format!("Ошибка вывода: {}", e))?;
    }
    Ok(())
}
