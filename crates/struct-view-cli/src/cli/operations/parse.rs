use std::path::PathBuf;

mod finalize;

use struct_view_core::parser::{DataFormat, JsonValueType};
use struct_view_core::search::SearchOptions;

use super::super::{Command, Source};
use super::{ImageFormat, NodePart, Operation, OperationOptions};

pub(in crate::cli) fn parse(name: &str, args: &[String]) -> Result<OperationOptions, String> {
    let operation = match name {
        "new" => Operation::New,
        "get" => Operation::Get,
        "add" => Operation::Add,
        "set" => Operation::Set,
        "rename" => Operation::Rename,
        "delete" => Operation::Delete,
        "copy" => Operation::Copy,
        "paste" => Operation::Paste,
        "table" => Operation::Table,
        "schema" => Operation::Schema,
        "graph" => Operation::Graph,
        "convert" => Operation::Convert,
        _ => return Err(format!("Unknown command: {name}")),
    };
    let mut options = OperationOptions {
        operation,
        input: Source::Stdin,
        output: None,
        to: None,
        in_place: false,
        minify: false,
        paths: Vec::new(),
        key: None,
        kind: None,
        value: String::new(),
        from: None,
        clipboard: false,
        query: String::new(),
        search: SearchOptions::default(),
        image: None,
        dark: false,
        part: NodePart::Value,
        raw: false,
    };
    let mut positional = Vec::new();
    let mut search_flags = Vec::new();
    let mut value_seen = false;
    let mut literal = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if !literal && arg == "--" {
            literal = true;
        } else if literal || !arg.starts_with('-') || arg == "-" {
            positional.push(args[i].clone());
        } else {
            let allowed = match arg {
                "-o" | "--output" => true,
                "--to" | "-m" | "--minify" => matches!(
                    operation,
                    Operation::New
                        | Operation::Get
                        | Operation::Add
                        | Operation::Set
                        | Operation::Rename
                        | Operation::Delete
                        | Operation::Paste
                        | Operation::Convert
                ),
                "--in-place" => is_edit(operation),
                "--key" => matches!(operation, Operation::Add | Operation::Set),
                "--type" | "--value" => matches!(operation, Operation::Add | Operation::Set),
                "--path" => matches!(operation, Operation::Delete | Operation::Copy),
                "--from" => operation == Operation::Paste,
                "--clipboard" => matches!(operation, Operation::Copy | Operation::Paste),
                "--query" | "--keys" | "--values" | "--paths" | "--case-sensitive" | "--exact"
                | "--whole-word" | "--regex" => {
                    matches!(operation, Operation::Table | Operation::Schema)
                }
                "--image" | "--dark" => operation == Operation::Graph,
                "--part" | "--raw" => operation == Operation::Get,
                _ => false,
            };
            if !allowed {
                return Err(format!("Unknown option for {name}: {arg}"));
            }
            match arg {
                "--in-place" => options.in_place = true,
                "-m" | "--minify" => options.minify = true,
                "--clipboard" => options.clipboard = true,
                "--dark" => options.dark = true,
                "--raw" => options.raw = true,
                "--keys" | "--values" | "--paths" | "--case-sensitive" | "--exact"
                | "--whole-word" | "--regex" => search_flags.push(arg.to_string()),
                _ => {
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| format!("{arg} requires a value"))?;
                    match arg {
                        "-o" | "--output" => {
                            if value.starts_with('-') {
                                return Err(
                                    "--output requires a file path, not an option".to_string()
                                );
                            }
                            options.output = Some(PathBuf::from(value));
                        }
                        "--to" => options.to = Some(output_format(value)?),
                        "--key" => options.key = Some(value.clone()),
                        "--type" => options.kind = Some(value_type(value)?),
                        "--value" => {
                            options.value = value.clone();
                            value_seen = true;
                        }
                        "--path" => options.paths.push(value.clone()),
                        "--from" => {
                            if value.starts_with('-') && value != "-" {
                                return Err(
                                    "--from requires a file path or -, not an option".to_string()
                                );
                            }
                            options.from = Some(source(value));
                        }
                        "--query" => options.query = value.clone(),
                        "--part" => {
                            options.part = match value.as_str() {
                                "value" => NodePart::Value,
                                "key" => NodePart::Key,
                                "path" => NodePart::Path,
                                _ => return Err("--part must be value, key or path".to_string()),
                            };
                        }
                        "--image" => {
                            options.image = Some(match value.as_str() {
                                "svg" => ImageFormat::Svg,
                                "png" => ImageFormat::Png,
                                _ => return Err("--image must be svg or png".to_string()),
                            });
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        i += 1;
    }

    finalize::validate(
        name,
        operation,
        positional,
        search_flags,
        value_seen,
        options,
    )
}

fn source(value: &str) -> Source {
    if value == "-" {
        Source::Stdin
    } else {
        Source::File(PathBuf::from(value))
    }
}

fn output_format(value: &str) -> Result<DataFormat, String> {
    match value {
        "json" => Ok(DataFormat::Json),
        "yaml" | "yml" => Ok(DataFormat::Yaml),
        "toml" => Ok(DataFormat::Toml),
        "json5" => Ok(DataFormat::Json5),
        _ => Err(format!(
            "Unsupported output format: {value}; choose json, yaml, toml or json5"
        )),
    }
}

fn value_type(value: &str) -> Result<JsonValueType, String> {
    match value {
        "string" => Ok(JsonValueType::String),
        "number" => Ok(JsonValueType::Number),
        "float" => Ok(JsonValueType::Float),
        "boolean" | "bool" => Ok(JsonValueType::Bool),
        "null" => Ok(JsonValueType::Null),
        "object" => Ok(JsonValueType::Object),
        "array" => Ok(JsonValueType::Array),
        "datetime" => Ok(JsonValueType::DateTime),
        "comment" => Ok(JsonValueType::Comment),
        "metadata" => Ok(JsonValueType::Metadata),
        _ => Err(format!("Unknown value type: {value}")),
    }
}

pub(super) fn is_edit(operation: Operation) -> bool {
    matches!(
        operation,
        Operation::Add | Operation::Set | Operation::Rename | Operation::Delete | Operation::Paste
    )
}
