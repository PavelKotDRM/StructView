# StructView

A fast, cross-platform viewer and editor for structured data. The application
displays JSON, YAML, TOML, and JSON5 as an interactive tree and also provides
headless commands for formatting, validation, search, and comparison.

[Русская версия документации](docs/README.ru.md)

## Features

- JSON, YAML, TOML, and JSON5 support;
- automatic format detection for stdin and files with unknown extensions;
- interactive object and array tree with expandable and collapsible nodes;
- full-text search across keys and values;
- search filters for keys/values, case sensitivity, and exact matching;
- switchable tree, relationship graph, flattened table, and schema views;
- graph links inferred from common entity identifiers and reference fields;
- CSV export of table rows, respecting the active search filter;
- JSON Schema and OpenAPI schema views, including component and inline path
  schemas, with inferred sample structure for ordinary data documents;
- native TOML date/time editing and round-tripping;
- editing mode for field values and primitive values;
- creating a new empty JSON, YAML, TOML, or JSON5 file directly in edit mode;
- adding object fields and array elements;
- a field constructor with explicit string, number, boolean, null, object, and
  array types, plus TOML date/time values;
- saving to the original or another supported format, with explicit conversion
  to every other supported format;
- copying a node value, key, or path from the context menu;
- selecting and copying multiple structures while preserving their hierarchy;
- pasting copied structures into another open file;
- comparing two or more structured files and showing changed paths;
- side-by-side diff for any selected pair of compared files, with added,
  removed, and changed values highlighted;
- light and dark themes;
- Russian and English GUI localization;
- command-line operation without starting the GUI.

## Supported formats

| Format | Extensions |
| --- | --- |
| JSON | `.json` |
| YAML | `.yaml`, `.yml` |
| TOML | `.toml` |
| JSON5 | `.json5` |

For files, the format is detected from the extension first. If the extension
is unknown, the content is parsed using automatic format detection. For stdin,
the format is detected from the content.

## Installation and build

Building requires Rust with `edition 2024` support and Cargo.

```sh
git clone https://github.com/PavelKotDRM/structview.git
cd structview
cargo build --release
```

After the build, the executable is located in `target/release`:

- Windows: `target\release\struct_view.exe`;
- Linux and macOS: `target/release/struct_view`.

On Linux, graphical mode requires an available X11 or Wayland display server.
If no graphical server is available, use command-line mode.

