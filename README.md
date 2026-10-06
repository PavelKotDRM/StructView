# StructView

A fast, cross-platform viewer and editor for structured data. The application
displays JSON, YAML, TOML, and JSON5 as an interactive tree, imports Graphviz
DOT, GraphML, and GEXF graphs for inspection and conversion, and provides
headless commands for formatting, validation, search, comparison, editing,
and table, schema, and graph export.

[Русская версия документации](docs/README.ru.md)

Step-by-step tutorials: [English](docs/tutorial/en.md) | [Русский](docs/tutorial/ru.md).

## Features

- JSON, YAML, TOML, and JSON5 support;
- read-only Graphviz DOT, GraphML, and GEXF import with conversion to JSON,
  YAML, TOML, or JSON5;
- automatic format detection for stdin and files with unknown extensions;
- interactive object and array tree with expandable and collapsible nodes;
- full-text search across keys, values, and JSON paths;
- search filters for key/value/path scope, case sensitivity, exact and
  whole-word matching, and regular expressions;
- a regex builder for escaped literal text and common pattern fragments;
- switchable tree, relationship graph, flattened table, and schema views;
- a separate JSON/JSON5/YAML/TOML data-structure diagram with source-ordered
  nodes, comment nodes for supported formats, collapse/expand, three layouts,
  search, zoom/pan, and SVG/PNG export;
- graph links inferred from common entity identifiers and reference fields;
- per-edge directed, undirected, bidirectional, and reverse links, mixed graphs,
  and self-loops in the GUI and graph exports;
- CSV export of table rows, respecting the active search filter;
- JSON Schema and OpenAPI schema views, including component and inline path
  schemas, with inferred sample structure for ordinary data documents;
- native TOML date/time editing and round-tripping;
- editing mode for field values and primitive values;
- creating a new empty JSON, YAML, TOML, or JSON5 file directly in edit mode;
- adding object fields and array elements;
- a field constructor with explicit string, number, boolean, null, object, and
  array types, plus TOML date/time values;
- saving editable structured-data files to supported output formats and
  converting imported graph files to those formats;
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

| Format | Extensions | Access |
| --- | --- | --- |
| JSON | `.json` | Read and write |
| YAML | `.yaml`, `.yml` | Read and write |
| TOML | `.toml` | Read and write |
| JSON5 | `.json5` | Read and write |
| Graphviz DOT | `.dot`, `.gv` | Read-only import |
| GraphML | `.graphml` | Read-only import |
| GEXF | `.gexf` | Read-only import |

For files, the format is detected from the extension first. If the extension
is unknown, the content is parsed using automatic format detection. For stdin,
the format is detected from the content.
GraphML or GEXF documents with an `.xml` extension are identified by their root
element.

DOT, GraphML, and GEXF are normalized to a graph with `graph`, `nodes`, and
`edges`. Use `File -> Convert to` in the GUI or, for example,
`struct_view format network.graphml --output network.json`, to save that
normalized data as JSON, YAML, TOML, or JSON5. Imported graph files are
read-only and are never overwritten. See the
[graph input guide](docs/tutorial/en.md) for mappings, supported features, and
conversion examples.

Non-string YAML mapping keys are preserved when saving as YAML. Converting a
document with such keys to JSON or TOML is rejected rather than changing their
types.

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
Single-file loads run in the background and show a loading indicator in the
status bar.
Use `File -> Compare files…` to select two or more files for comparison.
Use `File -> New file…`, choose a filename with a `.json`, `.yaml`, `.yml`,
`.toml`, or `.json5` extension, and start adding data to the empty object in
edit mode.

The interface provides:

- a responsive menu bar with `File`, `Edit`, `View`, `Settings`, and `Help`
  groups that collapse into one `Menu` on narrow windows;
- a `File` menu for creating, opening, saving, converting to another format,
  saving to a new file, and closing a document;
- a view selector for the interactive tree, relationship graph, flattened
  table, and schema diagram;
