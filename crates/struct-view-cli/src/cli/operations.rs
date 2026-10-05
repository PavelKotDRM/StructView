use std::io::Write;
use std::path::PathBuf;

use struct_view_core::files::write_bytes_atomic;
use struct_view_core::parser::{DataFormat, JsonValueType, parse_data, serialize_node};
use struct_view_core::search::{SearchOptions, SearchState};
use struct_view_ui::app::headless;
use struct_view_ui::clipboard;

use super::{Command, Source};

/// Additional document and export operations available without a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    New,
    Get,
    Add,
    Set,
    Rename,
    Delete,
    Copy,
    Paste,
    Table,
    Schema,
    Graph,
    Convert,
}

/// Image format for the graph renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Svg,
    Png,
}

/// Node information to print with `get`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePart {
    Value,
    Key,
    Path,
}

/// Parsed options for a document operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationOptions {
    pub operation: Operation,
    pub input: Source,
    pub output: Option<PathBuf>,
    pub to: Option<DataFormat>,
    pub in_place: bool,
    pub minify: bool,
    pub paths: Vec<String>,
    pub key: Option<String>,
    pub kind: Option<JsonValueType>,
    pub value: String,
    pub from: Option<Source>,
    pub clipboard: bool,
    pub query: String,
    pub search: SearchOptions,
    pub image: Option<ImageFormat>,
    pub dark: bool,
    pub part: NodePart,
    pub raw: bool,
}

pub(super) fn parse(name: &str, args: &[String]) -> Result<OperationOptions, String> {
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

    let path_count = match operation {
        Operation::Get | Operation::Add | Operation::Set | Operation::Paste => 1,
        Operation::Rename => 2,
        _ => 0,
    };
    if positional.len() < path_count || positional.len() > path_count + 1 {
        return Err(format!("Invalid arguments for {name}; see --help"));
    }
    if path_count > 0 {
        options.paths.push(positional[0].clone());
    }
    if operation == Operation::Rename {
        options.key = Some(positional[1].clone());
    }
    if let Some(file) = positional.get(path_count) {
        if operation == Operation::New {
            if options.output.is_some() || file == "-" {
                return Err("new accepts one output file, or no file for stdout".to_string());
            }
            options.output = Some(PathBuf::from(file));
        } else {
            options.input = source(file);
        }
    }
    if matches!(operation, Operation::Delete | Operation::Copy) && options.paths.is_empty() {
        return Err(format!("{name} requires at least one --path"));
    }
    if matches!(operation, Operation::Add | Operation::Set) {
        let kind = options
            .kind
            .as_ref()
            .ok_or_else(|| format!("{name} requires --type"))?;
        if !value_seen
            && !matches!(
                kind,
                JsonValueType::Object | JsonValueType::Array | JsonValueType::Null
            )
        {
            return Err(format!("{name} requires --value for this type"));
        }
        if value_seen
            && matches!(
                kind,
                JsonValueType::Object | JsonValueType::Array | JsonValueType::Null
            )
        {
            return Err(
                "object, array and null do not accept --value; use paste for populated structures"
                    .to_string(),
            );
        }
    }
    if options.in_place && (options.output.is_some() || options.input == Source::Stdin) {
        return Err(
            "--in-place requires a file input and cannot be combined with --output".to_string(),
        );
    }
    if operation == Operation::Get
        && (options.raw || options.part != NodePart::Value)
        && (options.to.is_some() || options.minify)
    {
        return Err(
            "--raw and --part key/path cannot be combined with --to or --minify".to_string(),
        );
    }
    if operation == Operation::Copy && options.clipboard && options.output.is_some() {
        return Err("--clipboard cannot be combined with --output".to_string());
    }
    if operation == Operation::Paste {
        if options.clipboard && options.from.is_some() {
            return Err("--clipboard cannot be combined with --from".to_string());
        }
        if !options.clipboard {
            options.from.get_or_insert(Source::Stdin);
            if options.input == Source::Stdin && options.from == Some(Source::Stdin) {
                return Err(
                    "paste accepts at most one stdin source; specify a target file or --from"
                        .to_string(),
                );
            }
        }
    }
    if operation == Operation::Graph {
        let extension = options
            .output
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);
        let detected = match extension.as_deref() {
            Some("svg") => Some(ImageFormat::Svg),
            Some("png") => Some(ImageFormat::Png),
            _ => None,
        };
        if let Some(detected) = detected {
            if options.image.is_some_and(|image| image != detected) {
                return Err("--image conflicts with the output extension".to_string());
            }
            options.image = Some(detected);
        }
        if options.image.is_some() && options.output.is_none() {
            return Err(
                "Graph images require --output (binary output is not sent to stdout)".to_string(),
            );
        }
    }
    if options.dark && options.image.is_none() {
        return Err("--dark requires --image or an SVG/PNG output extension".to_string());
    }
    if !search_flags.is_empty() && options.query.is_empty() {
        return Err("Search options require --query".to_string());
    }
    if !options.query.is_empty() {
        search_flags.extend(["--".to_string(), options.query.clone()]);
        let Command::Find {
            options: search, ..
        } = super::args::parse_find(&search_flags)?
        else {
            unreachable!()
        };
        options.search = search;
    }
    Ok(options)
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

