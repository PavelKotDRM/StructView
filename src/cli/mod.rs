//! # Модуль командной строки
//!
//! Разбирает аргументы командной строки и выполняет headless-команды
//! (без запуска GUI): форматирование, валидацию и поиск по JSON.
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
    " — просмотр и обработка JSON\n",
    "\n",
    "ИСПОЛЬЗОВАНИЕ:\n",
    "    json_viewer [ФАЙЛ]                     запустить GUI (опционально с файлом)\n",
    "    json_viewer format [ФАЙЛ] [ОПЦИИ]      форматировать JSON\n",
    "    json_viewer validate [ФАЙЛ]            проверить синтаксис JSON\n",
    "    json_viewer find <ЗАПРОС> [ФАЙЛ]       найти пути узлов по подстроке\n",
    "\n",
    "ОПЦИИ format:\n",
    "    -o, --output <ФАЙЛ>    записать результат в файл вместо stdout\n",
    "    -m, --minify           вывести компактный JSON без отступов\n",
    "\n",
    "ОБЩИЕ ОПЦИИ:\n",
    "    -h, --help             показать эту справку\n",
    "    -V, --version          показать версию и информацию о сборке\n",
    "\n",
    "Вместо ФАЙЛ можно указать `-`, чтобы читать JSON из stdin.\n",
    "\n",
    "КОДЫ ВОЗВРАТА:\n",
    "    0  успех\n",
    "    1  ошибка (некорректный JSON, нет совпадений, ошибка ввода-вывода)\n",
    "    2  ошибка разбора аргументов командной строки\n",
);
