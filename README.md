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
- CSS color recognition and swatches in value views, including named colors,
  hexadecimal, RGB/HSL/HWB, Lab/LCH, OKLab/OKLCH, and CSS `color()` formats;
- a separate JSON/JSON5/YAML/TOML data-structure diagram with source-ordered
  nodes, comment nodes for supported formats, collapse/expand, three layouts,
  search, zoom/pan, and SVG/PNG export;
- graph links inferred from common entity identifiers and reference fields;
- per-edge directed, undirected, bidirectional, and reverse links, mixed graphs,
  and self-loops in the GUI and graph exports;
- optional authenticated graph animation API with scheduled camera/effects,
  persistent realtime markers, and independently managed jobs;
- named jobs with stage-local progress, status badges, hover details, and a
  focusable job list; concurrent jobs can share the same nodes and routes;
- CSV export of table rows, respecting the active search filter;
- JSON Schema and OpenAPI schema views, including component and inline path
  schemas, with inferred sample structure for ordinary data documents;
- native TOML date/time editing and round-tripping;
- editing mode for field values and primitive values;
- color picker controls for string values and graph color attributes, with a
  screen eyedropper, immediately applied output notation, or the existing
  notation; the saturation marker stays compact until hovered or dragged.
  The eyedropper captures screens in the background. On Windows it uses a native
  GDI snapshot overlay without additional OpenGL windows or fullscreen mode,
  avoiding graphics-surface transitions that can trigger G-Sync display flicker.
  Click to select a color, or press Escape to cancel;
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

On Debian/Ubuntu, install the native build dependencies first:

```sh
sudo apt-get update
sudo apt-get install --yes \
  build-essential pkg-config libclang-dev libdbus-1-dev libegl-dev libgbm-dev \
  libpipewire-0.3-dev libwayland-dev libx11-dev libx11-xcb-dev \
  libxcb1-dev libxcb-randr0-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxcb-xinput-dev libxcb-xinerama0-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libxrandr-dev
```

`libegl-dev` provides `egl.pc`, which `khronos-egl` requires through
`pkg-config`. `libgbm-dev` provides `libgbm.so` for linking with `-lgbm`.
These native dependencies are required at build time even if you only
intend to use command-line mode.

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
  **View -> Source text** and paste JSON/JSON5/YAML/TOML into the source window.
  Choose **Settings -> Data format -> Auto / JSON / JSON5 / YAML / TOML** and
  **File -> Build diagram**. Unsaved edits in other views are not imported.
  Parsing happens in the background; errors show their line and leave the
  last valid diagram intact, with an explicit stale-source notice.
  A parse error opens the full source for correction; selected-node previews
  are unavailable until the changed text has been rebuilt successfully.
  Loading another file dismisses the previous field-edit dialog.
  Source text can be saved verbatim, including an empty file, but saving is
  disabled while a load or rebuild is pending.
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
  In automatic mode, preliminary routes for all edges are calculated in
  parallel on every logical CPU. Conflict resolution then runs on one thread
  in edge order, so the routes are the same for any number of CPUs.
  Routing uses a spatial segment index and caches obstacle and lane checks to
  avoid repeatedly checking distant links during conflict resolution.
  Routes may cross other edges but cannot share collinear sections or pass
  through nodes.
  Routes are orthogonal, except that a link whose ports can be joined by one
  straight segment is drawn as that segment when the segment is at most two
  layout columns long (680 points), clears every card, stays 8 points or more
  from other links' corners and ends, and crosses at most one other link.
  Straight links are chosen after the orthogonal layout is final, so they never
  change the routes of other links. Edges between layout columns leave
  and enter through the facing left and right sides, so relationships flow left
  to right. A node with more than seven edges uses, for each of its edges, the
  side that geometrically faces the other endpoint, because a left or right side
  has room for only about seven lanes. Cards with more than 22 edges grow wider,
  about 8 points per edge, so each edge keeps its own lane on a top or bottom
  side. Routes minimize length plus a fixed cost per bend (including
  turns at port leads), do not loop back around their own port entries, keep
  other ports' straight exits free, and among equal routes center themselves on
  the midline between their exits, so parallel routes bundle symmetrically.
  Port entries are reserved so other routes cannot occupy them. Only the gaps
  next to highly connected nodes grow. If routing still runs out of space,
  retries widen only the gaps around the crowded edges and recompute ports and
  routes without allowing shared sections. Up to two further passes widen the
  gaps around routes that other edges forced into extra bends or detours.
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
  close connections are listed together. Hold Ctrl/Cmd while hovering a
  connection line to keep its popup open as you move the pointer into it;
  release the modifier there to scroll through the connection details.
  `File -> Export diagram -> SVG / PNG` saves the entire graph at its original scale,
  without temporary selection or search highlights. Choose light styling
  with a transparent canvas or dark styling with an opaque dark canvas,
  independently of the application theme. Relationship labels are exported
  in full, with callouts and image bounds adjusted to fit the complete text.
  SVG preserves vector shapes and text. Large PNG images are automatically
  downscaled to at most 16 million pixels to bound memory use, preserving
  the entire graph, aspect ratio, and selected background style. Small images
  retain their original resolution. For full detail at any scale, use SVG.
  Incoming and outgoing links use distinct, neighbor-ordered card ports. Cards
  with more than 22 links also show their incoming and outgoing link counts and
  their document path.
  Long shared route segments are locally detoured when a clear lane is
  available; routes may still cross in dense layouts.
  Relationship labels avoid cards, connection lines, and arrowheads. Displaced labels have dashed
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

