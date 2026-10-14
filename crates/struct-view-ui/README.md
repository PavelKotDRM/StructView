# struct-view-ui

Native graphical interface and clipboard support for StructView, implemented
with `eframe` and `egui`.

The crate provides the application state and views for browsing and editing
structured documents, comparing files, and visualizing tables, schemas, and
graphs. It also handles localization, themes, file dialogs, and clipboard
integration.

## Public API

- `run_native_gui(files)` starts the native application, optionally opening
  the supplied files;
- `app` exposes the GUI application modules;
- `app::graph_animation::{AnimationCommand, AnimationStep, AnimationScenario}`
  exposes serializable visual-animation protocol types;
- `app::graph_animation::{RealtimeCommand, RealtimeUpdate, ElementStatus}`
  exposes persistent marker and element-state protocol types;
- `app::graph_animation::{JobCommand, JobUpdate}` exposes independent
  job snapshots and atomic lifecycle commands;
- `struct-view-animation-api` supplies the bounded authenticated HTTP/HTTPS
  transport consumed by graph animation;
- `clipboard` provides clipboard support.

## Crate structure

```text
struct-view-ui/
├── Cargo.toml
└── src/
    ├── lib.rs                 GUI startup API
    ├── clipboard.rs           clipboard integration
    └── app/
        ├── mod.rs             application composition
        ├── state.rs           application and document state
        ├── docking.rs         panel layout
        ├── edit.rs            shared editing facade
        ├── graph_animation.rs animation module facade
        ├── graph_animation/   animation API, playback, realtime/jobs state, tests
        │   ├── api/           settings, window, request dispatch, graph descriptor
        │   ├── player.rs      scheduled effects and camera playback
        │   ├── realtime.rs    low-level persistent markers and element effects
        │   ├── jobs.rs        independent job snapshots and lifecycle
        │   └── jobs/          job list UI and job lifecycle tests
        ├── graph_editor.rs    semantic graph editor
        ├── graph_editor/      graph editor tests and UI
        │   └── ui/            editor state, window, dialog/forms
        ├── tree.rs            document tree rendering
        ├── views.rs           view selection and shared view behavior
        ├── visualization.rs   shared data-model facade
        ├── widgets.rs         reusable UI widgets
        ├── theme.rs           theme support
        ├── i18n.rs            localization
        ├── panels/            top, central, and bottom panels
        ├── state/             dialogs, files, editing, history, and saving
        ├── tree/              tree model, rendering, and context menu
        ├── views/             graph, structure, table, schema, diff, diagram
        │   ├── graph/         layout, routing adapter, labels, canvas, and export
        │   │   ├── canvas/    interaction, rendering, and tooltips
        │   │   └── routing/   egui geometry adapter, route workers, and tests
        │   └── structure/     layout, navigation, search, source, and export
```

The GUI uses `struct-view-operations` for the document editing operations,
clipboard envelope, and graph/table/schema models shared with the CLI. Its
headless module adapts the GUI graph-image renderer to the CLI's renderer
interface. The GUI also uses `struct-view-core` for document parsing,
`struct-view-routing` for weighted pathfinding and obstacle-aware orthogonal
routing, and
`struct-view-build-info` for build metadata. The
[main README](../../README.md) documents the interface and its features.

## Relationship graph editing and animation

