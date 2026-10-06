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
        │   ├── graph/         layout, routing, labels, canvas, and export
        │   └── structure/     layout, navigation, search, source, and export
        ├── visualization/     table, schema, and graph view models
        └── edit/              field, path, and structure editing helpers
```

The GUI uses `struct-view-core` for document parsing and shared operations,
and `struct-view-build-info` for build metadata. The
[main README](../../README.md) documents the interface and its features.

## Relationship graph routing

The relationship graph draws each edge as a routed path between node borders.
Routing is performed after node layout and follows these steps:

1. **Select ports.** Each edge uses the side of its source and target nodes
   that faces the other endpoint. Edges sharing a node side are sorted by the
   position of their opposite endpoint and assigned separate, evenly spaced
   port offsets. Self-links use the right and bottom sides.
2. **Try a direct segment.** A straight line is retained when it clears every
   other node's expanded obstacle and does not conflict with an already routed
   edge. Obstacles extend 18 points beyond the node border.
3. **Find an orthogonal detour when needed.** The router builds a coordinate
   grid from obstacle boundaries, node centers, edge ports, and nearby tracks
   from previously routed edges. It searches this grid with A*, using
   Manhattan distance, path length, and turn cost. Candidate tracks are spaced
   10 points from existing route vertices, with at most 12 nearby candidate
   coordinates retained on each axis.
4. **Prefer clear, coherent paths.** Segments near existing routes receive a
   penalty (2,000 points within the 8-point edge clearance, or 50,000 points
   for crossings). Turns cost 48 points. A softer 240-point penalty discourages
   detours below an intersected obstacle for horizontal routes and to its right
   for vertical routes, encouraging routes to pass a shared obstacle on the
   same side. These are preferences, not hard constraints; avoiding nodes and
   keeping routes apart takes priority.
5. **Resolve edge conflicts.** With multiple workers, initial candidates are
   routed independently. A final deterministic pass checks them in edge order
   and reroutes any path that conflicts with an earlier route. The single-worker
   path uses this same edge order and incrementally considers accepted routes.
6. **Simplify the result.** Consecutive duplicate points and redundant
   collinear points are removed before the path is drawn.

Only nodes incident to at least one edge are routing obstacles. Isolated nodes
are laid out separately and do not stretch the connected graph's routes or
labels.
