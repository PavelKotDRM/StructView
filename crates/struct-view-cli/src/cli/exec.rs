//! Выполнение headless-команд: format, validate, find, diff.

use std::io::Write;
use std::path::Path;

use struct_view_core::diff::{Difference, compare_values, format_value};
use struct_view_core::files::write_text_atomic;
use struct_view_core::parser::{DataFormat, node_to_value, parse_data, serialize_node};
use struct_view_core::search::{SearchOptions, SearchState};

use super::args::Command;
use super::source::Source;

/// Выполнить headless-команду.
///
/// Возвращает `true`, если команда завершилась успешно, и `false` при
/// логической неудаче (невалидный JSON, отсутствие совпадений).
///
/// # Errors
///
/// Возвращает описание ошибки ввода-вывода, сериализации или вызова GUI-команды
/// из headless-режима.
pub fn run(command: &Command) -> Result<bool, String> {
    match command {
        Command::Operation(options) => super::operations::run(options),
        Command::Help => {
            write_text(super::HELP)?;
            Ok(true)
        }
        Command::Version => {
            write_text(&struct_view_build_info::detailed())?;
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
            Err("GUI commands cannot run in headless mode".to_string())
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
        let value = match node_to_value(&root) {
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
        write_lines(["Files are identical"])?;
        return Ok(true);
    }

    write_lines([format!("{} difference(s) found", differences.len()).as_str()])?;
    for difference in differences {
        let lines = difference_lines(&difference, inputs);
        write_lines(lines.iter().map(String::as_str))?;
    }
    Ok(false)
}

fn difference_lines(difference: &Difference, inputs: &[Source]) -> Vec<String> {
    let mut lines = vec![format!("{}:", difference.path)];
    for (input, value) in inputs.iter().zip(&difference.values) {
        lines.push(format!("  {}:", source_name(input)));
        let formatted_value = format_value(value.as_ref());
        lines.extend(formatted_value.lines().map(|line| format!("    {line}")));
    }
    lines
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
    input.check_output(output, input_format)?;
    let output_format = match output.and_then(DataFormat::from_path) {
        Some(format) if format.is_serializable() => format,
        Some(format) => {
            return Err(format!(
                "Cannot write {format}; choose JSON, YAML, TOML, or JSON5"
            ));
        }
        None if input_format.is_serializable() => input_format,
        None => DataFormat::Json,
    };

    let formatted = serialize_node(&root, output_format, minify)?;

    match output {
        Some(path) => write_text_atomic(path, &formatted)
            .map_err(|e| format!("Write error for {}: {}", path.display(), e))?,
        None => write_formatted(&formatted)?,
    }
    Ok(true)
}

/// Проверить синтаксис JSON.
fn run_validate(input: &Source) -> Result<bool, String> {
    let content = input.read()?;
    match parse_data(&content, input.format_hint()) {
        Ok((_, format)) => {
            write_lines([format!("{format} is valid").as_str()])?;
            Ok(true)
        }
        Err(error) => {
            eprintln!("Parse error: {}", error);
            Ok(false)
        }
    }
}

/// Найти узлы по заданным параметрам и вывести их пути.
fn run_find(query: &str, input: &Source, options: SearchOptions) -> Result<bool, String> {
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
    if let Some(error) = state.error {
        return Err(format!("Invalid regular expression: {}", error));
    }

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
    write_lines_to(&mut lock, lines)
}

fn write_lines_to<'a, W: Write, I: IntoIterator<Item = &'a str>>(
    writer: &mut W,
    lines: I,
) -> Result<(), String> {
    for line in lines {
        writeln!(writer, "{}", line).map_err(|e| format!("Output error: {}", e))?;
    }
    writer.flush().map_err(|e| format!("Output error: {e}"))?;
    Ok(())
}

fn write_text(text: &str) -> Result<(), String> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())
        .and_then(|_| lock.flush())
        .map_err(|error| format!("Output error: {error}"))
}

