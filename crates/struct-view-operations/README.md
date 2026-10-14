# struct-view-operations

Shared document operations and data models used by the command-line and GUI
crates. This crate does not depend on either frontend.

## Responsibilities

- tree editing, path lookup, multi-selection delete/copy/paste;
- the versioned structured clipboard envelope and system clipboard access;
- table, schema, and relationship graph models and exports;
- the window-free API consumed by CLI commands;
- the graph image renderer interface used by the CLI host to inject a backend.

The GUI owns the graph image implementation and adapts it through
`GraphImageRenderer`. The executable connects that adapter to the CLI. This
keeps native rendering details out of the CLI dependency graph while both
frontends share the same document semantics.

## Crate structure

```text
struct-view-operations/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── clipboard.rs       shared clipboard envelope and system integration
    └── app/
        ├── edit.rs        tree editing operations
        ├── headless.rs    shared operations and renderer interface
        ├── graph_editor/  graph document model
        └── visualization/ table, schema, and relationship graph models
```

See the [main README](../../README.md) for the workspace dependency overview.