Prebuilt archives for tagged versions are available on
[GitHub Releases](https://github.com/PavelKotDRM/structview/releases).

## Graphical interface

Starting the application without arguments opens an empty window:

```sh
struct_view
```

Open a file directly at startup:

```sh
struct_view data.json
```

Open several files directly in the comparison view:

```sh
struct_view first.json second.yaml third.toml
```

You can also choose a file through `File -> Open…` or drop it onto the window.
Use `File -> Compare files…` to select two or more files for comparison.
Use `File -> New file…`, choose a filename with a `.json`, `.yaml`, `.yml`,
`.toml`, or `.json5` extension, and start adding data to the empty object in
edit mode.

The interface provides:

- a `File` menu for creating, opening, saving, converting to another format,
  saving to a new file, and closing a document;
- a view selector for the interactive tree, relationship graph, flattened
  table, and schema diagram;
- a relationship graph for objects with `id`, `_id`, or `$id` identifiers and
  references such as `$ref`, `user_id`, and `depends_on`; ambiguous duplicate
  identifiers are not linked;
- a flattened path/value/type table that follows the search filter and can be
  exported as CSV;
- a schema diagram for JSON Schema and OpenAPI component/inline path schemas;
  other documents show an inferred schema, clearly marked as sample-derived
  rather than a contract;
- a comparison table for all selected files and a side-by-side diff for any
  selected pair, highlighting additions, removals, and changes;
- controls for expanding and collapsing the whole tree;
- search with previous and next match navigation;
- search options for key/value scope, case sensitivity, and exact matching;
- `View` and `Edit` modes;
- undo and redo for up to 100 document changes; inline edits are grouped into
  one action (`Ctrl+Z` / `Ctrl+Y`, or `Cmd+Z` / `Cmd+Shift+Z` on macOS);
- adding fields through a type-aware constructor; strings use plain text,
  numbers and booleans are validated, and objects/arrays start empty;
- adding document comments in JSON5, YAML, and TOML, plus tagged YAML values
  through the constructor;
- editing any node through `Edit field…` in its context menu, including
  changing its type and renaming object fields;
- node selection by clicking; hold `Ctrl`/`Cmd` while clicking to add nodes to
  the current selection;
- copying selected structures through `Edit -> Copy`, `Ctrl+C`, or the context
  menu;
- pasting into a selected object or array through `Edit -> Paste`, `Ctrl+V`, or
  `Paste here` in a container's context menu;
- horizontal mouse-wheel scrolling for the top toolbar when its controls do
  not fit the window width; the scrollbar does not cover toolbar controls;
- GUI language selection through `Settings -> Language`;
- node context menus for copying a value, key, or path;
- adding fields and elements in edit mode;
- light and dark theme switching.

### Large files and performance

The GUI keeps the parsed document in memory, but the tree uses row
virtualization: when a large subtree is expanded, egui creates widgets only
for rows inside the current viewport. Scrolling, selection, editing, search,
and context menus continue to work for the whole tree.

Virtualization reduces the cost of painting a fully expanded tree, but it does
not make memory usage constant. Opening a document still reads the complete
file and builds the complete in-memory tree, so memory usage is proportional to
the number of nodes. For especially large files, keep unrelated branches collapsed and use the CLI
`find`, `validate`, `format`, and `diff` commands when an interactive view is
not required.

When saving, the output format is selected from the destination file
extension. If the extension is unsupported, the format of the open document
is used.

To convert an open document, choose `File -> Convert to` and select `JSON`,
`YAML`, `TOML`, or `JSON5` (the current format is omitted). The conversion
dialog suggests a filename with the target format's extension and writes a
separate file, leaving the open document unchanged. `JSON5` output uses the
same JSON-compatible data as the other serializers; comments are retained only
in formats that support them, while trailing commas are normalized.

### Comparing files

The comparison view parses every selected file using the same format detection
as the regular viewer. Objects and arrays are compared recursively, so the
table lists the deepest changed paths. A missing path is displayed separately
from the JSON value `null`. Switch between the all-files comparison table and a
side-by-side diff; when more than two files are loaded, choose either version
from the diff selectors. The comparison is read-only; use `File -> Open…` or
`File -> Close file` to return to the regular document view.

### Format behavior and limitations

TOML date and time values retain their native type when saved back to TOML;
integer and floating-point values such as `1` and `1.0` remain distinct.
When converted to JSON or another JSON-compatible view, they are represented
as strings. TOML `nan` and infinity values can be saved as TOML, but cannot be converted to
JSON, YAML, or JSON5 through the shared JSON-compatible data model. TOML does
not support `null`.

YAML streams with multiple documents are shown as an array of documents.
Non-string YAML mapping keys are displayed using their compact JSON spelling;
if two keys would become the same string, parsing fails instead of silently
discarding one. YAML tags on values appear as `Metadata` nodes and are
preserved when saving as YAML; conversion to a format that cannot represent a
tag fails instead of dropping it. Tagged mapping keys and non-finite YAML
numbers remain unsupported. Comments in JSON5, YAML, and TOML appear as
separate nodes and are retained when saving to a format that supports them.
Comments are normalized to standalone lines at the start of the output;
original comment positions and styles, YAML anchors, and formatting are not
preserved. Conversion to strict JSON drops comments, and trailing commas are
normalized.

### Copying structures between files

1. Open the source file and select one or more tree nodes. Hold `Ctrl`
   (Windows/Linux) or `Cmd` (macOS) to select multiple nodes.
2. Press `Ctrl+C`/`Cmd+C` or use the copy command in the `Edit` menu.
3. Open the destination file, switch to `Edit` mode, and select the object or
   array that should receive the data.
4. Press `Ctrl+V`/`Cmd+V` or choose `Paste here` from the selected container's
   context menu.

Nested objects and arrays are copied as complete structures. When pasting into
an object, field names are preserved. When pasting into an array, elements are
appended with new indexes. If a keyless root object or array was copied, its
fields or elements are added to the target container. Pasting does not change
the file on disk until you save it.

### Localization

The GUI language can be switched without restarting the application through
`Settings -> Language`. The available languages are:

- `Русский` — the default language;
- `English`.

The translation covers menus, the toolbar, search, the status bar, tree
context menus, dialogs, and object/array item counts. Open document content,
JSON keys, and data values are not translated. The locale and message keys are
in [locale.rs](crates/struct-view-ui/src/app/i18n/locale.rs) and
[text_key.rs](crates/struct-view-ui/src/app/i18n/text_key.rs); the English and
Russian catalogs are maintained separately.

## Command-line interface

The release binary can be run directly. During development, use the same
commands through `cargo run --`.

CLI commands, options, help text, and runtime messages use English. Command and
option names are language-independent.

### Formatting

Print formatted data to stdout:

```sh
struct_view format data.json
```

Write the result to a file:

```sh
struct_view format data.json --output normalized.json
```

Compact output without indentation:

```sh
struct_view format data.json --minify
```

Read the source from stdin by using `-` or omitting the input file:

```sh
struct_view format - < data.json
```

The output format is determined from the extension of the file passed to
`--output`. If `--output` is not specified, the input format is preserved.

### Syntax validation

```sh
struct_view validate data.yaml
```

Validate data from stdin:

```sh
struct_view validate - < data.toml
```

On success, the command reports the detected format. It returns a non-zero
exit code when parsing fails.

### Path search

Search keys and values and print the paths of matching nodes:

```sh
struct_view find user data.json
```

Additional options:

```text
--keys            search keys;
--values          search values;
--case-sensitive  match letter case;
--exact           require an exact match.
```

Examples:

```sh
struct_view find --keys name data.json
struct_view find --values --case-sensitive ADMIN config.json
struct_view find --keys --exact id data.json
```

If no matches are found, the command exits with a non-zero code.

### Comparing files

Compare two or more files and print every changed path with the value found in
each input:

```sh
struct_view diff first.json second.json
struct_view diff base.yaml candidate.yaml generated.json
```

The command also accepts stdin as one input by using `-`:

```sh
struct_view diff reference.json - < candidate.json
```

Objects and arrays are compared recursively. The command prints `<missing>`
for a path that does not exist in a file, while JSON `null` is printed as
`null`. It returns exit code `0` when all inputs are identical and exit code
`1` when differences or a data error are found. `compare` is accepted as an
alias for `diff`.

### Common options

```sh
struct_view --help
struct_view --version
```

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | command completed successfully |
| `1` | data, I/O, no-match, or file-difference result |
| `2` | command-line argument parsing error |

## Development

Check formatting, run tests, and perform static analysis:

```sh
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

GitHub Actions can run the `Quality checks` workflow manually from the Actions
tab. Pushing a version tag that matches `Cargo.toml`, for example `v0.1.0`,
builds binaries for Linux, Windows, and macOS and publishes them as a GitHub
Release.

Cargo workspace crates:

```text
crates/
  struct-view-core/       parsing, document trees, search, and comparison
  struct-view-cli/        command-line parsing and headless commands
  struct-view-ui/         native GUI, editing, views, and clipboard
  struct-view-build-info/ shared compile-time build metadata
src/
  lib.rs                  compatibility facade over the workspace crates
  main.rs                 executable entry point
  console.rs              platform-specific console integration
```

The `struct-view-build-info` crate embeds the Rust version, target platform,
build time, and optimization mode. This information is available through
`--version` and the `Help` menu.

## License

This project is distributed under the [MIT License](LICENSE).
