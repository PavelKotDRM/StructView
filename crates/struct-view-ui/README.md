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

1. **Select ports.** Each edge uses the side of its source and target nodes
   that faces the other endpoint. Edges sharing a node side are sorted by the
   position of their opposite endpoint and assigned separate, evenly spaced
   port offsets. Detoured links receive a short lead-out past the expanded node
   boundary. Self-links use the right and bottom sides.
2. **Try a direct segment.** A straight line is retained when it clears every
   other node's expanded obstacle and does not conflict with an already routed
   edge. Obstacles extend 18 points beyond the node border.
3. **Find an orthogonal detour when needed.** The UI adapts node positions to
   the geometry types in `struct-view-routing`; its reusable orthogonal router
   builds a coordinate grid from obstacle boundaries, node centers, edge ports,
   and nearby tracks from previously routed edges. It searches the grid with
   indexed A*, using Manhattan distance, path length, and turn cost. Candidate
   tracks are spaced
   10 points from existing route vertices, with at most 128 nearby candidate
   coordinates retained on each axis. The search starts near the endpoints and
   expands vertically only as needed; it uses the full graph bounds only when
   no local path is available.
4. **Prefer clear, coherent paths.** Segments near existing routes receive a
   penalty (2,000 points within the 8-point edge clearance, or 50,000 points
   for crossings). Turns cost 48 points. A softer 240-point penalty discourages
   detours below an intersected obstacle for horizontal routes and to its right
   for vertical routes, encouraging routes to pass a shared obstacle on the
   same side. Collinear overlap penalties grow with shared length and are capped
   at 5,000,000 points, so a local crossing can be preferable to a long merged
   track. Remaining shared sections are locally detoured when a clear lane is
   available; crossings can remain in dense layouts.
5. **Resolve edge conflicts.** With multiple workers, initial candidates are
   routed independently. A final deterministic pass checks them in edge order
   and reroutes any path that conflicts with an earlier route. The single-worker
   path uses this same edge order and incrementally considers accepted routes.
6. **Simplify the result.** Consecutive duplicate points and redundant
   collinear points that continue in the same direction are removed before the
   path is drawn. Collinear reversals are preserved because they can encode a
   required detour.

Only nodes incident to at least one edge are routing obstacles. Isolated nodes
are laid out separately and do not stretch the connected graph's routes or
labels. If fallback relationship labels would make a vertical legend taller
than the connected graph, they are packed into multiple rows beside the graph;
isolated nodes are shifted right as needed to leave the legend clear.
