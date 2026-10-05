use super::*;

pub(super) fn validate(
    name: &str,
    operation: Operation,
    positional: Vec<String>,
    mut search_flags: Vec<String>,
    value_seen: bool,
    mut options: OperationOptions,
) -> Result<OperationOptions, String> {
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
        } = super::super::super::args::parse_find(&search_flags)?
        else {
            unreachable!()
        };
        options.search = search;
    }
    Ok(options)
}
