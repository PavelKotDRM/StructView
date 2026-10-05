use std::path::PathBuf;

use struct_view_core::parser::{DataFormat, JsonValueType};
use struct_view_core::search::SearchOptions;

use super::Source;

mod parse;
mod run;

pub(super) use parse::parse;
pub(super) use run::run;

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