- `View -> Visualization -> Data structure diagram`, also available without an open document.
  This independent mode draws the file hierarchy as boxes and parent-child
  edges; it never infers relationships or uses the relationship graph model.
  The current file is read as a separate disk snapshot on first entry. Use
  **File -> Open**, drop one data file onto its canvas, or open
  **View -> Source text** and paste JSON/YAML/TOML into the source window.
  Choose **Settings -> Data format -> Auto / JSON / YAML / TOML** and
  **File -> Build diagram**. Unsaved edits in other views are not imported.
  Parsing happens in the background; errors show their line and leave the
  last valid diagram intact, with an explicit stale-source notice.
  Choose top-to-bottom, left-to-right, or compact layout under
  **View -> Structure layout**. Zoom, 100%, fit, select-all, clear-selection, selected-node toggle,
  and **Show next 100 children** also live in **View**. Click a container
  to toggle it; large containers and deeper branches start collapsed.
  Expanded containers initially show at most 100 children; select the parent
  and use **Show next 100 children** to reveal more. The total node count
  includes hidden nodes. YAML anchors are labeled and aliases are terminal
  reference nodes, including recursive aliases; they do not add cross-edges.
  Keys retain source order within their parent; TOML dotted tables are grouped
  by their hierarchy. YAML streams have a synthetic array root.
  Both diagram modes share the same main-menu zoom/selection controls and
  **View -> Tree actions -> Expand all / Collapse all**. In structure mode,
  expand-all retains paging for large containers.
  Use the common advanced search window via the magnifying-glass button,
  **Edit -> Advanced search**, or Ctrl/Cmd+F. The same key/value/path scopes,
  case sensitivity, exact/whole-word matching, regex builder, and result list
  are available in both modes. Navigating to a match
  expands the necessary ancestors, reveals paged children, and centers the
  result. Selection highlights its path back to the root. Hover a node for
  its full key, value, and JSON Pointer path.
  Selected nodes, including whole objects and arrays, copy via `Edit -> Copy`,
  `Ctrl+C`, or a node's context menu. In edit mode, paste them into a selected
  object or array via `Edit -> Paste`, `Ctrl+V`, or the container's
  `Paste here` context-menu entry, just like in the JSON tree.
  Drag the canvas to pan and use the wheel or +/- to zoom. With the canvas or
  a node focused, use Up/Down to select, Left to collapse/go to the parent,
  Right to expand/go to a child, Enter/Space to toggle, and Home to fit.
  The canvas shows only the diagram; selected path/value, file name, format,
  node counts and zoom are shown in the status bar. Hover the selected path/value
  to see its full text. The color legend and navigation
  instructions live in **Help -> Diagram legend and controls**.
  Legend entries show a color swatch and type with the color name in parentheses,
  without a duplicate text legend. Controls are shown as keycaps/gesture badges
  next to actions, with shared controls and mode-specific navigation grouped separately.
  Collapsing/expanding a branch centers its node without changing zoom.
  All views use separate status fields with GUI dividers, not `|` characters,
  and do not repeat the view name in the status bar.
  Version and build metadata live in **Help -> Build information**.
  There is no separate diagram toolbar. Native AccessKit widget labels expose
  full node text; only on-screen nodes are painted. Input nesting is limited
  to 128 levels to report excessive depth rather than overflow the stack.
  **File -> Export diagram -> SVG / PNG** exports the entire currently
  expanded/paged layout, not just the viewport, without search/selection
  highlights. SVG includes full values in titles; PNG is proportionally
  scaled down if necessary to stay within 16 million pixels. Export runs in
  the background. Both diagrams offer the same light/transparent and
  dark/opaque export styles, independently of the application theme.