### Relationship graph editor

In the relationship view, use **File -> New graph** to start an empty, unsaved
graph in Edit mode. For any existing graph, including inferred relationships
and DOT/GraphML/GEXF imports, **File -> Create editable graph copy** opens a
separate document without writing to the source file.
Unsaved changes are confirmed before replacing the current document.

**View -> Graph editor** opens a resizable window with entity, link, group,
and graph-property tabs. Like the search window, it starts inside the main
window and can be collapsed or expanded by double-clicking its title.
Its three-dot menu offers **Detach window**, and the separate window
offers **Dock window** to return. Reopen a closed editor through **View**.
**Add entity**, **Add link**, **Add group**, and **Graph properties** are in
**Edit**, with duplicate buttons at the top of the editor window, including
when detached. Graph commands are hidden outside the relationship view; the editor
window and mutation commands require an editable graph in Edit mode.
The editor supports:

- add, rename, duplicate, and delete entities and links;
- choose source and target entities, including self-links and parallel links;
- set directed, undirected, bidirectional, or reverse directions, optional
  numeric weights, weight units, labels, and typed JSON-object attributes;
- create and rename groups, assign entities to them, or remove a group without
  deleting its entities;
- use **Apply and add connected entity** in an entity's dialog to create
  another entity with a link from the current one;
- open an element's editor from its list or by double-clicking a node or link
  label. Unlabelled links remain editable from the link list.

Deleting an entity also deletes its incident links, after confirmation.
Changes use the shared document undo/redo history. Node placement and link
routing are always automatic: dragging pans the canvas, not individual nodes.
Groups affect automatic layout; ungrouped entities have their own column when
groups exist. Changing the graph's default direction affects new links only.

**Save / Save as** stores the editable graph as JSON, JSON5, YAML, or TOML.
SVG/PNG export continues to use the automatic layout. The editable format uses
`graph.editor_version: 1`, `graph.type: "mixed_multigraph"`, `nodes`, `edges`,
and a `partitions` array of group names; entity membership is a `partition`
string. IDs must be nonempty and unique, and link endpoints must exist.
Copies retain a normalized, typed source snapshot in `graph.source_document`;
unsupported source-specific details remain available there. Node source
records are retained as attributes; recognized numeric link weights become
editable weights, and other link tooltip attributes are retained as text.
Editing the copy does not synchronize it back to the source.

**Attributes** contains additional data only. **Unique ID** and **Name / label**
are core `id`/`label` fields, not entries in `attributes`; they are saved even
when the attributes object is empty. Expand **Full element JSON (preview)** in
an entity, link, or graph-properties dialog to see all current fields together.
This read-only preview updates while typing; malformed attributes or weights
show an error instead of an outdated preview.

### Graph animation API

Open a graph in the relationship view and wait for its layout to finish.
Choose **Settings -> Graph animation API** to open the API settings window.
The window can be detached and docked back into the main window. Port `0`
selects an available port; alternatively set an explicit port before enabling.
The window displays the actual `http://127.0.0.1:PORT` address and a selectable
Bearer token. All requests require `Authorization: Bearer TOKEN`.
The API is off by default, initially listens on IPv4 loopback, rejects browser
`Origin` requests, and requires a configured `Host` header. Use a local or remote
script or another native application, not a browser page. Keep the token
private. **Disable API** closes the listener and stops playback; enabling it
again generates a new token. Neither the token nor enabled state is saved.

The API window includes **Copy token**, **Paste token**, **Apply token**, and
**Generate new token**. A custom token must contain 16–256 printable ASCII
characters without spaces; use a long random secret. Applying or regenerating
changes authentication immediately without restarting the listener or playback.
Old credentials are rejected for new requests; already authorized requests may
finish. Restart external controllers with the new token. No token is persisted.

