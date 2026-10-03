# StructView Tutorial

[Русская версия](ru.md) | [Project overview](../../README.md)

This walkthrough shows how to open, explore, edit, compare, and process
structured-data files with StructView. Menu names below use the English UI;
select it in `Settings -> Language` (`Настройки -> Язык`) if needed.

## 1. Install and open a document

StructView reads and writes JSON (`.json`), YAML (`.yaml`, `.yml`), TOML
(`.toml`), and JSON5 (`.json5`). It also imports Graphviz DOT (`.dot`, `.gv`),
GraphML (`.graphml`), and GEXF (`.gexf`) as read-only graph documents that can
be converted to those structured-data formats. For files, it uses the
extension when recognized and tries automatic format detection for unknown
extensions. Data read from standard input is detected from its contents.

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
- **Graph** — inspect inferred or explicitly described relationships. The
  supported graph shapes and examples are listed in [Graph input formats](#graph-input-formats).
  Hover a shortened edge label to see its full text. Layout is calculated in
  the background, with a progress indicator while it is being built.
- **Table** — see flattened paths, values, and types. The table follows the
  active search filter and can be exported as CSV.
- **Schema** — inspect JSON Schema or OpenAPI component and inline path
  schemas. For an ordinary data document, StructView infers a schema from the
  sample; that inferred view is not a formal contract.

### Graph input formats

The graph view recognizes graph-shaped data inside JSON, JSON5, YAML, and TOML
documents and imports standalone DOT, GraphML, and GEXF files. GraphML and GEXF
content with an `.xml` extension is detected by its root element. Select
**View → Graph** after opening a document.

Imported DOT, GraphML, and GEXF files are read-only. The importer converts them
to a normalized `graph`/`nodes`/`edges` document while retaining graph
direction, node IDs and labels, parallel edges, weights, and supported
attributes. Imported graphs use `directed_multigraph` or
`undirected_multigraph` so duplicate edges are not lost, except for strict DOT
graphs, which use `directed` or `undirected`. GraphML key
definitions and GEXF attribute definitions are kept in the `graph` metadata;
other XML extension elements are kept under `xml_extensions`.

DOT graph attributes and node/edge attributes are retained, and chained edges
are expanded into individual edges. Subgraphs are flattened for graph display;
their IDs, member nodes, and attributes are kept in `graph.subgraphs`. Source
comments and formatting are not retained.
Quoted DOT IDs and attributes are decoded without retaining surrounding
quotes; Graphviz label escapes such as `\n` remain intact. In strict graphs,
repeated edges update the existing edge's explicitly supplied attributes
instead of creating parallel edges or reapplying changed defaults.

GraphML and GEXF inputs must contain one graph with a uniform edge direction.
Mixed directed and undirected edges are rejected. Nested GraphML graphs and
hyperedges are not supported. External XML `DOCTYPE` declarations are ignored
without loading a DTD; internal DTD entities are not supported. GEXF `mutual`
edges are converted to undirected edges; dynamic timing and visualization
extensions are retained as metadata, but the graph view displays a static
topology.
XML attributes whose distinct keys map to the same output name are rejected
instead of silently overwriting data. An explicit value can override the
default for its own key. GEXF native `label` or `weight` values must not
conflict with user-defined attributes of the same name.
Generated `xml_attributes` and `xml_extensions` metadata must not overwrite
user-defined values with those names; such conflicts are also reported.

Convert an imported graph from **File → Convert to → JSON, YAML, or TOML** (or
JSON5). The source file stays open and is not overwritten. The CLI can write
the same normalized document to a new file:

```sh
struct_view format network.graphml --output network.json
struct_view format network.dot --output network.yaml
```

When formatting a graph input to standard output without `--output`, the CLI
emits JSON.

For example, the normalized representation has this shape:

```json
{
  "graph": {
    "type": "directed_multigraph",
    "source_format": "graphml",
    "name": "Build"
  },
  "nodes": [
    {"id": "compile", "label": "Compile", "attributes": {"kind": "task"}},
    {"id": "test", "label": "Test", "attributes": {}}
  ],
  "edges": [
    {
      "source": "compile",
      "target": "test",
      "edge_id": "e1",
      "label": "runs before",
      "weight": 0,
      "attributes": {"label": "runs before", "weight": 0}
    }
  ]
}
```

The same shape is written as YAML or TOML when those formats are selected.

#### Inferred entity relationships

Objects with `id`, `_id`, or `$id` become graph nodes. Their display label is
taken from `name`, `title`, or `label` when available. Fields such as `$ref`,
`user_id`, `parent_id`, and `depends_on` create links to matching IDs. A
reference may also be a JSON Pointer, such as `#/$defs/User`. Ambiguous
duplicate IDs are deliberately left unlinked.

```json
{
  "services": [
    {"id": "api", "name": "API", "depends_on": ["db", "cache"]},
    {"id": "db", "name": "Database"},
    {"id": "cache", "name": "Cache"}
  ]
}
```

Objects under JSON Schema `definitions`, `$defs`, and `schemas` are also
recognized as entities, so `$ref` links between them appear in the graph.

#### Explicit node and edge lists

Use an object with `graph`, `nodes`, and `edges`. Each node is an object with
`id`, `_id`, or `$id`; `name`, `title`, or `label` sets its display label.
Each edge uses `source` and `target` (or `from` and `to`). Edge labels may
include `label`, `name`, or `relation`; `weight` or `value`; and optional
`cardinality`, `contract`, or edge ID fields. `graph.weight_unit` is appended
to numeric weights.

```json
{
  "graph": {
    "name": "Build pipeline",
    "type": "weighted_directed",
    "weight_unit": "ms"
  },
  "nodes": [
    {"id": "compile", "label": "Compile"},
    {"id": "test", "label": "Test"}
  ],
  "edges": [
    {"source": "compile", "target": "test", "relation": "runs before", "weight": 120}
  ]
}
```

The `graph.type` values are `directed`, `weighted_directed`, `undirected`,
`weighted_undirected`, `directed_multigraph`, and `undirected_multigraph`.
Multigraph types keep parallel edges, including edges with the same endpoints.

#### Undirected adjacency lists

Set `graph.type` to `undirected` and provide an `adjacency` object whose keys
are nodes and whose arrays list their neighbors. A neighbor that has no key of
its own is still added as a node. Symmetric entries are deduplicated.

```json
{
  "graph": {"type": "undirected"},
  "adjacency": {
    "anna": ["boris"],
    "boris": ["anna"],
    "isolated": []
  }
}
```

#### Weighted undirected adjacency matrices

Set `graph.type` to `weighted_undirected`, provide `node_order`, and use a
square, symmetric `adjacency_matrix` in that order. The graph view reads the
upper triangle and ignores the diagonal. A numeric cell is an edge weight,
including `0`; use `null` to mean that no edge exists. Thus, a zero-weight edge
and a missing edge are distinct.

```yaml
graph:
  type: weighted_undirected
  weight_unit: km
  node_order: [A, B, C]
  adjacency_matrix:
    - [0, 12, null]
    - [12, 0, 0]
    - [null, 0, 0]
```

This describes edges `A—B` with weight `12 km` and `B—C` with weight `0 km`;
there is no edge between `A` and `C`.

#### TOML entity-relation catalogs

For named entities and labeled relations, provide all four metadata fields
shown below. `edges` contains tuples `[source_id, target_id, label,
cardinality?]`. Set `directed = false` for undirected relations.

```toml
[graph]
name = "Store"
directed = true
entity_count = 2
relation_count = 1

[entities]
customer = "Buyer"
order = "Order"

[relations]
edges = [["customer", "order", "places", "1:N"]]
```

#### Bipartite and multipartite graphs

List at least two named partitions as arrays of node IDs, then refer to those
IDs in `[relations].pairs`. A pair may connect any two different partitions;
same-partition pairs are ignored. IDs should be unique across all partitions.
Use `type = "bipartite"` for exactly two partitions and
`type = "multipartite"` for two or more partitions. Each partition is laid out
in its own column. `directed` is optional and defaults to `false`.

```toml
[graph]
type = "multipartite"
directed = false

[partitions]
people = ["ada"]
projects = ["compiler"]
organizations = ["lab"]

[labels]
ada = "Ada"
compiler = "Compiler"
lab = "Research lab"

[relations]
pairs = [
  ["ada", "compiler", "writes"],
  ["compiler", "lab", "belongs to"],
]
```

#### TOML multigraphs

Use a `[nodes]` table mapping node IDs to labels and one `[[edges]]` table per
edge. `graph.type` must be `directed_multigraph` or `undirected_multigraph`.
Parallel edges are kept; edge records accept the same endpoint and label
fields as the explicit node/edge-list form.

```toml
[graph]
type = "directed_multigraph"

[nodes]
api = "API"
database = "Database"

[[edges]]
source = "api"
target = "database"
relation = "reads"

[[edges]]
source = "api"
target = "database"
relation = "writes"
```

## 3. Find a key, value, or path

Open the search window from the toolbar or press `Ctrl+F` (`Cmd+F` on macOS).
Enter a query, choose whether to search keys, values, or paths, and refine it
with case-sensitive, exact, whole-word, or regular-expression matching. Use
the previous/next controls to move through matches, or select a result path to
locate it in the document. The search window opens inside the main window
first. To detach it, click the three-dot menu at the top of the window and
choose `Detach window`. To dock it again, click `Attach window` at the top of the
detached window. The same applies to add/edit field dialogs.

For the sample above, search for `Alice` to find a value, or search keys for
the exact name `id`. The regex builder can insert escaped literal text and
common pattern fragments. Regular expressions use Rust regex syntax.

## 4. Edit and save

Switch to **Edit** mode to change the document. Open a node's context menu and
choose `Edit field…` to edit its value or type; object fields can also be
renamed. Add object fields and array elements through the type-aware
constructor using the `+` button next to each object or array. Enter field
names and choose types manually. Strings are entered as plain text, numbers and
booleans are validated, and new objects and arrays start empty. Add nested
fields the same way from their own `+` buttons. TOML does not support
`null`, so that value is unavailable when editing TOML.

To start a document from scratch, choose `File -> New file…`, give it a
supported extension, and add fields to the new empty object with its `+`
button. Undo and redo are available for up to 100 document changes (`Ctrl+Z` /
`Ctrl+Y`; on macOS, `Cmd+Z` / `Cmd+Shift+Z`).

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