fn is_edit(operation: Operation) -> bool {
    matches!(
        operation,
        Operation::Add | Operation::Set | Operation::Rename | Operation::Delete | Operation::Paste
    )
}

pub(super) fn run(options: &OperationOptions) -> Result<bool, String> {
    let operation = options.operation;
    let (mut root, format) = if operation == Operation::New {
        parse_data("{}", Some(DataFormat::Json)).map_err(|error| error.to_string())?
    } else {
        parse_data(&options.input.read()?, options.input.format_hint())
            .map_err(|error| format!("Parse error: {error}"))?
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(name: &str, args: &[&str]) -> OperationOptions {
        parse(
            name,
            &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn clipboard_is_an_explicit_source_or_destination() {
        let copy = parsed("copy", &["source.json", "--path", "user", "--clipboard"]);
        assert!(copy.clipboard);
        let paste = parsed(
            "paste",
            &["$", "target.json", "--clipboard", "--output", "result.json"],
        );
        assert!(paste.clipboard);
        assert!(paste.from.is_none());
        assert_eq!(paste.output, Some(PathBuf::from("result.json")));
    }

    #[test]
    fn paste_accepts_stdin_for_either_source_but_not_both() {
        let target_stdin = parsed("paste", &["$", "--from", "selection.json"]);
        assert_eq!(target_stdin.input, Source::Stdin);
        assert_eq!(
            target_stdin.from,
            Some(Source::File(PathBuf::from("selection.json")))
        );
        let selection_stdin = parsed("paste", &["$", "target.json"]);
        assert_eq!(selection_stdin.from, Some(Source::Stdin));
        assert!(parse("paste", &["$".to_string()]).is_err());
    }

    #[test]
    fn terminator_and_negative_or_empty_values_are_not_options() {
        let get = parsed("get", &["--", "name", "-source.json"]);
        assert_eq!(get.input, Source::File(PathBuf::from("-source.json")));
        let negative = parsed("set", &["value", "--type", "number", "--value", "-12"]);
        assert_eq!(negative.value, "-12");
        let empty = parsed(
            "add",
            &["$", "--key", "empty", "--type", "string", "--value", ""],
        );
        assert!(empty.value.is_empty());
    }

    #[test]
    fn exports_share_find_scopes_and_match_options() {
        for name in ["table", "schema"] {
            let export = parsed(
                name,
                &[
                    "--query",
                    "id",
                    "--keys",
                    "--paths",
                    "--exact",
                    "--case-sensitive",
                ],
            );
            assert!(export.search.search_keys);
            assert!(export.search.search_paths);
            assert!(!export.search.search_values);
            assert!(export.search.exact_match);
            assert!(export.search.case_sensitive);
        }
    }

    #[test]
    fn image_extensions_are_case_insensitive_and_conflicts_are_rejected() {
        let graph = parsed("graph", &["--output", "graph.PNG", "--dark"]);
        assert_eq!(graph.image, Some(ImageFormat::Png));
        assert!(graph.dark);
        assert!(
            parse(
                "graph",
                &["--output", "graph.svg", "--image", "png"].map(String::from)
            )
            .is_err()
        );
    }
}