The server controls three independent visual layers:

| Layer | Use | Lifetime and cleanup |
| --- | --- | --- |
| Scheduled (`/v1/animation`) | One timed scenario with camera, highlights, and flow | New scenarios replace playback; scheduled `clear` affects only this layer |
| Realtime (`/v1/realtime`) | Low-level persistent marker and element effects | Explicit updates/removal; realtime `clear` affects only this layer |
| Jobs (`/v1/jobs`) | Up to 64 independently identified task snapshots | Explicit updates/removal; jobs `clear` affects only this layer |

Use jobs for concurrent processes with labels, stages, statuses, and progress.
They are visual representations of externally running work, not a worker
queue: the API does not execute tasks or provide per-job pause/resume.
The scheduled player remains a single scenario; concurrency is represented
by independent jobs or realtime IDs. Global stop clears all three layers.

Commands animate the existing automatically laid-out graph without editing
entities, attributes, routes, undo history, or saved files. Highlights and
moving markers are not included in SVG/PNG exports. Switching views, entering
comparison, or invalidating the graph calculation cancels playback. The API
works in View and Edit modes, including imported graphs; no editable copy is
required. It cannot move individual nodes or edit a document.

| Method | Endpoint | JSON response / behavior |
| --- | --- | --- |
| GET | `/v1/capabilities` | API version, features, job commands, statuses, and limits; also works without an active graph |
| GET | `/v1/graph` | `{graph_id, nodes:[{id,label}], edges:[{index,source,target,label,direction}]}` |
| GET | `/v1/animation` | `{graph_id,running}` |
| POST | `/v1/animation` | Accept a scenario, replacing playback; `{accepted:true,duration_ms}` |
| POST | `/v1/animation/stop` | Empty body; stop the scenario and clear realtime effects and jobs; `{stopped:true}`, also outside the graph view |
| GET | `/v1/realtime` | `{graph_id,markers,elements}` — persistent visual state |
| POST | `/v1/realtime` | Atomically apply realtime commands; `{accepted:true}` |
| GET | `/v1/jobs` | `{graph_id,jobs:[...]}` — independent jobs, stages, targets, and progress |
| POST | `/v1/jobs` | Atomically upsert/remove/clear jobs; `{accepted:true,job_count}` |

For JSON POST requests, send `Content-Type: application/json` and a
`Content-Length` (standard Python/PowerShell clients do this automatically).
Read `/v1/graph` first. Node targets use its string IDs; links use its zero-based
**indexes**, not the document's link IDs, so parallel links are distinguishable.
Send the returned `graph_id` unchanged; after graph changes fetch the descriptor
again. Unknown or ambiguous node IDs and unknown link indexes are rejected.
Hover over an entity or link to inspect each execution's label, current stage,
status, and marker progress/target while smoothing is in progress. Multiple
execution IDs may target the same entity or link simultaneously; their route
markers are drawn in separate lanes. Node badges include a short label, ID,
status, and stage; marker labels show the job and visible progress. Long badge
text is shortened, and nodes show at most three badges with a remaining-count
indicator. Full details remain in tooltips. For the jobs API, the expandable
**Graph jobs** list shows all jobs; click a location button to focus its node.

Scenario shape:

```json
{
  "graph_id": "COPY_FROM_GET_GRAPH",
  "steps": [
    {"at_ms": 0, "command": {"type": "highlight", "nodes": ["a"], "edges": [0], "pulse": true, "duration_ms": 2000}},
    {"at_ms": 0, "command": {"type": "camera", "node": "a", "zoom": 1.5, "duration_ms": 800}},
    {"at_ms": 800, "command": {"type": "flow", "edge": 0, "color": [40, 200, 255], "duration_ms": 1800}},
    {"at_ms": 2600, "command": {"type": "clear"}}
  ]
}
```

Replace `a`, `0`, and `graph_id` with values from your descriptor.
`at_ms` defaults to `0` and is measured from GUI acceptance, not wall-clock
time. Steps are stably sorted by time; equal-time steps keep input order.

| Command | Fields and behavior |
| --- | --- |
| `highlight` | `nodes` and/or `edges` arrays, optional RGB `color` and `pulse`; outlines existing entities/links for `duration_ms` |
| `flow` | Required `edge`, optional RGB `color` and `reverse`; a marker follows the routed polyline by distance |
| `flow_percent` | Required `edge` and `to_percent`, optional `from_percent` (default 0); move along that percentage segment over `duration_ms`, with the same `color`/`reverse` options as `flow` |
| `camera` | `node` and/or `zoom`, or `fit:true`; smoothly pans/zooms for `duration_ms` |
| `clear` | Cancels effects and camera commands preceding it; later steps still run; does not reset the camera |

