# struct-view-core

Format-aware data structures and operations shared by the StructView
application and command-line interface.

## Responsibilities

- Parse and serialize JSON, YAML, TOML, and JSON5 documents;
- import Graphviz DOT, GraphML, and GEXF graphs as normalized data;
- represent parsed documents as editable trees;
- search keys, values, and paths with literal or regular-expression queries;
- compare structured documents and report changes;
- provide shared file, graph, and structure operations.

## Public modules

- `parser` — formats, tree nodes, parsing, and serialization;
- `files` — document file operations;
- `search` — search options, state, and matching;
- `diff` — structured document comparison;
- `graph` — shared graph data and edge-direction semantics;
- `structure` — data-structure diagram support.

## Crate structure

```text
struct-view-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── diff.rs                 document comparison
    ├── files.rs                file operations
    ├── graph.rs                graph primitives and edge directions
    ├── numbers.rs              shared numeric helpers
    ├── search.rs
    │   └── matcher.rs          query matching
    ├── structure.rs
    │   └── yaml.rs             YAML structure support
    └── parser/
        ├── mod.rs              public parsing API
        ├── node.rs             document tree nodes and parse errors
        ├── build.rs            parser module declarations
        └── build/
            ├── format.rs       format detection
            ├── parse.rs        document parsing
            ├── tree.rs         tree construction
            ├── serialize.rs    serialization
            │   └── serialize/
            │       └── value.rs
            ├── comments.rs     comment preservation
            │   └── comments/
            │       ├── hash.rs
            │       └── json5.rs
            ├── json5.rs
            ├── paths.rs        path utilities
            ├── special_graphs.rs graph importer module declarations
            ├── special_graphs/
            │   ├── dot.rs      Graphviz DOT import
            │   └── xml.rs      GraphML and GEXF import
            │       └── xml/
            │           ├── gexf.rs
            │           ├── graphml.rs
            │           └── values.rs
            └── tests.rs
```

The CLI and GUI depend on this crate so they share parsing and data behavior.
See the [main README](../../README.md) for supported formats and application
usage.
