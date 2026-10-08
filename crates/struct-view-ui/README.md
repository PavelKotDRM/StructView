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
        ├── edit.rs            edit actions
        ├── tree.rs            document tree rendering
        ├── views.rs           view selection and shared view behavior
        ├── visualization.rs   data-to-view models
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
        ├── visualization/     table, schema, and graph view models
        └── edit/              field, path, and structure editing helpers
```

The GUI uses `struct-view-core` for document parsing and shared operations,
`struct-view-routing` for weighted pathfinding and obstacle-aware orthogonal
routing, and
`struct-view-build-info` for build metadata. The
[main README](../../README.md) documents the interface and its features.

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