For example, `{"type":"flow_percent","edge":0,"from_percent":25,"to_percent":75,"duration_ms":2000}`
plays the middle half of a link in two seconds. Percentages are route-length
percentages, including bends and loops: `0 <= from_percent <= to_percent <= 100`.
Scheduled flow markers disappear when their command finishes; use realtime
markers below when a marker must keep its position.

Defaults: `duration_ms:1000`, `color:[255,180,40]`, `pulse:false`,
`reverse:false`, `fit:false`, empty target arrays. A highlight needs at least
one target. `flow` follows stored source-to-target order irrespective of arrow
direction; `reverse:true` reverses travel, including for self-links.
Camera zoom is `0.15..3`; `fit` cannot be combined with `node` or `zoom`.
The latest due camera command takes over; completed camera movement keeps its
final position and releases manual pan/zoom even if other effects continue.

Limits: 1–256 steps, 1–60000 ms per non-clear command, total timeline at most
600000 ms, at most 1024 targets per highlight, 65536 bytes per request body,
8192 bytes of HTTP headers. Unknown JSON fields are rejected. Errors return
`{"error":"..."}`: `400` invalid scenario/request, `401` invalid/missing token,
`403` invalid Host/browser Origin, `404` unknown endpoint, `408` transport
timeout, `409` inactive/not-ready graph or stale graph ID, `413` oversized
request, `415` incorrect content type, `503` unavailable queue, `504` GUI
timeout. Expired queued commands are not executed later. The HTTP API accepts
HTTP/1.1 with content length, not chunked encoding or `Expect: 100-continue`.

**Python (standard library):**

```python
import getpass
import json
import urllib.request

base = input("API address shown in StructView: ").rstrip("/")
token = getpass.getpass("Bearer token: ")

def request(path, body=None):
    headers = {"Authorization": f"Bearer {token}"}
    data = None
    if body is not None:
        headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(base + path, data=data, headers=headers)
    with urllib.request.urlopen(req, timeout=5) as response:
        return json.load(response)

graph = request("/v1/graph")
if not graph["edges"]:
    raise RuntimeError("Open a graph containing at least one link")
edge = graph["edges"][0]
print(request("/v1/animation", {
    "graph_id": graph["graph_id"],
    "steps": [
        {"command": {"type": "highlight", "nodes": [edge["source"]], "pulse": True}},
        {"command": {"type": "camera", "node": edge["source"], "zoom": 1.5}},
        {"at_ms": 1000, "command": {"type": "flow", "edge": edge["index"], "duration_ms": 3000}},
    ],
}))
print(request("/v1/animation"))  # playback status
# To stop early, send an empty POST:
stop = urllib.request.Request(base + "/v1/animation/stop",
    method="POST", headers={"Authorization": f"Bearer {token}"})
# urllib.request.urlopen(stop, timeout=5).close()
```

**PowerShell:**

```powershell
$base = (Read-Host 'API address shown in StructView').TrimEnd('/')
$headers = @{ Authorization = 'Bearer ' + (Read-Host 'Bearer token') }
$graph = Invoke-RestMethod "$base/v1/graph" -Headers $headers
if (@($graph.nodes).Count -eq 0) { throw 'Open a graph with entities' }
$scenario = @{
    graph_id = $graph.graph_id
    steps = @(@{
        command = @{
            type = 'highlight'; nodes = @($graph.nodes[0].id)
            color = @(255, 180, 40); pulse = $true; duration_ms = 3000
        }
    })
} | ConvertTo-Json -Depth 8 -Compress
Invoke-RestMethod "$base/v1/animation" -Method Post -Headers $headers `
    -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($scenario))
