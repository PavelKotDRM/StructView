use super::TextKey;
use super::catalog::{english_text, russian_text};

/// Поддерживаемый язык интерфейса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Locale {
    /// Русский язык.
    #[default]
    Russian,
    /// English.
    English,
}

impl Locale {
    /// Все языки, доступные в меню выбора языка.
    pub(in crate::app) const ALL: [Self; 2] = [Self::Russian, Self::English];

    /// Название языка, отображаемое в меню.
    pub(in crate::app) fn language_name(self) -> &'static str {
        match self {
            Self::Russian => "Русский",
            Self::English => "English",
        }
    }

    /// Сформировать строку статуса режима сравнения.
    pub(in crate::app) fn comparison_status(
        self,
        file_count: usize,
        difference_count: usize,
    ) -> String {
        match self {
            Self::Russian => {
                format!("Сравнение файлов: {file_count}  |  отличий: {difference_count}")
            }
            Self::English => {
                format!("File comparison: {file_count}  |  differences: {difference_count}")
            }
        }
    }

    /// Получить перевод статического сообщения.
    pub(in crate::app) fn text(self, key: TextKey) -> &'static str {
        match self {
            Self::Russian => russian_text(key),
            Self::English => english_text(key),
        }
    }

    /// Сформировать подпись объекта с правильной формой множественного числа.
    pub(in crate::app) fn object_count(self, count: usize) -> String {
        match self {
            Self::Russian => format!(
                "{{{count}}} {}",
                russian_plural(count, "поле", "поля", "полей")
            ),
            Self::English => {
                let noun = if count == 1 { "field" } else { "fields" };
                format!("{{{count}}} {noun}")
            }
        }
    }

    /// Сформировать подпись массива с правильной формой множественного числа.
    pub(in crate::app) fn array_count(self, count: usize) -> String {
        match self {
            Self::Russian => format!(
                "[{count}] {}",
                russian_plural(count, "элемент", "элемента", "элементов")
            ),
            Self::English => {
                let noun = if count == 1 { "element" } else { "elements" };
                format!("[{count}] {noun}")
            }
        }
    }

    /// Сформировать строку статуса загруженного файла.
    pub(in crate::app) fn loaded_file_status(
        self,
        name: &str,
        format: &str,
        size_kb: f64,
        load_time_ms: u128,
    ) -> String {
        match self {
            Self::Russian => format!(
                "📄 {name}  |  {format}  |  {size_kb:.1} КБ  |  загружено за {load_time_ms} мс"
            ),
            Self::English => format!(
                "📄 {name}  |  {format}  |  {size_kb:.1} KB  |  loaded in {load_time_ms} ms"
            ),
        }
    }

    /// Сформировать уведомление о количестве скопированных структур.
    pub(in crate::app) fn structures_copied(self, count: usize) -> String {
        match self {
            Self::Russian => format!("Скопировано структур: {count}"),
            Self::English => format!("Structures copied: {count}"),
        }
    }

    /// Сформировать уведомление о количестве вставленных структур.
    pub(in crate::app) fn structures_pasted(self, count: usize) -> String {
        match self {
            Self::Russian => format!("Вставлено структур: {count}"),
            Self::English => format!("Structures pasted: {count}"),
        }
    }

    /// Сформировать уведомление о количестве удалённых структур.
    pub(in crate::app) fn structures_deleted(self, count: usize) -> String {
        match self {
            Self::Russian => format!("Удалено структур: {count}"),
            Self::English => format!("Structures deleted: {count}"),
        }
    }
}

impl Locale {
    /// Сформировать сообщение об ошибке чтения файла.
    pub(in crate::app) fn file_read_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка чтения файла: {error}"),
            Self::English => format!("File read error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке сохранения.
    pub(in crate::app) fn save_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка сохранения: {error}"),
            Self::English => format!("Save error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке копирования.
    pub(in crate::app) fn copy_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка копирования: {error}"),
            Self::English => format!("Copy error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке записи в системный буфер.
    pub(in crate::app) fn system_copy_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка копирования в системный буфер: {error}"),
            Self::English => format!("System clipboard copy error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке чтения системного буфера.
    pub(in crate::app) fn clipboard_read_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Не удалось прочитать буфер обмена: {error}"),
            Self::English => format!("Could not read the clipboard: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке вставки.
    pub(in crate::app) fn paste_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка вставки: {error}"),
            Self::English => format!("Paste error: {error}"),
        }
    }
}

fn russian_plural<'a>(count: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let rem100 = count % 100;
    let rem10 = count % 10;
    if (11..=19).contains(&rem100) {
        many
    } else if rem10 == 1 {
        one
    } else if (2..=4).contains(&rem10) {
        few
    } else {
        many
    }
}
