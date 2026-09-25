//! Build-time version and platform metadata shared by StructView crates.

/// Returns a compile-time environment variable, or `unknown` if it was not set.
macro_rules! env_or_unknown {
    ($name:literal) => {
        match option_env!($name) {
            Some(value) => value,
            None => "unknown",
        }
    };
}

/// Package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build timestamp in RFC 3339 (UTC) format.
pub const BUILD_TIMESTAMP: &str = env_or_unknown!("VERGEN_BUILD_TIMESTAMP");

/// Target platform for which the binary was built.
pub const TARGET_TRIPLE: &str = env_or_unknown!("VERGEN_CARGO_TARGET_TRIPLE");

/// Build optimization level.
pub const OPT_LEVEL: &str = env_or_unknown!("VERGEN_CARGO_OPT_LEVEL");

/// Whether this is a debug build.
pub const DEBUG: &str = env_or_unknown!("VERGEN_CARGO_DEBUG");

/// Rust compiler version.
pub const RUSTC_SEMVER: &str = env_or_unknown!("VERGEN_RUSTC_SEMVER");

/// Rust compiler channel.
pub const RUSTC_CHANNEL: &str = env_or_unknown!("VERGEN_RUSTC_CHANNEL");

/// Platform on which the build was performed.
pub const HOST_TRIPLE: &str = env_or_unknown!("VERGEN_RUSTC_HOST_TRIPLE");

/// Detailed build information for the `--version` command.
#[must_use]
pub fn detailed() -> String {
    format!(
        "StructView {VERSION}\n\
         \n\
         Build time:          {BUILD_TIMESTAMP}\n\
         Target platform:     {TARGET_TRIPLE}\n\
         Build platform:      {HOST_TRIPLE}\n\
         Optimization level:  {OPT_LEVEL}\n\
         Debug build:         {DEBUG}\n\
         rustc compiler:      {RUSTC_SEMVER} ({RUSTC_CHANNEL})\n"
    )
}

#[cfg(test)]
mod tests {
    use super::detailed;

    #[test]
    fn detailed_output_uses_english_labels() {
        let output = detailed();

        assert!(output.contains("Build time:"));
        assert!(output.contains("Target platform:"));
        assert!(!output.contains("Время сборки"));
    }
}
