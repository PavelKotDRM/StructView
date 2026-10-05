use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::time::Instant;

use struct_view_core::files::write_bytes_atomic;
use struct_view_core::parser::{DataFormat, JsonValueType, parse_data, serialize_node};
use struct_view_core::search::SearchState;
use struct_view_ui::app::headless;
use struct_view_ui::clipboard;

use super::super::Source;
use super::parse::is_edit;
use super::{ImageFormat, NodePart, Operation, OperationOptions};

mod progress;
use progress::{CliGraphProgress, cli_spinner};

pub(in crate::cli) fn run(options: &OperationOptions) -> Result<bool, String> {
    let operation = options.operation;
    let show_progress = std::io::stderr().is_terminal()
        && matches!(
            operation,
            Operation::Table | Operation::Schema | Operation::Graph
        );
    let input_started = Instant::now();
    let input_progress = show_progress.then(|| cli_spinner("Reading and parsing input"));
    let parsed = if operation == Operation::New {
        parse_data("{}", Some(DataFormat::Json)).map_err(|error| error.to_string())?
    } else {
        match parse_data(&options.input.read()?, options.input.format_hint()) {
            Ok(parsed) => parsed,
            Err(error) => {
                if let Some(progress) = input_progress {
                    progress.finish_with_message("Input parsing failed");
                }
                return Err(format!("Parse error: {error}"));
            }
        }
    };
    let (mut root, format) = parsed;
    if let Some(progress) = input_progress {
        progress.finish_with_message(format!(
            "Input ready ({:.2}s)",
            input_started.elapsed().as_secs_f64()
        ));
    }
    if is_edit(operation) && !format.is_serializable() {
        return Err(
            "Imported graph documents are read-only; convert them before editing".to_string(),
        );
    }
    options
        .input
        .check_output(options.output.as_deref(), format)?;
    if operation == Operation::New && options.output.as_ref().is_some_and(|path| path.exists()) {
        return Err("new refuses to overwrite an existing file".to_string());
    }
    let path = if matches!(
        operation,
        Operation::Get | Operation::Add | Operation::Set | Operation::Rename | Operation::Paste
    ) {
        options
            .paths
            .first()
            .map(String::as_str)
            .ok_or("Missing node path")?
    } else {
        "$"
    };
    let mut search = SearchState::default();
    search.search_with_options(&root, &options.query, options.search);
    if let Some(error) = &search.error {
        return Err(format!("Invalid regular expression: {error}"));
    }
    let operation_started = Instant::now();
    let operation_progress =
        if show_progress && !matches!(operation, Operation::Graph if options.image.is_some()) {
            Some(cli_spinner(match operation {
                Operation::Table => "Building table",
                Operation::Schema => "Building schema",
                Operation::Graph => "Building graph",
                _ => unreachable!(),
            }))
        } else {
            None
        };
    let bytes = match operation {
        Operation::Get if options.raw || options.part != NodePart::Value => {
            let node = headless::node_at_path(&root, path)?;
            match options.part {
                NodePart::Key => node
                    .key
                    .as_ref()
                    .ok_or("The root or this node has no key")?
                    .clone(),
                NodePart::Path => {
                    if node.path.is_empty() {
                        "$".to_string()
                    } else {
                        node.path.clone()
                    }
                }
                NodePart::Value => {
                    if node.value_type == JsonValueType::Comment {
                        node.display_value.clone()
                    } else {
                        struct_view_core::parser::node_to_value(node)?
                            .as_str()
                            .ok_or("--raw requires a string, datetime or comment node")?
                            .to_string()
                    }
                }
            }
            .into_bytes()
        }
        Operation::Table => headless::table(&root, &search).into_bytes(),
        Operation::Schema => serde_json::to_vec_pretty(&headless::schema(&root, &search)?)
            .map_err(|error| error.to_string())?,
        Operation::Graph => match options.image {
            Some(image) if std::io::stderr().is_terminal() => {
                let reporter = Arc::new(CliGraphProgress::default());
                headless::graph_image_with_progress(
                    &root,
                    image == ImageFormat::Png,
                    options.dark,
                    move |snapshot| reporter.report(snapshot),
                )?
            }
            Some(image) => headless::graph_image(&root, image == ImageFormat::Png, options.dark)?,
            None => serde_json::to_vec_pretty(&headless::graph(&root)?)
                .map_err(|error| error.to_string())?,
        },
        Operation::Copy => {
            let text = headless::copy(&root, &options.paths)?;
            if options.clipboard {
                clipboard::copy_to_clipboard(&text)?;
                return Ok(true);
            }
            text.into_bytes()
        }
        _ => {
            match operation {
                Operation::Add => headless::add(
                    &mut root,
                    path,
                    options.key.as_deref().unwrap_or(""),
                    options.kind.as_ref().ok_or("Missing value type")?,
                    &options.value,
                    format,
                )?,
                Operation::Set => headless::set(
                    &mut root,
                    path,
                    options.key.as_deref(),
                    options.kind.as_ref().ok_or("Missing value type")?,
                    &options.value,
                    format,
                )?,
                Operation::Rename => headless::rename(
                    &mut root,
                    path,
                    options.key.as_deref().ok_or("Missing name")?,
                )?,
                Operation::Delete => headless::delete(&mut root, &options.paths)?,
                Operation::Paste => {
                    let text = if options.clipboard {
                        clipboard::read_from_clipboard()?
                    } else {
                        options
                            .from
                            .as_ref()
                            .ok_or("Missing paste source")?
                            .read()?
                    };
                    headless::paste(&mut root, path, &text)?;
                }
                _ => {}
            }
            let output = if options.in_place {
                match &options.input {
                    Source::File(path) => Some(path),
                    Source::Stdin => return Err("Cannot edit stdin in place".to_string()),
                }
            } else {
                options.output.as_ref()
            };
            let detected = output.and_then(|path| DataFormat::from_path(path));
            if detected.is_some_and(|format| !format.is_serializable()) {
                return Err("Cannot write imported graph formats".to_string());
            }
            if options.to.is_some() && detected.is_some() && options.to != detected {
                return Err("--to conflicts with the output file extension".to_string());
            }
            let node = if operation == Operation::Get {
                headless::node_at_path(&root, path)?
            } else {
                &root
            };
            let target = options.to.or(detected).unwrap_or_else(|| {
                if !format.is_serializable()
                    || operation == Operation::Get
                        && format == DataFormat::Toml
                        && node.value_type != JsonValueType::Object
                {
                    DataFormat::Json
                } else {
                    format
                }
            });
            serialize_node(node, target, options.minify)?.into_bytes()
        }
    };
    if let Some(progress) = operation_progress {
        let name = match operation {
            Operation::Table => "Table",
            Operation::Schema => "Schema",
            Operation::Graph => "Graph",
            _ => unreachable!(),
        };
        progress.finish_with_message(format!(
            "{name} calculation completed ({:.2}s)",
            operation_started.elapsed().as_secs_f64()
        ));
    }
    let destination = if options.in_place {
        match &options.input {
            Source::File(path) => Some(path),
            Source::Stdin => None,
        }
    } else {
        options.output.as_ref()
    };
    if let Some(path) = destination {
        write_bytes_atomic(path, &bytes)
            .map_err(|error| format!("Write error for {}: {error}", path.display()))?;
    } else {
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&bytes)
            .and_then(|_| {
                if bytes.ends_with(b"\n") {
                    Ok(())
                } else {
                    stdout.write_all(b"\n")
                }
            })
            .and_then(|_| stdout.flush())
            .map_err(|error| format!("Output error: {error}"))?;
    }
    Ok(true)
}
