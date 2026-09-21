# JSON Viewer

A fast, cross-platform viewer and editor for structured data. The application
displays JSON, YAML, TOML, and JSON5 as an interactive tree and also provides
headless commands for formatting, validation, and search.

[Русская версия документации](docs/README.ru.md)

## Features

- JSON, YAML, TOML, and JSON5 support;
- automatic format detection for stdin and files with unknown extensions;
- interactive object and array tree with expandable and collapsible nodes;
- full-text search across keys and values;
- search filters for keys/values, case sensitivity, and exact matching;
- editing mode for primitive values;
- adding object fields and array elements;
- saving to the original or another supported format;
- copying a node value, key, or path from the context menu;
- selecting and copying multiple structures while preserving their hierarchy;
- pasting copied structures into another open file;
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
git clone https://github.com/PavelKotDRM/json_viewer.git
cd json_viewer
cargo build --release
```

After the build, the executable is located in `target/release`:

- Windows: `target\release\json_viewer.exe`;
- Linux and macOS: `target/release/json_viewer`.

On Linux, graphical mode requires an available X11 or Wayland display server.
If no graphical server is available, use command-line mode.

## Graphical interface

Starting the application without arguments opens an empty window:

```sh
json_viewer
```

Open a file directly at startup:

```sh
json_viewer data.json
```

You can also choose a file through `File -> Open…` or drop it onto the window.

The interface provides:

- a `File` menu for opening, saving, saving to a new file, and closing a
  document;
- controls for expanding and collapsing the whole tree;
- search with previous and next match navigation;
- search options for key/value scope, case sensitivity, and exact matching;
- `View` and `Edit` modes;
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

When saving, the output format is selected from the destination file
extension. If the extension is unsupported, the format of the open document
is used.

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
JSON keys, and data values are not translated. The translation catalog is
located in [src/app/i18n.rs](src/app/i18n.rs).

## Command-line interface

The release binary can be run directly. During development, use the same
commands through `cargo run --`.

CLI commands, options, help text, and runtime messages use English. Command and
option names are language-independent.

### Formatting

Print formatted data to stdout:

```sh
json_viewer format data.json
```

Write the result to a file:

```sh
json_viewer format data.json --output normalized.json
```

Compact output without indentation:

```sh
json_viewer format data.json --minify
```

Read the source from stdin by using `-` or omitting the input file:

```sh
json_viewer format - < data.json
```

The output format is determined from the extension of the file passed to
`--output`. If `--output` is not specified, the input format is preserved.

### Syntax validation

```sh
json_viewer validate data.yaml
```

Validate data from stdin:

```sh
json_viewer validate - < data.toml
```

On success, the command reports the detected format. It returns a non-zero
exit code when parsing fails.

### Path search

Search keys and values and print the paths of matching nodes:

```sh
json_viewer find user data.json
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
json_viewer find --keys name data.json
json_viewer find --values --case-sensitive ADMIN config.json
json_viewer find --keys --exact id data.json
```

If no matches are found, the command exits with a non-zero code.

### Common options

```sh
json_viewer --help
json_viewer --version
```

Exit codes:

| Code | Meaning |
| --- | --- |
| `0` | command completed successfully |
| `1` | data, I/O, or no-match error |
| `2` | command-line argument parsing error |

## Development

Check formatting, run tests, and perform static analysis:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Main source tree:

```text
src/
  app/       application state, localization, and GUI rendering;
  cli/       CLI argument parsing and command execution;
  parser/    format parsing and tree construction;
  search.rs  node search;
  clipboard.rs
             system clipboard integration.
```

The `build.rs` script embeds build information such as the Rust version,
target platform, build time, and optimization mode. This information is
available through `--version` and the `Help` menu.

## License

This project is distributed under the [MIT License](LICENSE).
