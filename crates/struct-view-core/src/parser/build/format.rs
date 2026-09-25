use std::fmt;
use std::path::Path;

/// Поддерживаемый формат структурированных данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// JavaScript Object Notation.
    Json,
    /// YAML Ain't Markup Language.
    Yaml,
    /// Tom's Obvious Minimal Language.
    Toml,
    /// Расширенный синтаксис JSON с комментариями и trailing commas.
    Json5,
}

impl DataFormat {
    /// Все форматы, доступные для преобразования и сохранения.
    pub const ALL: [Self; 4] = [Self::Json, Self::Yaml, Self::Toml, Self::Json5];

    /// Определить формат по расширению пути.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "json5" => Some(Self::Json5),
            _ => None,
        }
    }

    /// Основное расширение файла без точки.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json5 => "json5",
        }
    }

    /// Расширения, принимаемые диалогом выбора файла для этого формата.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Json => &["json"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Json5 => &["json5"],
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Json5 => "JSON5",
        })
    }
}
