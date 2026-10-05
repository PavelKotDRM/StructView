# struct-view-build-info

Shared compile-time version and build metadata for StructView crates.

The build script uses `vergen` to capture the build timestamp, Cargo target,
optimization level, debug mode, and Rust compiler details. The library exposes
these values as constants and provides `detailed()` for the multiline
information shown by the application's `--version` command.

## Public API

- `VERSION` — package version;
- `BUILD_TIMESTAMP` — build time in RFC 3339 format;
- `TARGET_TRIPLE` and `HOST_TRIPLE` — target and build platforms;
- `OPT_LEVEL` and `DEBUG` — build configuration;
- `RUSTC_SEMVER` and `RUSTC_CHANNEL` — Rust compiler information;
- `detailed()` — formatted build information.

If a generated metadata value is unavailable, its constant is `"unknown"`.

## Crate structure

```text
struct-view-build-info/
├── Cargo.toml   package metadata and vergen build dependency
├── build.rs     emits compile-time build and compiler metadata
└── src/
    └── lib.rs   metadata constants and detailed()
```

This crate is used by the CLI and UI crates. It is part of the
[StructView workspace](../../README.md).