Invoke-RestMethod "$base/v1/animation" -Headers $headers
# Stop early:
# Invoke-RestMethod "$base/v1/animation/stop" -Method Post -Headers $headers
```

#### Jobs: independent executions

Use `/v1/jobs` for application-level tasks instead of manually coordinating
marker and element IDs. The old scheduled and realtime APIs remain compatible
and independent. Each job has a stable ID, display label, current stage,
status, target nodes, an optional route, and stage-local progress.

Read `/v1/capabilities` to discover support and limits, then `/v1/graph` to
resolve node IDs and edge indexes. Submit a batch:

```json
{
  "graph_id": "COPY_FROM_GET_GRAPH",
  "commands": [
    {"type": "upsert", "id": "order-42", "label": "Order #42", "stage": "Capturing payment",
     "status": "running", "nodes": ["payment"], "edge": 0, "percent": 45,
     "color": [40, 160, 255], "smoothing_ms": 300},
    {"type": "upsert", "id": "risk-42", "label": "Risk review #42", "stage": "Checking payment",
     "status": "running", "nodes": ["payment"], "edge": 0, "percent": 30,
     "color": [255, 160, 40]}
  ]
}
```

- `upsert` is a **full snapshot**, not a partial patch. Required fields are
  `id`, `label`, `stage`, and `status`; status is `waiting`, `running`, `success`,
  or `error`. Supply `nodes` and/or an `edge`. Updating a job does not affect
  other jobs, even when their targets overlap.
- Optional fields: `nodes:[]`, `edge:null`, `percent:0`, `reverse:false`,
  `smoothing_ms:300`, and RGB `color`. Missing color keeps the job's existing
  color, or assigns a deterministic palette color to a new ID.
- For the same stage, edge, and direction, progress targets increase
  monotonically and marker position is preserved. Changing any of these starts
  a new stage-local marker from zero. Use a **new stage name** for retry attempts.
  Node-only jobs show the reported percentage in the job list.
- `GET /v1/jobs` returns each full snapshot, resolved `color`, the accepted
  target `percent`, and smoothed `current_percent`. Completed or failed jobs
  remain visible until explicitly removed. The list shows target progress;
  route labels and tooltips show current visible progress.
- To stop one job, send `{"type":"remove","id":"risk-42"}`. To clear only jobs,
  send `{"type":"clear"}` to `/v1/jobs`. Neither operation stops scheduled
  animation or clears legacy realtime effects. Unknown job IDs return `400`.
- Batches contain 1–64 commands, with at most 64 jobs in total. ID limit is
  128 UTF-8 bytes; label and stage must be nonblank and at most 256 bytes each.
  `percent` is `0..100`, smoothing is `16..10000` ms, and a job supports at most
  1024 node/link targets. Invalid batches are atomic: they leave all prior
  jobs unchanged. Stale graph IDs return `409`. Existing transport limits,
  authentication, and TLS apply.
- Graph invalidation/view changes, disabling the API, and global **Stop**
  remove jobs along with both older layers. Jobs are visual-only: they never
  edit the document and are not exported or persisted to disk.

#### Real-time markers and element states

Realtime updates run alongside a scenario; starting a new scenario or its
scheduled `clear` does not remove them. `GET /v1/animation` reports only the
scenario. `GET /v1/realtime` reports marker IDs, link indexes, current `percent`,
`target_percent`, color, direction, and smoothing, plus element-effect IDs,
node IDs, link indexes, status, color, and pulse.
Send updates to `POST /v1/realtime`:

```json
{
  "graph_id": "COPY_FROM_GET_GRAPH",
  "commands": [
    {"type": "marker", "id": "transfer-42", "edge": 0, "percent": 37.5, "smoothing_ms": 300, "color": [40, 200, 255], "label": "Order fulfillment", "stage": "Reserving stock"},
    {"type": "element", "id": "transfer-42", "nodes": ["a"], "edges": [0], "status": "running", "label": "Order fulfillment", "stage": "Reserving stock"},
    {"type": "marker", "id": "risk-42", "edge": 0, "percent": 62, "label": "Risk review", "stage": "Checking payment"}
  ]
}
```

- A marker starts at `0%` and smoothly approaches the highest percentage
  received for its ID. Updating `37.5 -> 60 -> 100` changes its target without
  resetting position or restarting motion. A delayed lower value never moves
  it backwards. Each ID is independent, so multiple jobs may share a link;
  each marker is lane-offset so concurrent executions remain visible.
- Optional `label` and `stage` metadata (up to 256 UTF-8 bytes each) is shown
  when hovering the affected link or entity. Reuse the same ID across a marker
  and element effect to associate their progress, status, and stage; different
  IDs remain independent. Omitted marker metadata preserves the existing value.
- `smoothing_ms` defaults to `300` and is the exponential smoothing time
  constant, **not a fixed travel duration**: about 63% of the remaining gap is
  covered per interval, about 95% after three intervals. The last gap below
  `0.01` percentage point snaps to the exact target. Updates may arrive faster
  than rendering; interpolation is based on elapsed time, not request count.
- `percent` is numeric `0..100`, `smoothing_ms` is `16..10000`, and optional
  `reverse:true` maps `0%` to the target end of the route. A marker remains
  visible at its final position until removed; no continuous repaint is needed
  once it settles.
- An existing marker cannot switch links or direction. Send
  `{"type":"remove_marker","id":"transfer-42"}` before reusing its ID, including
  when restarting progress from zero. Remove and recreate in the same batch
  for an atomic restart.
- `element` replaces the effect with the same ID. Supply `nodes` and/or `edges`.
  `status` may be `waiting` (gray), `running` (blue/pulsing), `success` (green),
  or `error` (red/pulsing). Nodes also display a localized status badge.
  Optional `label` and `stage` identify the execution and current phase in
  entity/link tooltips.
  Explicit `color:[r,g,b]` and `pulse:true/false` override status defaults.
  With no status, the default is a steady amber highlight.
- Send `{"type":"remove_element","id":"worker-state"}` to remove an effect, or
  `{"type":"clear"}` in `/v1/realtime` to clear **only** the realtime layer.
  Removing an unknown ID returns `400` rather than silently succeeding.
- Batches contain 1–256 commands and are atomic: invalid targets, stale graph
  IDs, malformed values, or resource-limit failures leave the visual state
  unchanged. At most 256 markers and element effects combined may exist,
  each visual ID is nonblank and at most 128 UTF-8 bytes; each element effect
  targets at most 1024 nodes/links. General body/header limits still apply.
- View changes, graph recalculation, disabling the server, and **Stop
  animation** clear both layers. Nothing is saved to the graph or exported.
  Concurrent clients are applied in GUI queue order; monotonic marker targets
  tolerate out-of-order percentages, while element states use the last applied
  update. A graph ID mismatch returns `409`.

**Python realtime producer**, using `request()` and `base` from the earlier
example (the same calls work with HTTP or HTTPS):

```python
import time

