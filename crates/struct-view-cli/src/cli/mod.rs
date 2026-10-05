//! # Модуль командной строки
//!
//! Разбирает аргументы командной строки и выполняет headless-команды
//! (без запуска GUI): форматирование, валидацию, поиск и сравнение данных.
//!
//! ## Поддерживаемые вызовы
//!
//! ```text
//! struct_view                      # открыть GUI
//! struct_view data.json            # открыть GUI с файлом
//! struct_view format data.json     # pretty-print в stdout
//! struct_view validate data.json   # проверить синтаксис
//! struct_view find name data.json  # вывести пути совпадений
//! struct_view diff one.json two.json # сравнить файлы
//! ```
//!
//! ## Состав подмодулей
//!
//! | Подмодуль | Назначение |
//! |-----------|-----------|
//! | `args` | Тип [`Command`] и разбор аргументов ([`parse_args`]) |
//! | `source` | Источник данных [`Source`]: файл или stdin |
//! | `exec` | Выполнение разобранной команды ([`run`]) |

mod args;
mod exec;
mod operations;
mod source;

pub use args::{Command, parse_args};
pub use exec::run;
pub use operations::{ImageFormat, NodePart, Operation, OperationOptions};
pub use source::Source;

/// Текст справки, выводимый по `--help`.
pub const HELP: &str = concat!(
    "StructView ",
    env!("CARGO_PKG_VERSION"),
    " — view JSON, YAML, TOML, JSON5, Graphviz DOT, GraphML, and GEXF\n",
    "\n",
    "USAGE:\n",
    "    struct_view [FILE]                      start the GUI (optionally with a file)\n",
    "    struct_view FILE...                       start the GUI and compare files\n",
    "    struct_view format [FILE] [OPTIONS]     format data\n",
    "    struct_view validate [FILE]             validate syntax\n",
    "    struct_view find [OPTIONS] <QUERY> [FILE] find matching node paths\n",
    "    struct_view diff <FILE> <FILE> [FILE...] compare files\n",
    "    struct_view convert [FILE] --to FORMAT convert data (also supports --output)\n",
    "    struct_view new [FILE] [--to FORMAT]    create an empty object\n",
    "    struct_view get PATH [FILE]            print a node value, key or path\n",
    "    struct_view add PARENT [FILE] --type TYPE [--key KEY] [--value VALUE]\n",
    "    struct_view set PATH [FILE] --type TYPE [--key KEY] [--value VALUE]\n",
    "    struct_view rename PATH NAME [FILE]    rename an object field\n",
    "    struct_view delete [FILE] --path PATH [--path PATH...]\n",
    "    struct_view copy [FILE] --path PATH [--path PATH...]\n",
    "    struct_view paste PARENT [FILE] [--from FILE | --clipboard]\n",
    "    struct_view table [FILE]               export flattened rows as CSV\n",
    "    struct_view schema [FILE]              export schema diagram rows as JSON\n",
    "    struct_view graph [FILE]               export entities/links as JSON or SVG/PNG\n",
    "\n",
    "format OPTIONS:\n",
    "    -o, --output <FILE>    write the result to a file instead of stdout\n",
    "    -m, --minify           print a compact representation\n",
    "\n",
    "find OPTIONS:\n",
    "    --keys                 search keys only\n",
    "    --values               search values only\n",
    "    --paths                search JSON paths only\n",
    "    --case-sensitive       match letter case\n",
    "    --exact                require an exact match\n",
    "    --whole-word           require a whole-word match\n",
    "    --regex                interpret the query as a regular expression (incompatible with --exact and --whole-word)\n",
    "    use `key: value` in a query to match both fields on one node\n",
    "\n",
    "diff:\n",
    "    compare two or more files; use `-` for one stdin input\n",
    "\n",
    "DOCUMENT AND EXPORT OPTIONS:\n",
    "    -o, --output FILE      atomic file output; otherwise stdout\n",
    "    --to FORMAT           json, yaml, toml or json5 (new/get/edits/convert)\n",
    "    -m, --minify           compact document output (new/get/edits/convert)\n",
    "    --in-place            save edits to the input file; incompatible with --output\n",
    "    --type TYPE           string, number, float, boolean, null, object, array,\n",
    "                          datetime (TOML), comment (JSON5/YAML/TOML), metadata (YAML)\n",
    "    --value VALUE         plain text for strings; typed literal otherwise\n",
    "                          object/array/null take no value; containers retain existing children\n",
    "    --key KEY             new object field name (add) or optional rename (set)\n",
    "    --part value|key|path  get: choose what to print (default: value)\n",
    "    --raw                 get: print a string/datetime/comment without quoting\n",
    "                          extracted TOML non-object values default to JSON\n",
    "    --clipboard           copy/paste: use the system clipboard instead of stdout/stdin\n",
    "    --from FILE           paste: JSON or copy envelope source (default: stdin)\n",
    "    --query QUERY         table/schema filter; accepts the same search options as find\n",
    "    --image svg|png        graph image export; requires --output (extension also detects type)\n",
    "    --dark                graph: opaque dark image; default is light/transparent\n",
    "    graph JSON retains per-edge direction (directed/undirected/bidirectional/reverse),\n",
    "    mixed direction summaries and self-loops. DOT dir and GEXF mutual are honored.\n",
    "    Paths accept find output or $-prefixed table paths; $ denotes the root.\n",
    "    copy preserves keys and hierarchy in a GUI-compatible JSON envelope.\n",
    "    paste appends arrays, merges objects and rejects duplicate keys; one stdin source only.\n",
    "    new refuses existing files. Imported graph files cannot be edited or overwritten.\n",
    "\n",
    "COMMON OPTIONS:\n",
    "    -h, --help             show this help\n",
    "    -V, --version          show the version and build information\n",
    "\n",
    "The file format is detected from the extension; stdin is detected from its content.\n",
    "Supported extensions: .json, .yaml, .yml, .toml, .json5, .dot, .gv, .graphml, .gexf, and .xml (auto-detected).\n",
    "Graphviz DOT, GraphML, and GEXF inputs are read-only; format them to JSON, YAML, or TOML to convert.\n",
    "Use `-` instead of FILE to read data from stdin.\n",
    "Use `--` to end options before a query or file path starting with a dash.\n",
    "\n",
    "EXIT CODES:\n",
    "    0  success\n",
    "    1  data, no-match, difference, or I/O error\n",
    "    2  command-line argument parsing error\n",
);

#[cfg(test)]
mod tests {
    use super::HELP;

    #[test]
    fn help_uses_english_cli_labels() {
        assert!(HELP.contains("USAGE:"));
        assert!(HELP.contains("COMMON OPTIONS:"));
        assert!(HELP.contains("EXIT CODES:"));
        assert!(!HELP.contains("ИСПОЛЬЗОВАНИЕ"));
    }
}