The relationship view also provides a semantic graph editor. It creates
untitled graphs or independent editable copies of any supported graph, with
entity/link/group CRUD, directions, weights, typed attributes, and shared
document undo/redo and saving. `struct-view-operations::app::graph_editor`
owns the canonical editable model; `app/graph_editor` provides the GUI
integration and UI, including a dockable editor window using the same docking
helpers as search. Creation/copy commands live in File, mutation commands in
Edit, and the editor-window toggle in View; graph-specific commands are
hidden in unrelated views. `graph.editor_version: 1` distinguishes its mixed-multigraph
documents. Node positions and routes are never editable and continue to use
the layout and routing pipeline below. See the
[graph editor guide](../../README.md#relationship-graph-editor).

Element forms show a live read-only complete JSON record; `id` and `label`
remain separate from additional `attributes`. The optional
`app/graph_animation` subsystem accepts authenticated HTTP/HTTPS scenarios
and realtime batches,
validates targets against the completed graph calculation, and stores visual
playback in the egui context. The GUI thread handles requests; the canvas
paints highlights and arc-length flow markers over existing routes and smoothly
updates the camera. Its independent realtime layer tracks percentage targets
monotonically with elapsed-time exponential smoothing and paints persistent
status highlights and non-overlapping badges. Multiple realtime execution IDs
can share entities and links; markers on a shared route are lane-offset.
Hovering an entity or link shows each execution's optional label and stage with
its status, while link tooltips also include marker progress and target.
Realtime batches
validate atomically before replacing state. Commands never mutate document data
or layout. The jobs API adds independent labeled task snapshots with stage-local
progress, route changes, retries, and individual removal. Job state is separate
from legacy realtime effects; the canvas merges copies for display only.
Node badges and labeled markers identify work without hovering, and the
expandable job list can focus a task's location. `GET /v1/capabilities` exposes
supported features and limits. The `struct-view-animation-api` crate owns the bounded concurrent
HTTP/HTTPS server; it defaults to loopback, requires explicit network opt-in,
supports configurable listen IP/public host and PEM-backed rustls TLS, and
stops all workers when disabled or the application closes. Its settings use a
detachable window. See the
[animation API guide](../../README.md#graph-animation-api) for endpoints,
limits, lifecycle, and Python/PowerShell examples.

## Relationship graph routing

The relationship graph draws each edge as a routed path between node borders.
Routing is performed after node layout and follows these steps:

1. **Select ports.** Edges between two layout columns leave and enter through
   the left and right sides that face each other, so relationships flow left to
   right. Edges within one column use the top and bottom sides that face each
   other. A node with more than seven incident edges uses, for each of its edges,
   the side that geometrically faces the other endpoint, because a left or right
   side only has room for about seven lanes. Edges sharing a node side are sorted
   by the position of their opposite endpoint and assigned separate, evenly
   spaced port offsets. Cards with more than 22 incident edges are drawn as big
   cards: they grow wider, about 8 points per edge, so each edge keeps its own
   8-point lane on a top or bottom side, and they show their incoming and
   outgoing link counts and the node's document path below the label. Detoured
   links receive a short lead-out past the expanded node boundary. Self-links use
   the right and bottom sides.
2. **Keep aligned ports straight.** A straight line is retained when the two ports line up,
   it clears every other node's expanded obstacle, and it does not conflict with
   an already routed edge. Obstacles extend 18 points beyond the node border.
3. **Find an orthogonal detour when needed.** The UI adapts node positions to
   the geometry types in `struct-view-routing`; its reusable orthogonal router
   builds a coordinate grid from obstacle boundaries, node centers, edge ports,
   the midline between each pair of exits, and nearby tracks from previously
   routed edges. Indexed A* searches this coordinate grid using path length plus
   48 points per bend, including turns at the destination port. Among equal
   routes it prefers the track nearest the midline, so channels between columns
   are centered and parallel links bundle symmetrically around it. Candidate
   tracks are spaced 10 points from existing route vertices, with at most 128
   nearby candidate coordinates retained on each axis. If the initial grid has
   no path, intermediate tracks are added.
4. **Keep parallel routes apart.** Shared collinear sections are forbidden.
   Parallel segments use the configured 8-point clearance outside endpoint
   lead zones. If dense paths remain blocked after intermediate tracks are
   added, fallback searches progressively reduce the clearance, while exact
   shared sections remain forbidden. Perpendicular crossings remain allowed so
   a crossing alone does not create a long detour. If no path remains, routing
   reports a crowded edge and the layout widens nearby gaps before retrying.
5. **Resolve edge conflicts.** Initial candidates are routed in parallel, one per
   edge, on every logical CPU in automatic mode. Conflict resolution then runs
   on one thread in edge order: an edge whose candidate conflicts with an
   earlier accepted route is rerouted against the routes accepted so far. The
   routes are the same for any number of workers. Perpendicular crossings are
   not treated as lane conflicts.
6. **Straighten clean links.** Once the orthogonal routes are final, a link whose
   ports can be joined by one straight segment is drawn as that segment: it needs
   no bends and is the shortest route. The segment must be at most two layout
   columns long (680 points), clear every other node's obstacle (18 points beyond
   the node border), and stay 8 points or more from the other links' corners and
   ends. It may cross at most one other link, and a straight link that already has
   a crossing may not gain another. Because this runs after conflict resolution,
   it never changes the routes of other links.
7. **Simplify the result.** Duplicate points, closed detours, and redundant
   collinear points—including collinear backtracking—are removed before the
   path is drawn. An intentionally closed route keeps its final endpoint.

Only nodes incident to at least one edge are routing obstacles. Isolated nodes
are laid out separately and do not stretch the connected graph's routes or
labels. If fallback relationship labels would make a vertical legend taller
than the connected graph, they are packed into multiple rows beside the graph;
isolated nodes are shifted right as needed to leave the legend clear.
