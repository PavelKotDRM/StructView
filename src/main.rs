//! Точка входа приложения JSON Viewer.
//!
//! Разбирает аргументы командной строки: headless-команды выполняются через
//! [`json_viewer::cli`], иначе инициализируется [`eframe`] окно и запускается
//! [`json_viewer::app::JsonViewerApp`].
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use eframe::NativeOptions;
use json_viewer::cli::{self, Command};
use json_viewer::console::attach_parent_console;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Любой аргумент означает возможный CLI-режим: нужен доступ к консоли родителя.
    if !args.is_empty() {
        attach_parent_console();
    }

    let command = match cli::parse_args(args) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("Ошибка: {}", err);
            eprintln!("Подсказка: json_viewer --help");
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Gui { file } => run_gui(file),
        other => match cli::run(&other) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(err) => {
                eprintln!("Ошибка: {}", err);
                ExitCode::FAILURE
            }
        },
    }
}

/// Запустить графический интерфейс, опционально открыв указанный файл.
fn run_gui(file: Option<PathBuf>) -> ExitCode {
    if !display_available() {
        eprintln!("Ошибка: графический сервер не найден (DISPLAY и WAYLAND_DISPLAY не заданы).");
        eprintln!("Доступен режим командной строки: json_viewer --help");
        return ExitCode::FAILURE;
    }

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("JSON Viewer")
            .with_inner_size([1024.0, 720.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    let result = eframe::run_native(
        "JSON Viewer",
        options,
        Box::new(move |cc| {
            Ok(Box::new(json_viewer::app::JsonViewerApp::new_with_file(
                cc, file,
            )))
        }),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Ошибка запуска GUI: {}", err);
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
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// На платформах с гарантированным оконным менеджером проверка не требуется.
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
fn display_available() -> bool {
    true
}
