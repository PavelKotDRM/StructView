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
//! | `struct-view-cli` | Command-line parsing and headless commands |
//! | `struct-view-ui` | Native GUI, views, editing, and clipboard |
//! | `struct-view-build-info` | Shared compile-time build metadata |

#![deny(warnings)]

pub mod console;

pub use struct_view_build_info as build_info;
pub use struct_view_cli::cli;
pub use struct_view_core::{diff, parser, search};
pub use struct_view_ui::{app, clipboard, run_native_gui};
