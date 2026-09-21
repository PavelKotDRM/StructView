//! # Модуль командной строки
//!
//! Разбирает аргументы командной строки и выполняет headless-команды
//! (без запуска GUI): форматирование, валидацию и поиск по данным.
//!
//! ## Поддерживаемые вызовы
//!
//! ```text
//! json_viewer                      # открыть GUI
//! json_viewer data.json            # открыть GUI с файлом
//! json_viewer format data.json     # pretty-print в stdout
//! json_viewer validate data.json   # проверить синтаксис
//! json_viewer find name data.json  # вывести пути совпадений
//! ```
//!
//! ## Состав подмодулей
//!
//! | Подмодуль | Назначение |
//! |-----------|-----------|
//! | `args` | Тип [`Command`] и разбор аргументов ([`parse_args`]) |
//! | `source` | Источник данных [`Source`]: файл или stdin |
//! | `exec` | Выполнение разобранной команды ([`run`]) |

mod args;
mod exec;
mod source;

pub use args::{Command, parse_args};
pub use exec::run;
pub use source::Source;

/// Текст справки, выводимый по `--help`.
pub const HELP: &str = concat!(
    "JSON Viewer ",
    env!("CARGO_PKG_VERSION"),
    " — view and process JSON, YAML, TOML, and JSON5\n",
    "\n",
    "USAGE:\n",
    "    json_viewer [FILE]                      start the GUI (optionally with a file)\n",
    "    json_viewer format [FILE] [OPTIONS]     format data\n",
    "    json_viewer validate [FILE]             validate syntax\n",
    "    json_viewer find [OPTIONS] <QUERY> [FILE] find matching node paths\n",
    "\n",
    "format OPTIONS:\n",
    "    -o, --output <FILE>    write the result to a file instead of stdout\n",
    "    -m, --minify           print a compact representation\n",
    "\n",
    "find OPTIONS:\n",
    "    --keys                 search keys only\n",
    "    --values               search values only\n",
    "    --case-sensitive       match letter case\n",
    "    --exact                require an exact match\n",
    "\n",
    "COMMON OPTIONS:\n",
    "    -h, --help             show this help\n",
    "    -V, --version          show the version and build information\n",
    "\n",
    "The file format is detected from the extension; stdin is detected from its content.\n",
    "Supported extensions: .json, .yaml, .yml, .toml, and .json5.\n",
    "Use `-` instead of FILE to read data from stdin.\n",
    "\n",
    "EXIT CODES:\n",
    "    0  success\n",
    "    1  data, no-match, or I/O error\n",
    "    2  command-line argument parsing error\n",
);

#[cfg(test)]
mod tests {
    use super::HELP;

    #[test]
    fn help_uses_english_cli_labels() {
        assert!(HELP.contains("USAGE:"));
        assert!(HELP.contains("COMMON OPTIONS:"));
        assert!(HELP.contains("EXIT CODES:"));
        assert!(!HELP.contains("ИСПОЛЬЗОВАНИЕ"));
    }
}
