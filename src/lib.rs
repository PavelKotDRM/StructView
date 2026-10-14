//! # StructView
//!
//! Cross-platform desktop application and command-line tools for structured
//! JSON, YAML, TOML, and JSON5 data.
//!
//! This crate is the compatibility facade and executable entry point.
//! Functionality is implemented by the workspace crates:
//!
//! | Crate | Responsibility |
//! |-------|----------------|
//! | `struct-view-core` | Parsing, document trees, search, and comparison |
//! | `struct-view-operations` | Shared editing, clipboard, graph documents, and visualization models |
//! | `struct-view-routing` | Graph algorithms and orthogonal routing |
//! | `struct-view-animation-api` | Framework-independent authenticated HTTP/HTTPS transport |
//! | `struct-view-cli` | Command-line parsing and headless commands |
//! | `struct-view-ui` | Native GUI, editor windows, graph rendering, and animation/job state |
//! | `struct-view-build-info` | Shared compile-time build metadata |
//!
//! The executable composes CLI and GUI; neither frontend depends on the other.
//! Both use shared core/operations models. The GUI supplies graph image rendering
//! through the renderer interface, while the transport crate delegates graph
//! validation and animation state to its embedding application.

#![deny(warnings)]

pub mod console;

pub use struct_view_build_info as build_info;
pub use struct_view_cli::cli;
pub use struct_view_core::{diff, files, parser, search};
pub use struct_view_ui::{app, clipboard, run_native_gui};
