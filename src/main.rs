//! Точка входа приложения StructView.
//!
//! Разбирает аргументы командной строки: headless-команды выполняются через
//! [`struct_view::cli`], иначе GUI-крейт запускает окно с
//! [`struct_view::app::StructViewApp`].
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use struct_view::cli::{self, Command};
use struct_view::console::attach_parent_console;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Любой аргумент означает возможный CLI-режим: нужен доступ к консоли родителя.
    if !args.is_empty() {
        attach_parent_console();
    }

    let command = match cli::parse_args(args) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("Error: {}", err);
            eprintln!("Hint: struct_view --help");
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Gui { file } => run_gui(file.into_iter().collect()),
        Command::GuiCompare { files } => run_gui(files),
        other => match cli::run(&other) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(err) => {
                eprintln!("Error: {}", err);
                ExitCode::FAILURE
            }
        },
    }
}

/// Запустить графический интерфейс, опционально открыв указанный файл.
fn run_gui(files: Vec<PathBuf>) -> ExitCode {
    if !display_available() {
        eprintln!(
            "Error: no graphical server found (DISPLAY and WAYLAND_DISPLAY are unset or empty)."
        );
        eprintln!("Command-line mode is available: struct_view --help");
        return ExitCode::FAILURE;
    }

    match struct_view::run_native_gui(files) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error starting GUI: {}", err);
            ExitCode::FAILURE
        }
    }
}

/// Проверить наличие графического сервера X11 или Wayland.
///
/// На Linux и BSD запуск GUI без дисплея приводит к неинформативной ошибке
/// драйвера, поэтому окружение проверяется заранее.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn display_available() -> bool {
    display_variable_set(std::env::var_os("WAYLAND_DISPLAY"))
        || display_variable_set(std::env::var_os("DISPLAY"))
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn display_variable_set(value: Option<std::ffi::OsString>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

/// На платформах с гарантированным оконным менеджером проверка не требуется.
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
fn display_available() -> bool {
    true
}

#[cfg(all(test, unix, not(any(target_os = "macos", target_os = "android"))))]
mod tests {
    use super::display_variable_set;

    #[test]
    fn empty_display_variable_does_not_claim_a_graphical_server() {
        assert!(!display_variable_set(None));
        assert!(!display_variable_set(Some("".into())));
        assert!(display_variable_set(Some(":0".into())));
    }
}