graph = request("/v1/graph")
if not graph["edges"]:
    raise RuntimeError("Open a graph containing at least one link")
edge = graph["edges"][0]

def send_progress(percent, status="running"):
    return request("/v1/realtime", {
        "graph_id": graph["graph_id"],
        "commands": [
            {"type": "marker", "id": "job-42", "edge": edge["index"],
             "percent": percent, "smoothing_ms": 300},
            {"type": "element", "id": "job-42-state",
             "nodes": [edge["target"]], "status": status},
        ],
    })

# In a real producer, call send_progress() from progress notifications.
for percent in (10, 30, 80, 45, 100):  # late 45 does not reverse progress
    send_progress(percent)
    time.sleep(0.03)
send_progress(100, "success")
print(request("/v1/realtime"))
# Clear live effects without stopping the scenario:
# request("/v1/realtime", {"graph_id": graph["graph_id"],
#                         "commands": [{"type": "clear"}]})
```

#### Network HTTP, built-in HTTPS, and reverse proxy

This is a server **inside the running GUI application**, not a headless daemon.
Leave the application and graph view open. Before enabling the server:

1. Set **Listen IP** to the machine's LAN IP, or `0.0.0.0`/`::` to listen on
   all IPv4/IPv6 interfaces. The safe default is `127.0.0.1`.
2. Select **Allow network access** for any non-loopback bind address.
3. Set a fixed port for other machines. If listening on all interfaces, fill
   **Public IP or DNS host** with the IP/name clients will use, **without**
   scheme, port, or path. For a concrete listen IP this is optional, but use it
   when clients connect by DNS. IPv6 URLs use brackets. The settings window
   displays the client URL and the actual listen socket separately.
4. Allow inbound traffic in the system firewall only for trusted clients.
   The application does not alter firewall rules or open router ports.

**HTTP:** leave HTTPS unchecked. Use it only on a trusted LAN, or keep the
listener on loopback behind a HTTPS reverse proxy. Token authentication is
required even on loopback, but plain HTTP does not encrypt it.

**Built-in HTTPS:** check **HTTPS (TLS)**, choose a PEM certificate chain
(leaf certificate first, then intermediates) and matching PEM private key.
The certificate's SAN must contain the DNS name/IP used by clients. Supported
private-key encodings are PKCS#8, RSA PKCS#1, and SEC1 as supported by rustls;
the key must be unencrypted. Invalid/missing/mismatched files prevent startup
and show an error. There is no fallback to HTTP. Protect the key with filesystem
permissions, never commit it, and keep it outside the repository. Certificates
are loaded at startup; disable/re-enable the server after renewal.
TLS 1.2/1.3 is supported; authentication still uses the API token, not client
certificates. Install a trusted certificate, or explicitly trust your internal
CA on clients rather than disabling verification.

For Python with an internal CA, add `import ssl`, create
`tls_context = ssl.create_default_context(cafile=r"C:\certs\ca.pem")`, and
pass `context=tls_context` to `urllib.request.urlopen`. With a publicly trusted
certificate the earlier example needs only an `https://` base URL.
PowerShell uses the operating system trust store; trust the internal CA there
and use the displayed HTTPS URL. Do not bypass certificate validation.