- a relationship graph inferred from IDs and references, plus explicit
  directed, undirected, weighted, and multi-edge graph schemas; undirected
  adjacency lists and weighted matrices; TOML entity-relation, bipartite, and
  multipartite graphs; read-only Graphviz DOT, GraphML, and GEXF imports.
  Parallel edges are retained for multigraphs. See the
  [graph input guide](docs/tutorial/en.md) for supported shapes and examples;
  layout is calculated in the background with a progress indicator.
  Graphs with at least 64 edges calculate preliminary routes in parallel,
  using up to eight workers and leaving one logical CPU available when
  possible. Conflict checks and rerouting also run in parallel against a
  snapshot of accepted routes. Results are accepted in a fixed order; stale
  conflicting proposals are recalculated before acceptance. Small graphs
  and single-CPU systems use sequential routing.
  Routing uses a spatial segment index and caches search-step penalties to
  avoid repeatedly checking distant links during conflict resolution.
  Nodes are arranged by relationships rather than document order: directed
  acyclic connections flow left to right, cycles share a layer, disconnected
  components have separate row bands, and partitioned graphs retain their
  partition columns. Neighbor-based ordering reduces crossings within layers.
  Calculation feedback shows the current stage and completed link count.
  `View -> Calculation stage timings` contains elapsed time, worker count,
  per-stage timings, and the slowest completed stage, including after completion.
  Per-stage durations are listed in its `Stage details` submenu.
  The shared **View** menu provides zoom out/in, 100%, fit-to-view, select-all,
  and clear-selection controls for both diagram modes. Ctrl-click (Cmd-click on macOS) toggles individual
  nodes; Shift-drag on empty canvas selects nodes with a rectangle, holding
  Ctrl/Cmd to add to the selection. Both diagrams use ordinary dragging to pan,
  wheel/pinch to zoom about the pointer, +/- to zoom, and Home to fit.
  With the canvas or a node focused, Up/Down select the previous/next node.
  In the relationship graph, Left/Right visit incoming/outgoing neighbors
  (or another connected neighbor when none exists in that direction);
  Enter/Space select the current node. Selected nodes highlight their connections.
  Hover a connection line or its label to see its full label, direction, and
  endpoint names, identifiers, and document paths. At a crossing, equally
  close connections are listed together.
  `File -> Export diagram -> SVG / PNG` saves the entire graph at its original scale,
  without temporary selection or search highlights. Choose light styling
  with a transparent canvas or dark styling with an opaque dark canvas,
  independently of the application theme. Relationship labels are exported
  in full, with callouts and image bounds adjusted to fit the complete text.
  SVG preserves vector shapes and text. Large PNG images are automatically
  downscaled to at most 16 million pixels to bound memory use, preserving
  the entire graph, aspect ratio, and selected background style. Small images
  retain their original resolution. For full detail at any scale, use SVG.
  Incoming and outgoing links share distinct, neighbor-ordered card ports;
  detours use separate tracks where space permits, and relationship labels
  avoid cards, connection lines, and arrowheads. Displaced labels have dashed
  leaders identifying their connection. If a dense graph has no free space
  near a route, its label is placed in a reserved callout column instead of
  being hidden; the scrollable canvas and exported image expand to include it;
  displaced-label leaders are checked against nodes, labels, routes, and other
  leaders. Where a clear direct leader is impossible, matching numbered
  markers identify the connection and its label without a long crossing line.
  Numbered connection markers reserve space around arrowheads, including when
  dense routes require fallback placement; the same rule applies to SVG/PNG.
- a flattened path/value/type table that follows the search filter and can be
  exported as CSV;
- a schema diagram for JSON Schema and OpenAPI component/inline path schemas;
  other documents show an inferred schema, clearly marked as sample-derived
  rather than a contract;
- a comparison table for all selected files and a side-by-side diff for any
  selected pair, highlighting additions, removals, and changes;
- controls for expanding and collapsing the whole tree;
- a separate, resizable search window opened from the toolbar or with
  `Ctrl+F` (`Cmd+F` on macOS), with previous/next navigation and selectable
  result paths;
- search options for key/value/path scope, case sensitivity, exact and
  whole-word matching, and Rust regular expressions;
- a regex builder that appends escaped literal text and common pattern
  fragments to the active query;
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
The selected pair is compared recursively independently of the other loaded
files, even when another file has a different container type.

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

Editing an existing field preserves its exact name, including empty names and
leading or trailing spaces. Newly entered field names are trimmed and must not
be empty. The float constructor rejects text that the selected format would
interpret as a string. CSV exports use the same atomic file replacement as
document saves, so a failed write does not truncate an existing file.

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

CLI commands, options, help text, and command diagnostics use English.
Details from the shared parsers and editing validators may be localized.
Command and option names are language-independent.

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
Standard output ends with one newline, even when the serializer already
includes one (as with YAML and TOML).

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

Search keys, values, and JSON paths and print the paths of matching nodes:

```sh
struct_view find user data.json
```

Use `key: value` to match both a field name and its value on the same node:

```sh
struct_view find 'name: Alice' data.json
```

Additional options:

