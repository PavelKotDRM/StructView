# StructView Tutorial

[Русская версия](ru.md) | [Project overview](../../README.md)

This walkthrough shows how to open, explore, edit, compare, and process
structured-data files with StructView. Menu names below use the English UI;
select it in `Settings -> Language` (`Настройки -> Язык`) if needed.

## 1. Install and open a document

StructView reads JSON (`.json`), YAML (`.yaml`, `.yml`), TOML (`.toml`), and
JSON5 (`.json5`). For files, it uses the extension when recognized and tries
automatic format detection for unknown extensions. Data read from standard
input is detected from its contents.

Build the project with Rust that supports Edition 2024 and Cargo:

```sh
cargo build --release
```

The executable is `target\release\struct_view.exe` on Windows and
`target/release/struct_view` on Linux and macOS.

Create a file named `data.json` with this sample:

```json
{
  "service": "checkout",
  "enabled": true,
  "retries": 3,
  "users": [
    { "id": "u-1", "name": "Alice", "role": "admin" },
    { "id": "u-2", "name": "Bob", "role": "developer" }
  ],
  "tasks": [
    { "id": "task-1", "user_id": "u-1", "title": "Review release" },
    { "id": "task-2", "user_id": "u-2", "title": "Update tests" }
  ]
}
```

Open it from a terminal. If `struct_view` is not on your `PATH`, run the
executable from the project root:

```sh
# When struct_view is on PATH
struct_view data.json

# Windows
.\target\release\struct_view.exe data.json

# Linux or macOS
./target/release/struct_view data.json

# During development
cargo run -- data.json
```

You can also open a file with `File -> Open…` or drag it into the application
window. Starting `struct_view` without a file opens an empty window.

## 2. Explore the document

The tree view displays objects and arrays as expandable nodes. Expand a node to
inspect its children, and use the tree controls to expand or collapse the
document. Select a node to work with it or open its context menu.

Use the view selector to switch between:

- **Tree** — browse the nested document.
- **Graph** — inspect links inferred from common identifiers such as `id`,
  `_id`, and `$id`, and reference fields such as `$ref`, `user_id`, and
  `depends_on`. These links are inferred; duplicate identifiers that make a
  link ambiguous are not connected.
- **Table** — see flattened paths, values, and types. The table follows the
  active search filter and can be exported as CSV.
- **Schema** — inspect JSON Schema or OpenAPI component and inline path
  schemas. For an ordinary data document, StructView infers a schema from the
  sample; that inferred view is not a formal contract.

## 3. Find a key, value, or path

Open the search window from the toolbar or press `Ctrl+F` (`Cmd+F` on macOS).
Enter a query, choose whether to search keys, values, or paths, and refine it
with case-sensitive, exact, whole-word, or regular-expression matching. Use
the previous/next controls to move through matches, or select a result path to
locate it in the document.

For the sample above, search for `Alice` to find a value, or search keys for
the exact name `id`. The regex builder can insert escaped literal text and
common pattern fragments. Regular expressions use Rust regex syntax.

## 4. Edit and save

Switch to **Edit** mode to change the document. Open a node's context menu and
choose `Edit field…` to edit its value or type; object fields can also be
renamed. Add object fields and array elements through the type-aware
constructor. Strings are entered as plain text, numbers and booleans are
validated, and new objects and arrays start empty. TOML does not support
`null`, so that value is unavailable when editing TOML.

To start a document from scratch, choose `File -> New file…`, give it a
supported extension, and add fields to the new empty object. Undo and redo are
available for up to 100 document changes (`Ctrl+Z` / `Ctrl+Y`; on macOS,
`Cmd+Z` / `Cmd+Shift+Z`).

Save your changes with the file commands. When saving, StructView chooses the
output format from the destination extension; if the extension is
unsupported, it uses the open document's format. To create an explicit
conversion, choose `File -> Convert to` and select the target format. This
writes a separate file and leaves the open document unchanged.

You can copy one or more selected structures with `Edit -> Copy` or
`Ctrl+C`/`Cmd+C`, then paste them into a selected object or array in another
open document with `Edit -> Paste` or `Ctrl+V`/`Cmd+V`. Pasting changes the
in-memory document; save it to write the changes to disk.

## 5. Compare files

Choose `File -> Compare files…` and select two or more files. The comparison
table shows changed paths relative to the first file. Objects and arrays are
compared recursively, and a missing path is distinct from the value `null`.
Switch to the side-by-side diff to inspect a selected pair; when more than two
files are loaded, use the diff selectors to choose the pair.

You can start in comparison mode from a terminal as well:

```sh
struct_view before.json after.yaml
```

Comparison is read-only. Use `File -> Open…` or `File -> Close file` to return
to the regular document view.

## 6. Use the command line

The CLI runs without opening the GUI. During development, prefix the examples
with `cargo run --`; otherwise, use the release executable or a `struct_view`
command available on your `PATH`. CLI commands and messages are in English.

Check a file's syntax and detected format:

```sh
struct_view validate data.json
```

Format a file, write the result to another file, or produce compact output:

```sh
struct_view format data.json --output normalized.json
struct_view format data.json --minify
```

The output extension selects the format when `--output` is supplied. If that
extension is unsupported, the input format is used. Without `--output`, the
input format is preserved.

Search for a value, a key, or a path:

```sh
struct_view find Alice data.json
struct_view find --keys --exact id data.json
struct_view find --paths --regex '^users\.[0-9]+\.name$' data.json
```

Use `key: value` to match a field name and its value on the same node:

```sh
struct_view find 'name: Alice' data.json
```

`--regex` cannot be combined with `--exact` or `--whole-word`. Case
sensitivity can still be enabled for a regular expression.

Compare two or more files and print changed paths:

```sh
struct_view diff before.json after.json
```

`compare` is an alias for `diff`. Use `-` as an input to read one source from
standard input. In shells that support input redirection, for example:

```sh
struct_view validate - < data.yaml
```

`--help` lists all commands and options.

The CLI returns exit code `0` on success, `1` for data or I/O failures, no
search matches, or differences found by `diff`, and `2` for command-line
argument errors. A `diff` exit code of `1` can therefore mean that the files
were successfully compared and differences were found.

## 7. Format and performance notes

- The GUI reads the complete document and keeps its parsed tree in memory.
  Row virtualization reduces rendering work for large expanded trees, but
  memory use still grows with the number of nodes. For very large documents,
  keep unrelated branches collapsed or use CLI commands.
- TOML has no `null` value. TOML date/time values keep their native type when
  saved as TOML and become strings when converted to JSON-compatible formats.
- YAML non-string mapping keys are preserved when saving as YAML.
  Converting them to JSON or TOML is rejected rather than changing their
  types.
- Comments in JSON5, YAML, and TOML can be retained when saving to a format
  that supports them, but their original positions and styles are not
  preserved. Comments are normalized to standalone lines at the start of the
  output; strict JSON does not support comments.

For the complete feature list and additional format details, see the
[project README](../../README.md).
