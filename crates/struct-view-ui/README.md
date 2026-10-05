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
