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

    /// Сформировать строку статуса режима сравнения.
    pub(super) fn comparison_status(self, file_count: usize, difference_count: usize) -> String {
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
    NewFile,
    Open,
    CompareFiles,
    Save,
    SaveAs,
    ConvertTo,
    CloseFile,
    Exit,
    EditMenu,
    CopySelectedStructures,
    PasteSelectedContainer,
    ViewMenu,
    Visualization,
    TreeView,
    GraphView,
    TableView,
    SchemaView,
    ComparisonView,
    DiffView,
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
    EditField,
    AddField,
    AddElement,
    EditFieldTitle,
    AddFieldTitle,
    AddElementTitle,
    FieldName,
    FieldType,
    TypeString,
    TypeDateTime,
    TypeInteger,
    TypeNumber,
    TypeFloat,
    TypeBoolean,
    TypeNull,
    TypeObject,
    TypeArray,
    EmptyObject,
    EmptyArray,
    Value,
    Add,
    Apply,
    Cancel,
    FileCreated,
    FileSaved,
    FileConverted,
    FieldUpdated,
    DataAdded,
    Copied,
    NoDocument,
    SelectContainer,
    SelectOneContainer,
    PasteEditOnly,
    ComparisonPath,
    ComparisonNoDifferences,
    MissingValue,
    ComparisonRequiresFiles,
    UnsupportedFileExtension,
    TableDescription,
    ExportCsv,
    TableExported,
    SchemaType,
    SchemaRequired,
    SchemaConstraints,
    SchemaReference,
    SchemaInferredNotice,
    SchemaRequiredValue,
    SchemaOptionalValue,
    SchemaPresentInAllSamples,
    SchemaPresentInSomeSamples,
    SchemaJsonSchema,
    SchemaOpenApi,
    SchemaInferred,
    SchemaAnyType,
    SchemaNeverType,
    SchemaNoDefinitions,
    GraphNodes,
    GraphEdges,
    GraphNoEntities,
    GraphNoRelationships,
    DiffLeft,
    DiffRight,
    DiffNoDifferences,
    DiffChangeType,
    DiffAdded,
    DiffRemoved,
    DiffChanged,
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
        TextKey::NewFile => "✨  Создать файл…",
        TextKey::Open => "📂  Открыть…",
        TextKey::CompareFiles => "⚖  Сравнить файлы…",
        TextKey::Save => "💾  Сохранить",
        TextKey::SaveAs => "💾  Сохранить как…",
        TextKey::ConvertTo => "🔄  Преобразовать в",
        TextKey::CloseFile => "✖  Закрыть файл",
        TextKey::Exit => "❌  Выход",
        TextKey::EditMenu => "Правка",
        TextKey::CopySelectedStructures => "📋  Копировать выбранные структуры  Ctrl+C",
        TextKey::PasteSelectedContainer => "📥  Вставить в выбранный контейнер  Ctrl+V",
        TextKey::ViewMenu => "Вид",
        TextKey::Visualization => "Представление:",
        TextKey::TreeView => "Интерактивное дерево",
        TextKey::GraphView => "Граф связей",
        TextKey::TableView => "Таблица",
        TextKey::SchemaView => "Диаграмма схемы",
        TextKey::ComparisonView => "Все отличия",
        TextKey::DiffView => "Парный diff",
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
        TextKey::EditField => "✏  Редактировать поле…",
        TextKey::AddField => "➕  Добавить поле…",
        TextKey::AddElement => "➕  Добавить элемент…",
        TextKey::EditFieldTitle => "Редактировать поле",
        TextKey::AddFieldTitle => "Добавить поле",
        TextKey::AddElementTitle => "Добавить элемент",
        TextKey::FieldName => "Имя поля",
        TextKey::FieldType => "Тип",
        TextKey::TypeString => "Строка",
        TextKey::TypeDateTime => "Дата/время (TOML)",
        TextKey::TypeInteger => "Целое число",
        TextKey::TypeNumber => "Число",
        TextKey::TypeFloat => "Вещественное число",
        TextKey::TypeBoolean => "Логическое",
        TextKey::TypeNull => "Пустое значение (null)",
        TextKey::TypeObject => "Объект",
        TextKey::TypeArray => "Массив",
        TextKey::EmptyObject => "Будет создан пустой объект",
        TextKey::EmptyArray => "Будет создан пустой массив",
        TextKey::Value => "Значение",
        TextKey::Add => "Добавить",
        TextKey::Apply => "Применить",
        TextKey::Cancel => "Отмена",
        TextKey::FileCreated => "Файл создан",
        TextKey::FileSaved => "Файл сохранён",
        TextKey::FileConverted => "Файл преобразован",
        TextKey::FieldUpdated => "Поле изменено",
        TextKey::DataAdded => "Данные добавлены",
        TextKey::Copied => "Скопировано в буфер обмена",
        TextKey::NoDocument => "Нет открытого документа",
        TextKey::SelectContainer => "Выберите контейнер для вставки",
        TextKey::SelectOneContainer => "Для вставки выберите ровно один контейнер",
        TextKey::PasteEditOnly => "Вставка доступна только в режиме редактирования",
        TextKey::ComparisonPath => "Путь",
        TextKey::ComparisonNoDifferences => "Файлы не отличаются",
        TextKey::MissingValue => "<отсутствует>",
        TextKey::ComparisonRequiresFiles => "Для сравнения выберите минимум два файла",
        TextKey::UnsupportedFileExtension => {
            "Укажите расширение .json, .yaml, .yml, .toml или .json5"
        }
        TextKey::TableDescription => "Путь → значение → тип. Поиск фильтрует строки.",
        TextKey::ExportCsv => "Экспорт CSV…",
        TextKey::TableExported => "Таблица экспортирована",
        TextKey::SchemaType => "Тип",
        TextKey::SchemaRequired => "Обязательность",
        TextKey::SchemaConstraints => "Ограничения",
        TextKey::SchemaReference => "Связь / $ref",
        TextKey::SchemaInferredNotice => "Выведено из примера; это не контракт.",
        TextKey::SchemaRequiredValue => "Обязательное",
        TextKey::SchemaOptionalValue => "Необязательное",
        TextKey::SchemaPresentInAllSamples => "Во всех примерах",
        TextKey::SchemaPresentInSomeSamples => "Не во всех примерах",
        TextKey::SchemaJsonSchema => "JSON Schema",
        TextKey::SchemaOpenApi => "Схемы OpenAPI",
        TextKey::SchemaInferred => "Выведенная схема",
        TextKey::SchemaAnyType => "Любой тип",
        TextKey::SchemaNeverType => "Недопустимо",
        TextKey::SchemaNoDefinitions => "В документе не найдено описаний схем.",
        TextKey::GraphNodes => "Сущности",
        TextKey::GraphEdges => "Связи",
        TextKey::GraphNoEntities => "Сущности с идентификаторами не найдены.",
        TextKey::GraphNoRelationships => {
            "Узлы есть, но распознаваемые ссылки между ними не найдены."
        }
        TextKey::DiffLeft => "Версия A:",
        TextKey::DiffRight => "Версия B:",
        TextKey::DiffNoDifferences => "Эти две версии не отличаются.",
        TextKey::DiffChangeType => "Изменение",
        TextKey::DiffAdded => "Добавлено",
        TextKey::DiffRemoved => "Удалено",
        TextKey::DiffChanged => "Изменено",
    }
}

