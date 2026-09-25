//! Скрипт сборки: генерирует переменные окружения `VERGEN_*`,
//! которые читает модуль `build_info`.

use std::error::Error;

use vergen::{Build, Cargo, Emitter, Rustc};

fn main() -> Result<(), Box<dyn Error>> {
    let build = Build::builder().build_timestamp(true).build();
    let cargo = Cargo::builder()
        .target_triple(true)
        .opt_level(true)
        .debug(true)
        .build();
    let rustc = Rustc::builder()
        .semver(true)
        .channel(true)
        .host_triple(true)
        .build();

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&cargo)?
        .add_instructions(&rustc)?
        .emit()?;

    Ok(())
}
