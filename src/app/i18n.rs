//! Локализация пользовательского интерфейса.

/// Поддерживаемый язык интерфейса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Locale {
    /// Русский язык.
    #[default]
    Russian,
    /// English.
    English,
}

impl Locale {
    /// Все языки, доступные в меню выбора языка.
    pub(super) const ALL: [Self; 2] = [Self::Russian, Self::English];

    /// Название языка, отображаемое в меню.
    pub(super) fn language_name(self) -> &'static str {
        match self {
            Self::Russian => "Русский",
            Self::English => "English",
        }
    }

    /// Получить перевод статического сообщения.
    pub(super) fn text(self, key: TextKey) -> &'static str {
        match self {
            Self::Russian => russian_text(key),
            Self::English => english_text(key),
        }
    }

    /// Сформировать подпись объекта с правильной формой множественного числа.
    pub(super) fn object_count(self, count: usize) -> String {
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
    pub(super) fn array_count(self, count: usize) -> String {
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
    pub(super) fn loaded_file_status(
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
    pub(super) fn structures_copied(self, count: usize) -> String {
        match self {
            Self::Russian => format!("Скопировано структур: {count}"),
            Self::English => format!("Structures copied: {count}"),
        }
    }

    /// Сформировать уведомление о количестве вставленных структур.
    pub(super) fn structures_pasted(self, count: usize) -> String {
        match self {
            Self::Russian => format!("Вставлено структур: {count}"),
            Self::English => format!("Structures pasted: {count}"),
        }
    }
}

/// Идентификаторы статических сообщений интерфейса.
#[derive(Debug, Clone, Copy)]
pub(super) enum TextKey {
    FileMenu,
    Open,
    Save,
    SaveAs,
    CloseFile,
    Exit,
    EditMenu,
    CopySelectedStructures,
    PasteSelectedContainer,
    ViewMenu,
    LightTheme,
    DarkTheme,
    ExpandAll,
    CollapseAll,
    HelpMenu,
    BuildTime,
    TargetPlatform,
    HostPlatform,
    OptimizationLevel,
    DebugBuild,
    RustcCompiler,
    RustcChannel,
    SettingsMenu,
    Language,
    ToolbarExpandAll,
    ToolbarCollapseAll,
    Copy,
    Paste,
    Close,
    Mode,
    ViewMode,
    EditMode,
    SearchPlaceholder,
    SearchOptions,
    SearchKeys,
    SearchValues,
    CaseSensitive,
    ExactMatch,
    NotFound,
    UnknownFile,
    UnknownFormat,
    Placeholder,
    DropFilePlaceholder,
    Error,
    DataParseError,
    CopyValue,
    CopyKey,
    CopyPath,
    CopyStructure,
    CopySelected,
    PasteHere,
    AddField,
    AddElement,
    AddFieldTitle,
    AddElementTitle,
    FieldName,
    Value,
    Add,
    Cancel,
    FileSaved,
    DataAdded,
    Copied,
    NoDocument,
    SelectContainer,
    SelectOneContainer,
    PasteEditOnly,
}

impl Locale {
    /// Сформировать сообщение об ошибке чтения файла.
    pub(super) fn file_read_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка чтения файла: {error}"),
            Self::English => format!("File read error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке сохранения.
    pub(super) fn save_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка сохранения: {error}"),
            Self::English => format!("Save error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке копирования.
    pub(super) fn copy_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка копирования: {error}"),
            Self::English => format!("Copy error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке записи в системный буфер.
    pub(super) fn system_copy_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка копирования в системный буфер: {error}"),
            Self::English => format!("System clipboard copy error: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке чтения системного буфера.
    pub(super) fn clipboard_read_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Не удалось прочитать буфер обмена: {error}"),
            Self::English => format!("Could not read the clipboard: {error}"),
        }
    }

    /// Сформировать сообщение об ошибке вставки.
    pub(super) fn paste_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка вставки: {error}"),
            Self::English => format!("Paste error: {error}"),
        }
    }
}

fn russian_text(key: TextKey) -> &'static str {
    match key {
        TextKey::FileMenu => "Файл",
        TextKey::Open => "📂  Открыть…",
        TextKey::Save => "💾  Сохранить",
        TextKey::SaveAs => "💾  Сохранить как…",
        TextKey::CloseFile => "✖  Закрыть файл",
        TextKey::Exit => "❌  Выход",
        TextKey::EditMenu => "Правка",
        TextKey::CopySelectedStructures => "📋  Копировать выбранные структуры  Ctrl+C",
        TextKey::PasteSelectedContainer => "📥  Вставить в выбранный контейнер  Ctrl+V",
        TextKey::ViewMenu => "Вид",
        TextKey::LightTheme => "☀  Светлая тема",
        TextKey::DarkTheme => "🌙  Тёмная тема",
        TextKey::ExpandAll => ">  Развернуть все",
        TextKey::CollapseAll => "<  Свернуть все",
        TextKey::HelpMenu => "Помощь",
        TextKey::BuildTime => "Время сборки",
        TextKey::TargetPlatform => "Целевая платформа",
        TextKey::HostPlatform => "Платформа сборки",
        TextKey::OptimizationLevel => "Уровень оптимизации",
        TextKey::DebugBuild => "Отладочная сборка",
        TextKey::RustcCompiler => "Компилятор rustc",
        TextKey::RustcChannel => "Канал rustc",
        TextKey::SettingsMenu => "Настройки",
        TextKey::Language => "Язык",
        TextKey::ToolbarExpandAll => ">> Развернуть все",
        TextKey::ToolbarCollapseAll => "<< Свернуть все",
        TextKey::Copy => "📋 Копировать",
        TextKey::Paste => "📥 Вставить",
        TextKey::Close => "✖ Закрыть",
        TextKey::Mode => "Режим:",
        TextKey::ViewMode => "Просмотр",
        TextKey::EditMode => "Редактирование",
        TextKey::SearchPlaceholder => "Поиск по ключам и значениям…",
        TextKey::SearchOptions => "Параметры",
        TextKey::SearchKeys => "Искать в ключах",
        TextKey::SearchValues => "Искать в значениях",
        TextKey::CaseSensitive => "Учитывать регистр",
        TextKey::ExactMatch => "Точное совпадение",
        TextKey::NotFound => "Не найдено",
        TextKey::UnknownFile => "неизвестный файл",
        TextKey::UnknownFormat => "неизвестный формат",
        TextKey::Placeholder => "Откройте файл данных через меню Файл или перетащите его сюда",
        TextKey::DropFilePlaceholder => {
            "Перетащите JSON, YAML, TOML или JSON5 сюда\nили используйте Файл -> Открыть…"
        }
        TextKey::Error => "Ошибка",
        TextKey::DataParseError => "Ошибка разбора данных:",
        TextKey::CopyValue => "📋  Копировать значение",
        TextKey::CopyKey => "🔑  Копировать ключ",
        TextKey::CopyPath => "📍  Копировать путь",
        TextKey::CopyStructure => "🌿  Копировать структуру",
        TextKey::CopySelected => "🌿  Копировать выбранные структуры",
        TextKey::PasteHere => "📥  Вставить сюда",
        TextKey::AddField => "➕  Добавить поле…",
        TextKey::AddElement => "➕  Добавить элемент…",
        TextKey::AddFieldTitle => "Добавить поле",
        TextKey::AddElementTitle => "Добавить элемент",
        TextKey::FieldName => "Имя поля",
        TextKey::Value => "Значение",
        TextKey::Add => "Добавить",
        TextKey::Cancel => "Отмена",
        TextKey::FileSaved => "Файл сохранён",
        TextKey::DataAdded => "Данные добавлены",
        TextKey::Copied => "Скопировано в буфер обмена",
        TextKey::NoDocument => "Нет открытого документа",
        TextKey::SelectContainer => "Выберите контейнер для вставки",
        TextKey::SelectOneContainer => "Для вставки выберите ровно один контейнер",
        TextKey::PasteEditOnly => "Вставка доступна только в режиме редактирования",
    }
}

fn english_text(key: TextKey) -> &'static str {
    match key {
        TextKey::FileMenu => "File",
        TextKey::Open => "📂  Open…",
        TextKey::Save => "💾  Save",
        TextKey::SaveAs => "💾  Save as…",
        TextKey::CloseFile => "✖  Close file",
        TextKey::Exit => "❌  Exit",
        TextKey::EditMenu => "Edit",
        TextKey::CopySelectedStructures => "📋  Copy selected structures  Ctrl+C",
        TextKey::PasteSelectedContainer => "📥  Paste into selected container  Ctrl+V",
        TextKey::ViewMenu => "View",
        TextKey::LightTheme => "☀  Light theme",
        TextKey::DarkTheme => "🌙  Dark theme",
        TextKey::ExpandAll => ">  Expand all",
        TextKey::CollapseAll => "<  Collapse all",
        TextKey::HelpMenu => "Help",
        TextKey::BuildTime => "Build time",
        TextKey::TargetPlatform => "Target platform",
        TextKey::HostPlatform => "Build platform",
        TextKey::OptimizationLevel => "Optimization level",
        TextKey::DebugBuild => "Debug build",
        TextKey::RustcCompiler => "rustc compiler",
        TextKey::RustcChannel => "rustc channel",
        TextKey::SettingsMenu => "Settings",
        TextKey::Language => "Language",
        TextKey::ToolbarExpandAll => ">> Expand all",
        TextKey::ToolbarCollapseAll => "<< Collapse all",
        TextKey::Copy => "📋 Copy",
        TextKey::Paste => "📥 Paste",
        TextKey::Close => "✖ Close",
        TextKey::Mode => "Mode:",
        TextKey::ViewMode => "View",
        TextKey::EditMode => "Edit",
        TextKey::SearchPlaceholder => "Search keys and values…",
        TextKey::SearchOptions => "Options",
        TextKey::SearchKeys => "Search in keys",
        TextKey::SearchValues => "Search in values",
        TextKey::CaseSensitive => "Case-sensitive",
        TextKey::ExactMatch => "Exact match",
        TextKey::NotFound => "Not found",
        TextKey::UnknownFile => "unknown file",
        TextKey::UnknownFormat => "unknown format",
        TextKey::Placeholder => "Open a data file from the File menu or drop it here",
        TextKey::DropFilePlaceholder => {
            "Drop a JSON, YAML, TOML, or JSON5 file here\nor use File -> Open…"
        }
        TextKey::Error => "Error",
        TextKey::DataParseError => "Data parsing error:",
        TextKey::CopyValue => "📋  Copy value",
        TextKey::CopyKey => "🔑  Copy key",
        TextKey::CopyPath => "📍  Copy path",
        TextKey::CopyStructure => "🌿  Copy structure",
        TextKey::CopySelected => "🌿  Copy selected structures",
        TextKey::PasteHere => "📥  Paste here",
        TextKey::AddField => "➕  Add field…",
        TextKey::AddElement => "➕  Add element…",
        TextKey::AddFieldTitle => "Add field",
        TextKey::AddElementTitle => "Add element",
        TextKey::FieldName => "Field name",
        TextKey::Value => "Value",
        TextKey::Add => "Add",
        TextKey::Cancel => "Cancel",
        TextKey::FileSaved => "File saved",
        TextKey::DataAdded => "Data added",
        TextKey::Copied => "Copied to clipboard",
        TextKey::NoDocument => "No document is open",
        TextKey::SelectContainer => "Select a container to paste into",
        TextKey::SelectOneContainer => "Select exactly one container to paste into",
        TextKey::PasteEditOnly => "Pasting is available only in edit mode",
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

#[cfg(test)]
mod tests {
    use super::{Locale, TextKey};

    #[test]
    fn russian_is_the_default_locale() {
        assert_eq!(Locale::default(), Locale::Russian);
    }

    #[test]
    fn both_catalogs_have_core_translations() {
        for locale in Locale::ALL {
            assert!(!locale.text(TextKey::FileMenu).is_empty());
            assert!(!locale.text(TextKey::SearchPlaceholder).is_empty());
            assert!(!locale.text(TextKey::CopyStructure).is_empty());
        }
    }

    #[test]
    fn container_counts_are_localized() {
        assert_eq!(Locale::Russian.object_count(2), "{2} поля");
        assert_eq!(Locale::English.object_count(2), "{2} fields");
        assert_eq!(Locale::Russian.array_count(1), "[1] элемент");
        assert_eq!(Locale::English.array_count(1), "[1] element");
    }
}