**HTTPS reverse proxy:** keep StructView at `127.0.0.1:8088` (or your chosen
fixed port) with HTTPS unchecked. Terminate TLS in your proxy, preserve
`Authorization`, restrict clients there, and rewrite `Host` to the internal
address/port expected by StructView. For example, an nginx location:

```nginx
location /v1/ {
    proxy_pass http://127.0.0.1:8088;
    proxy_set_header Host 127.0.0.1:8088;
    proxy_set_header Authorization $http_authorization;
    proxy_http_version 1.1;
    proxy_request_buffering on;
    proxy_set_header Expect "";
}
```

Configure HTTPS/certificates and an appropriate client allowlist on the proxy's
server block. Requests must still arrive with `Content-Length`; chunked bodies
are unsupported. Do not strip browser `Origin` to bypass the API's origin
restriction, and do not expose its token in frontend JavaScript.
The server accepts up to 8 simultaneous clients and bounds request parsing/
TLS-handshake time to 2 seconds, separately from the 2-second GUI reply deadline.
One slow client no longer blocks others. Excess HTTP clients receive `503`;
excess TLS clients are disconnected before handshake. Disabling joins client
workers and releases the listener; it may briefly wait for their I/O deadlines.

#### Compact graph animation demo (Python)

The standard-library controller [tests/demo_graph_animation.py](tests/demo_graph_animation.py)
demonstrates the compact, connected
[tests/fixtures/GraphAnimationDemo.toml](tests/fixtures/GraphAnimationDemo.toml):
8 entities and 9 relationships along a concise order/payment/delivery workflow.
It requires **Python 3.11+**, no pip packages. StructView recognizes the fixture's
`[[entities]]` and `[[relations]]` arrays as an explicit node/link graph.
See the [Russian](tests/README.md) or
[English](tests/README.en.md) quick-start guide for step-by-step launch
instructions.
For the larger 100-entity, 250-relationship layout demo, open
[tests/fixtures/Graph.toml](tests/fixtures/Graph.toml) and pass
`--graph tests\fixtures\Graph.toml`; regenerate that stress fixture with
`python tests\generate_shop_graph.py`.

1. Open the fixture in StructView, select the relationship graph, and wait for
   layout/routing to finish.
2. Enable the animation API and copy its URL and Bearer token.
3. From the repository root, run (replace the API address with the displayed one):

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088
```

Enter the token at the hidden prompt. Alternatively the script reads
`STRUCTVIEW_API_TOKEN` from its environment; do not put real tokens in source
files or command-line arguments. For HTTPS, use the HTTPS address and, if
needed, `--ca C:\certs\ca.pem`; certificate verification remains enabled.

The controller opens a local dashboard at `http://127.0.0.1:8765`. It stays
idle until you choose a mode: **Both**, **Scheduled**, or **Realtime**.
Use `--step-by-step` to wait for **Next chapter** at each checkpoint, or
`--autoplay` to start immediately with timed pauses.

