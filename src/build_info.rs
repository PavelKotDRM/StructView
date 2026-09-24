//! # Информация о сборке
//!
//! Значения подставляются на этапе компиляции скриптом `build.rs`
//! через `vergen`. If a variable was not generated, for example when
//! building from a source archive without all metadata, `unknown` is used.

/// Возвращает значение переменной окружения времени компиляции или заглушку.
macro_rules! env_or_unknown {
    ($name:literal) => {
        match option_env!($name) {
            Some(value) => value,
            None => "unknown",
        }
    };
}

/// Версия пакета из `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Время сборки в формате RFC 3339 (UTC).
pub const BUILD_TIMESTAMP: &str = env_or_unknown!("VERGEN_BUILD_TIMESTAMP");

/// Целевая платформа, для которой собран бинарник.
pub const TARGET_TRIPLE: &str = env_or_unknown!("VERGEN_CARGO_TARGET_TRIPLE");

/// Уровень оптимизации (`opt-level`).
pub const OPT_LEVEL: &str = env_or_unknown!("VERGEN_CARGO_OPT_LEVEL");

/// Признак отладочной сборки (`true`/`false`).
pub const DEBUG: &str = env_or_unknown!("VERGEN_CARGO_DEBUG");

/// Версия компилятора Rust.
pub const RUSTC_SEMVER: &str = env_or_unknown!("VERGEN_RUSTC_SEMVER");

/// Канал компилятора Rust (`stable`, `beta`, `nightly`).
pub const RUSTC_CHANNEL: &str = env_or_unknown!("VERGEN_RUSTC_CHANNEL");

/// Платформа, на которой выполнялась сборка.
pub const HOST_TRIPLE: &str = env_or_unknown!("VERGEN_RUSTC_HOST_TRIPLE");

/// Подробная информация о сборке для вывода по `--version`.
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
