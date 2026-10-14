# struct-view-cli

Command-line argument parsing and headless operations for StructView.

The crate parses arguments into a `Command` and executes commands that do not
require starting the GUI. It also describes GUI launch requests so the
application's entry point can open one file or start a multi-file comparison.

## Public API

The `cli` module exports:

- `parse_args` and `Command` for parsing invocations;
- `run` for executing parsed headless commands;
- `run_with_graph_image_renderer` for hosts that provide graph image rendering;
- `Source` for file and standard-input inputs;
- `HELP` for the command-line help text.

Available operations include formatting and converting documents, validating,
searching, comparing, editing, and exporting data. Run `struct_view --help`
for the supported commands and options.

## Crate structure

```text
struct-view-cli/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── cli/
        ├── mod.rs             public CLI API and help text
        ├── args.rs            command types and argument parsing
        ├── source.rs          file and stdin input abstraction
        ├── exec.rs            dispatch and execution of commands
        ├── operations.rs      operation module entry point
        └── operations/
            ├── parse.rs       document loading and operation parsing
            │   └── parse/
            │       └── finalize.rs
            └── run.rs         operation execution
                └── run/
                    └── progress.rs
```

This crate uses `struct-view-core` and `struct-view-operations`; it does not
depend on the GUI crate. The executable provides a graph image renderer through
`run_with_graph_image_renderer`, keeping rendering in the GUI layer while
allowing CLI commands to reuse it. See the
[main README](../../README.md) for installation, usage, and the full feature
list.