```text
--keys            search keys;
--values          search values;
--paths           search JSON paths;
--case-sensitive  match letter case;
--exact           require an exact match;
--whole-word      require a whole-word match;
--regex           interpret the query as a Rust regular expression.
```

`--regex` cannot be combined with `--exact` or `--whole-word`; case sensitivity
still applies to regular expressions.

Examples:

```sh
struct_view find --keys name data.json
struct_view find --values --case-sensitive ADMIN config.json
struct_view find --keys --exact id data.json
struct_view find --paths --regex '^users\.[0-9]+\.name$' data.json
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

### Reading, creating, and editing documents

All commands run without a native window. A missing input file or `-` reads
stdin. Document output goes to stdout unless `--output FILE` or, for edits,
`--in-place` is specified. Writes use atomic replacement and are attempted only
after parsing, editing, and serialization succeed.

```sh
struct_view new empty.json
struct_view new --to yaml
struct_view convert data.json --to yaml
struct_view get user.name data.json
struct_view get user.name data.json --raw
struct_view get user.name data.json --part key
struct_view get user.name data.json --part path
struct_view add '$' data.json --key enabled --type boolean --value true
struct_view add users data.json --type object
struct_view set user.name data.json --type string --value 'Grace Hopper' --in-place
struct_view rename user.name display_name data.json --output renamed.json
struct_view delete data.json --path users[0] --path obsolete --in-place
```

Paths accept the spelling printed by `find`, including quoted keys such as
`user["a.b"]`, or the `$`-prefixed paths exported by the table. `$` identifies
the root. Multiple `--path` selections for `copy` and `delete` are validated
before modification; selecting a parent and its descendants processes the
parent only once. Root deletion is not allowed.

`get` prints the serialized value by default. `--part key` or `--part path`
prints the field name/index or tree path as text. `--raw` prints the text of a
string, native date/time, or comment without JSON quotes. A root has no key.
Extracted TOML values other than objects default to JSON because TOML cannot
serialize a standalone scalar or array document; `--to` explicitly overrides this.

`new` creates an empty object and refuses to replace an existing file.
`convert`, `new`, `get`, and edits support `--to json|yaml|toml|json5` and
`--minify`. Without `--to`, an output filename's extension selects the format,
otherwise the input format is retained. Conflicting `--to` and output
extensions are rejected. Imported graphs default to JSON for conversion and
are read-only: they cannot be edited or overwritten, even through an `.xml`
filename.

`add` and `set` use the same typed constructor as the GUI:

| `--type` | `--value` |
| --- | --- |
| `string` | plain text (not a quoted literal) |
| `number`, `float` | a numeric literal; `float` preserves the float type |
| `boolean` (or `bool`) | `true` or `false` |
| `null`, `object`, `array` | no `--value`; new containers are empty |
| `datetime` | native TOML date/time, TOML documents only |
| `comment` | comment text, JSON5/YAML/TOML only |
| `metadata` | tagged YAML value, for example `!custom value` |

`add` requires `--key` when inserting an object field; array elements append.
`set --key` also renames an object field. Newly entered names are trimmed and
must be nonempty; duplicate names fail. As in the GUI, setting a container to
its existing type preserves its children. To insert populated structures, use
`paste`. Native dates, comments, and YAML tags retain the same format-specific
restrictions as GUI editing and saving.

### Copying and pasting structures

```sh
struct_view copy source.json --path user --path settings --output selection.json
struct_view paste '$' target.json --from selection.json --in-place
struct_view copy source.json --path user | struct_view paste '$' target.json --in-place
struct_view copy source.json --path user --clipboard
struct_view paste '$' target.json --clipboard --output pasted.json
```

`copy` emits the same JSON selection envelope as the GUI, retaining field names
and nested structure. `paste` accepts that envelope or ordinary JSON from
`--from FILE` (or `-`), defaulting to stdin. Only one source can be stdin.
Unkeyed root objects merge their fields into objects; root arrays append their
elements to arrays. Keyed entries retain names in objects and append as values
in arrays. Duplicate object keys fail rather than overwrite.

`--clipboard` explicitly uses the system clipboard instead of the selection
stream. This option requires an accessible platform clipboard; all other
operations, including image export, work without a display server.

### Table, schema, and graph exports

```sh
struct_view table data.json --output table.csv
struct_view table data.json --query name --keys --exact
struct_view schema data.json --output schema.json
struct_view schema api.yaml --query id --keys
struct_view graph network.graphml --output graph.json
struct_view graph network.graphml --output graph.svg
struct_view graph network.graphml --output graph.png --dark
```

`table` exports the GUI's flattened CSV with `path,value,type` columns, including
containers, empty values, comments, and native types. `table` and `schema`
accept `--query` and all `find` search flags; an empty result is a successful
empty export (CSV header or empty schema rows).

`schema` exports the GUI diagram model, not a newly generated JSON Schema
contract: `source` is `json-schema`, `openapi`, or `inferred`; `title` is optional;
`rows` contain `key`, `path`, `type`, `required`, `constraints`, and `reference`.
Sample-derived schemas are explicitly marked `inferred`.

`graph` exports the GUI relationship model as JSON: `directed`, `direction`, `partitions`,
`nodes` (`id`, `label`, `path`, `partition`), and `edges`
(`source`, `target`, `label`, `direction`). The summary `direction` is `mixed`
when edge directions differ; `directed` means at least one edge has an arrow.
Edge directions are `directed`, `undirected`, `bidirectional`, or `reverse`.
Edge endpoints are zero-based node indexes;
parallel edges and partitions are retained. No recognized entities is an error.
Self-loops are routed outside their node and retained in all graph outputs.
See [per-edge direction and self-loops](docs/tutorial/en.md#per-edge-direction-and-self-loops)
for input fields, DOT `dir`, GEXF `mutual`, and GraphML direction rules.
The GUI and CLI also share adapters for node-link/NetworkX, Cytoscape,
keyed dictionaries, edge tuples, and adjacency lists in JSON, JSON5, YAML,
and TOML. See [supported graph structures](docs/tutorial/en.md#supported-graph-structures)
for shapes, defaults, and validation rules.
An `.svg` or `.png` output extension selects image export; alternatively use
`--image svg|png --output FILE`. Images require an output file and use the GUI's
full graph layout and unabridged relationship labels. The default is light
styling on a transparent canvas; `--dark` uses an opaque dark canvas.
PNG retains the entire graph and is downscaled when necessary to at most
16 million pixels; SVG retains vector detail. Interactive zoom, selection,
themes, language, and undo/redo are not CLI document operations.
When stderr is an interactive terminal, graph image exports show per-stage
progress bars with route counts, worker counts, elapsed time, and stage timings.
Table, schema, and graph-model exports show activity spinners while reading,
parsing, and calculating. Progress is written to stderr and is omitted when
stderr is redirected, so machine-readable stdout stays unchanged.

The CLI reuses a window-free API in the UI crate for editing, clipboard
envelopes, and visualization models, and the existing in-memory graph renderer
for SVG/PNG. It does not initialize the native GUI.

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
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --doc
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

GitHub Actions runs the `Quality checks` workflow on pushes and pull requests;
it can also be started manually from the Actions tab. Checks cover every
workspace crate, CLI integration tests, and documentation examples.
Pushing a version tag that matches `Cargo.toml`, for example `v0.1.0`,
builds binaries for Linux, Windows, and macOS and publishes them as a GitHub
Release.

Cargo workspace crates:

- [`struct-view-core`](crates/struct-view-core/README.md) — parsing, document
  trees, search, and comparison;
- [`struct-view-cli`](crates/struct-view-cli/README.md) — command-line parsing
  and headless commands;
- [`struct-view-ui`](crates/struct-view-ui/README.md) — native GUI, views,
  editing, and clipboard;
- [`struct-view-build-info`](crates/struct-view-build-info/README.md) — shared
  compile-time build metadata.

The root `struct_view` crate is the executable entry point and compatibility
facade; its documentation is this README. Its source layout is:

```text
src/
├── main.rs      executable entry point and command dispatch
├── lib.rs       compatibility facade re-exporting workspace crates
└── console.rs   platform-specific console integration
```

Each workspace crate's README contains its source tree and module
responsibilities. The `struct-view-build-info` crate embeds the Rust version,
target platform, build time, and optimization mode. This information is
available through `--version` and the `Help` menu.

## License

This project is distributed under the [MIT License](LICENSE).