fn write_formatted(text: &str) -> Result<(), String> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    write_formatted_to(&mut lock, text)
}

fn write_formatted_to(writer: &mut impl Write, text: &str) -> Result<(), String> {
    writer
        .write_all(text.as_bytes())
        .and_then(|_| {
            if text.ends_with('\n') {
                Ok(())
            } else {
                writer.write_all(b"\n")
            }
        })
        .and_then(|_| writer.flush())
        .map_err(|error| format!("Output error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{difference_lines, run, run_format, write_formatted_to, write_lines_to};
    use crate::cli::Command;
    use crate::cli::Source;
    use serde_json::json;
    use std::path::PathBuf;
    use struct_view_core::diff::compare_values;
    use struct_view_core::parser::{DataFormat, node_to_value, parse_data};

    #[test]
    fn groups_pretty_values_under_each_changed_path_and_source() {
        let inputs = [
            Source::File(PathBuf::from("before.json")),
            Source::File(PathBuf::from("after.json")),
        ];
        let values = [
            json!({"settings": {"enabled": true}}),
            json!({"settings": false}),
        ];
        let differences = compare_values(&values);

        assert_eq!(
            difference_lines(&differences[0], &inputs),
            vec![
                "$.settings:".to_string(),
                "  before.json:".to_string(),
                "    {".to_string(),
                "      \"enabled\": true".to_string(),
                "    }".to_string(),
                "  after.json:".to_string(),
                "    false".to_string(),
            ]
        );
    }

    #[test]
    fn output_errors_are_returned_instead_of_panicking() {
        struct BrokenWriter;
        impl std::io::Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(
            write_lines_to(&mut BrokenWriter, ["line"])
                .unwrap_err()
                .starts_with("Output error:")
        );
        let mut output = Vec::new();
        write_lines_to(&mut output, ["first", "second"]).unwrap();
        assert_eq!(output, b"first\nsecond\n");
    }

    #[test]
    fn formatted_output_has_one_trailing_newline_for_all_formats() {
        use struct_view_core::parser::serialize_node;

        for (format, source) in [
            (DataFormat::Json, r#"{"value":1}"#),
            (DataFormat::Yaml, "value: 1\n"),
            (DataFormat::Toml, "value = 1\n"),
            (DataFormat::Json5, "{value: 1}"),
        ] {
            let root = parse_data(source, Some(format)).unwrap().0;
            let formatted = serialize_node(&root, format, false).unwrap();
            let mut output = Vec::new();
            write_formatted_to(&mut output, &formatted).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert_eq!(
                output.trim_end_matches('\n').len() + 1,
                output.len(),
                "{format}"
            );
        }
    }

    #[test]
    fn format_command_converts_dot_inputs_to_json_yaml_and_toml() {
        let input = std::env::temp_dir().join(format!(
            "struct_view-cli-dot-convert-{}.dot",
            std::process::id()
        ));
        std::fs::write(&input, "digraph { a [label=\"Alpha\"]; b; a -> b; }").unwrap();

        for (index, format) in [DataFormat::Json, DataFormat::Yaml, DataFormat::Toml]
            .into_iter()
            .enumerate()
        {
            let output = std::env::temp_dir().join(format!(
                "struct_view-cli-dot-convert-{}-{index}.{}",
                std::process::id(),
                format.extension()
            ));
            assert!(run_format(&Source::File(input.clone()), Some(&output), false).unwrap());
            let serialized = std::fs::read_to_string(&output).unwrap();
            let (root, detected_format) = parse_data(&serialized, Some(format)).unwrap();
            assert_eq!(detected_format, format);
            assert_eq!(
                node_to_value(&root).unwrap()["graph"]["type"],
                "directed_multigraph"
            );
            std::fs::remove_file(output).unwrap();
        }

        std::fs::remove_file(input).unwrap();
    }

    #[test]
    fn gui_commands_return_an_error_in_headless_mode() {
        assert!(
            run(&Command::Gui { file: None })
                .unwrap_err()
                .contains("headless")
        );
    }
}
