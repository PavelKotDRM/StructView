//! # Информация о сборке
//!
//! Значения подставляются на этапе компиляции скриптом `build.rs`
//! через `vergen`. Если переменная не была сгенерирована (например,
//! сборка из тарбола без части метаданных), используется `неизвестно`.

/// Возвращает значение переменной окружения времени компиляции или заглушку.
macro_rules! env_or_unknown {
    ($name:literal) => {
        match option_env!($name) {
            Some(value) => value,
            None => "неизвестно",
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
        "json_viewer {VERSION}\n\
         \n\
         Время сборки:        {BUILD_TIMESTAMP}\n\
         Целевая платформа:   {TARGET_TRIPLE}\n\
         Платформа сборки:    {HOST_TRIPLE}\n\
         Уровень оптимизации: {OPT_LEVEL}\n\
         Отладочная сборка:   {DEBUG}\n\
         Компилятор rustc:    {RUSTC_SEMVER} ({RUSTC_CHANNEL})\n"
    )
}