fn english_text(key: TextKey) -> &'static str {
    match key {
        TextKey::FileMenu => "File",
        TextKey::NewFile => "✨  New file…",
        TextKey::Open => "📂  Open…",
        TextKey::CompareFiles => "⚖  Compare files…",
        TextKey::Save => "💾  Save",
        TextKey::SaveAs => "💾  Save as…",
        TextKey::ConvertTo => "🔄  Convert to",
        TextKey::CloseFile => "✖  Close file",
        TextKey::Exit => "❌  Exit",
        TextKey::EditMenu => "Edit",
        TextKey::CopySelectedStructures => "📋  Copy selected structures  Ctrl+C",
        TextKey::PasteSelectedContainer => "📥  Paste into selected container  Ctrl+V",
        TextKey::ViewMenu => "View",
        TextKey::Visualization => "View:",
        TextKey::TreeView => "Interactive tree",
        TextKey::GraphView => "Relationship graph",
        TextKey::TableView => "Table",
        TextKey::SchemaView => "Schema diagram",
        TextKey::ComparisonView => "All differences",
        TextKey::DiffView => "Pair diff",
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
        TextKey::EditField => "✏  Edit field…",
        TextKey::AddField => "➕  Add field…",
        TextKey::AddElement => "➕  Add element…",
        TextKey::EditFieldTitle => "Edit field",
        TextKey::AddFieldTitle => "Add field",
        TextKey::AddElementTitle => "Add element",
        TextKey::FieldName => "Field name",
        TextKey::FieldType => "Type",
        TextKey::TypeString => "String",
        TextKey::TypeDateTime => "Date/time (TOML)",
        TextKey::TypeInteger => "Integer",
        TextKey::TypeNumber => "Number",
        TextKey::TypeFloat => "Float",
        TextKey::TypeBoolean => "Boolean",
        TextKey::TypeNull => "Null",
        TextKey::TypeObject => "Object",
        TextKey::TypeArray => "Array",
        TextKey::EmptyObject => "An empty object will be created",
        TextKey::EmptyArray => "An empty array will be created",
        TextKey::Value => "Value",
        TextKey::Add => "Add",
        TextKey::Apply => "Apply",
        TextKey::Cancel => "Cancel",
        TextKey::FileCreated => "File created",
        TextKey::FileSaved => "File saved",
        TextKey::FileConverted => "File converted",
        TextKey::FieldUpdated => "Field updated",
        TextKey::DataAdded => "Data added",
        TextKey::Copied => "Copied to clipboard",
        TextKey::NoDocument => "No document is open",
        TextKey::SelectContainer => "Select a container to paste into",
        TextKey::SelectOneContainer => "Select exactly one container to paste into",
        TextKey::PasteEditOnly => "Pasting is available only in edit mode",
        TextKey::ComparisonPath => "Path",
        TextKey::ComparisonNoDifferences => "Files are identical",
        TextKey::MissingValue => "<missing>",
        TextKey::ComparisonRequiresFiles => "Select at least two files to compare",
        TextKey::UnsupportedFileExtension => "Use a .json, .yaml, .yml, .toml, or .json5 extension",
        TextKey::TableDescription => "Path → value → type. Search filters the rows.",
        TextKey::ExportCsv => "Export CSV…",
        TextKey::TableExported => "Table exported",
        TextKey::SchemaType => "Type",
        TextKey::SchemaRequired => "Requirement",
        TextKey::SchemaConstraints => "Constraints",
        TextKey::SchemaReference => "Link / $ref",
        TextKey::SchemaInferredNotice => "Inferred from the sample; not a contract.",
        TextKey::SchemaRequiredValue => "Required",
        TextKey::SchemaOptionalValue => "Optional",
        TextKey::SchemaPresentInAllSamples => "In all samples",
        TextKey::SchemaPresentInSomeSamples => "Not in all samples",
        TextKey::SchemaJsonSchema => "JSON Schema",
        TextKey::SchemaOpenApi => "OpenAPI schemas",
        TextKey::SchemaInferred => "Inferred schema",
        TextKey::SchemaAnyType => "Any type",
        TextKey::SchemaNeverType => "Never valid",
        TextKey::SchemaNoDefinitions => "No schema definitions were found in this document.",
        TextKey::GraphNodes => "Entities",
        TextKey::GraphEdges => "Links",
        TextKey::GraphNoEntities => "No entities with identifiers were found.",
        TextKey::GraphNoRelationships => {
            "Entities were found, but no supported references connect them."
        }
        TextKey::DiffLeft => "Version A:",
        TextKey::DiffRight => "Version B:",
        TextKey::DiffNoDifferences => "These two versions are identical.",
        TextKey::DiffChangeType => "Change",
        TextKey::DiffAdded => "Added",
        TextKey::DiffRemoved => "Removed",
        TextKey::DiffChanged => "Changed",
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