For the first run, rebuild/restart StructView and use:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --step-by-step
```

Then open the printed dashboard URL and click **Both**. Checkpoints wait
between demonstrations; they do not freeze the scheduled player. Transient
scheduled effects finish before the checkpoint, while jobs remain available
for inspection.

- **Scheduled (chapters 1–9):** camera fit/focus/zoom, static and pulsing
  highlights, full flow, percentage segments, reverse flow, and scheduled clear.
  Each feature is isolated instead of animating every link at once.
- **Realtime/jobs (chapters 10–19):** one waiting job, smooth progress and a late
  lower update, a second job on the same route/node, changing stage/location,
  error, retry, individual stop, compatibility with legacy realtime, independent
  clearing, and state inspection. At most two jobs run at once.
- Every chapter includes an explanation in the dashboard and console. Keep
  StructView visible beside the dashboard to inspect badges, hover details,
  and the focusable job list.
- Dashboard buttons **Both**, **Scheduled**, **Realtime**, **Stop / clear**
  allow replay and cleanup. Status and errors appear in the page and console.
  Only one demonstration may run at once.
- **Camera:** fit the graph, focus a selected entity, or set zoom (0.15–3).
  Select an entity/link, color, duration, pulse and reverse direction.
- **Manual effects:** highlight/pulse, full flow, percentage segment, realtime
  percentage updates with configurable smoothing, all four element statuses
  with optional custom color/pulse, named execution ID/label/stage, removal of
  the named marker/element effect. Use a distinct ID for each concurrent run.
  Create/update/remove jobs and clear any layer independently, or stop all.
- **API inspection / JSON console:** read capabilities, jobs, and older animation
  states. Send arbitrary scheduled steps/timestamps or atomic realtime batches;
  `graph_id` is filled from the verified graph. API errors are displayed.
  Stop the automatic demonstration before manual mutations, so commands cannot
  accidentally replace its scheduled scenarios. Inspection remains available.

The script resolves link indexes from `/v1/graph`, verifies entity IDs and
relationships against the TOML fixture, and refuses a mismatched graph.
Starting a tour stops existing animations and jobs. A completed job remains
visible after the realtime tour for inspection; **Stop / clear** removes it.
At speed 1 the complete guided tour takes roughly 90 seconds plus request
overhead; manual checkpoints can be held as long as needed. Keep the graph
view active. Ctrl+C stops playback, clears all layers, and closes the dashboard.
The tour requires jobs support; it checks capabilities and reports an explicit
error if the running StructView API is too old.
The browser talks only to the local controller, which forwards authenticated
requests to StructView; the StructView token is never embedded in the page.

Options: `--mode scenario|realtime|both`, `--speed 0.1..10`, `--port 0` for an
available dashboard port, `--graph PATH` for a fixture in the same shop schema,
`--autoplay` for immediate timed playback, `--step-by-step` for manual
checkpoints, and `--once` to run without a dashboard and exit after completion
(cannot be combined with `--step-by-step`).

`--once` starts immediately and leaves the final completed job visible; unlike
Ctrl+C, normal completion does not send a global stop.

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --mode both --once --speed 2
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --step-by-step
python -B -m unittest discover -s tests -p test_demo_graph_animation.py -v
```

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

JSON5 `NaN`, `Infinity`, and numeric literals that overflow to infinity are
rejected explicitly, rather than silently converted to `null`.

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

Opening or creating another file prompts you to save, discard, or cancel when
there are unsaved changes, including a document hidden by comparison mode.
The same confirmation protects a modified structure-diagram source and closing
the native application window. If both the regular document and its independent
structure snapshot have edits, save each view separately before replacing the
document. A failed save or file read preserves the current data.
Graph imports cannot be overwritten by conversion or Save As, including when
the destination is a symbolic-link alias of the input.

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

On Windows, release builds retain redirected stdin, stdout, and stderr;
attaching to a console does not replace pipes or file redirection.

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

`new` creates an empty object and refuses to replace an existing file, even if
another process creates the destination while the command is running.
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

The CLI and GUI share editing, clipboard-envelope, and visualization logic from
`struct-view-operations`. The executable supplies the GUI's in-memory renderer
to the CLI for SVG/PNG exports; the CLI crate itself has no UI dependency and
does not initialize a native window.

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

Test the standard-library demo controller and dashboard separately:

```powershell
python -B -m unittest discover -s tests -p test_demo_graph_animation.py -v
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
- [`struct-view-operations`](crates/struct-view-operations/README.md) —
  shared editing, clipboard, and visualization operations;
- [`struct-view-routing`](crates/struct-view-routing/README.md) — reusable
  weighted-graph algorithms, orthogonal routing, and an adjacency-list graph;
- [`struct-view-animation-api`](crates/struct-view-animation-api/README.md) —
  authenticated HTTP/HTTPS transport for graph animation;
- [`struct-view-cli`](crates/struct-view-cli/README.md) — command-line parsing
  and headless commands;
- [`struct-view-ui`](crates/struct-view-ui/README.md) — native GUI, views,
  editor windows, graph rendering, and animation/job state;
- [`struct-view-build-info`](crates/struct-view-build-info/README.md) — shared
  compile-time build metadata.

The executable composes the CLI and GUI crates; both share `struct-view-core`
and `struct-view-operations`, without depending on each other. The GUI integrates
graph routing and the animation HTTP server. The
`struct-view-animation-api` crate is framework-independent: an embedding
application provides an optional wake callback and handles domain-specific
requests itself.
Shared editing, clipboard envelopes, graph documents, and table/schema/graph
models belong to `struct-view-operations`, not to the UI. Graph image rendering
remains in the UI; the executable injects its adapter into the CLI through
`GraphImageRenderer`. Thus window-free CLI commands do not depend on the UI
crate, even though the desktop executable composes both.

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
