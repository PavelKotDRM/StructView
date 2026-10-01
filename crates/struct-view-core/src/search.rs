//! # Модуль поиска по JSON-дереву
//!
//! Реализует полнотекстовый поиск по ключам, значениям и путям узлов дерева [`JsonNode`].
//! Найденные совпадения сохраняются как список путей, по которым можно
//! навигировать (Next / Previous).

use crate::parser::JsonNode;
use regex::{Regex, RegexBuilder};
use std::borrow::Cow;

/// Escape a string so it is interpreted literally in a regular expression.
pub fn escape_regex_literal(text: &str) -> String {
    regex::escape(text)
}

/// Параметры поиска по дереву данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    /// Искать в именах полей.
    pub search_keys: bool,
    /// Искать в отображаемых значениях.
    pub search_values: bool,
    /// Искать в JSON-путях узлов.
    pub search_paths: bool,
    /// Учитывать регистр символов.
    pub case_sensitive: bool,
    /// Требовать полного совпадения вместо поиска по подстроке.
    pub exact_match: bool,
    /// Требовать совпадения отдельного слова.
    pub whole_word: bool,
    /// Интерпретировать запрос как регулярное выражение.
    pub use_regex: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            search_keys: true,
            search_values: true,
            search_paths: true,
            case_sensitive: false,
            exact_match: false,
            whole_word: false,
            use_regex: false,
        }
    }
}

/// Состояние поискового запроса.
///
/// Хранит текущий запрос, список путей совпадений и индекс активного совпадения.
#[derive(Debug, Default, Clone)]
pub struct SearchState {
    /// Текущая поисковая строка.
    pub query: String,
    /// Пути узлов, соответствующих запросу.
    pub matches: Vec<String>,
    /// Индекс текущего активного совпадения.
    pub current_index: usize,
    /// Активные параметры поиска.
    pub options: SearchOptions,
    /// Ошибка поискового запроса, например некорректное регулярное выражение.
    pub error: Option<String>,
    pattern: Option<SearchPattern>,
}

#[derive(Debug, Clone)]
enum SearchMatcher {
    Literal {
        query: String,
        case_sensitive: bool,
        exact_match: bool,
        whole_word: bool,
    },
    Regex(Regex),
}

impl SearchMatcher {
    fn new(query: &str, options: SearchOptions) -> Result<Self, String> {
        if options.use_regex {
            let mut builder = RegexBuilder::new(query);
            builder.case_insensitive(!options.case_sensitive);
            return builder
                .build()
                .map(Self::Regex)
                .map_err(|error| error.to_string());
        }

        Ok(Self::Literal {
            query: if options.case_sensitive {
                query.to_string()
            } else {
                query.to_lowercase()
            },
            case_sensitive: options.case_sensitive,
            exact_match: options.exact_match,
            whole_word: options.whole_word,
        })
    }

    fn is_match(&self, text: &str) -> bool {
        match self {
            SearchMatcher::Regex(regex) => regex.is_match(text),
            SearchMatcher::Literal {
                query,
                case_sensitive,
                exact_match,
                whole_word,
            } => {
                let text = if *case_sensitive {
                    Cow::Borrowed(text)
                } else {
                    Cow::Owned(text.to_lowercase())
                };

                if *exact_match {
                    text == query.as_str()
                } else if *whole_word {
                    contains_whole_word(&text, query)
                } else {
                    text.contains(query)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum SearchPattern {
    Text(SearchMatcher),
    KeyValue {
        key: SearchMatcher,
        value: SearchMatcher,
    },
}

impl SearchPattern {
    fn new(query: &str, options: SearchOptions) -> Result<Self, String> {
        if let Some((key, value)) = split_key_value_query(query) {
            return Ok(Self::KeyValue {
                key: SearchMatcher::new(key, options)?,
                value: SearchMatcher::new(value, options)?,
            });
        }

        SearchMatcher::new(query, options).map(Self::Text)
    }

    fn matches_text(&self, text: &str) -> bool {
        match self {
            Self::Text(pattern) => pattern.is_match(text),
            Self::KeyValue { .. } => false,
        }
    }

    fn matches_fields(
        &self,
        key: Option<&str>,
        value: &str,
        path: &str,
        options: SearchOptions,
    ) -> bool {
        match self {
            Self::Text(pattern) => {
                (options.search_keys && key.is_some_and(|key| pattern.is_match(key)))
                    || (options.search_values && pattern.is_match(value))
                    || (options.search_paths && pattern.is_match(path))
            }
            Self::KeyValue {
                key: key_pattern,
                value: value_pattern,
            } => {
                options.search_keys
                    && options.search_values
                    && key.is_some_and(|key| key_pattern.is_match(key))
                    && value_pattern.is_match(value)
            }
        }
    }
}

fn split_key_value_query(query: &str) -> Option<(&str, &str)> {
    let (key, value) = query.split_once(':')?;
    // Whitespace distinguishes this syntax from URLs and timestamps.
    if !key.chars().next_back().is_some_and(char::is_whitespace)
        && !value.chars().next().is_some_and(char::is_whitespace)
    {
        return None;
    }

    let key = key.trim();
    let value = value.trim();
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

fn contains_whole_word(text: &str, query: &str) -> bool {
    let Some(first) = query.chars().next() else {
        return false;
    };
    let is_word_character = |character: char| character.is_alphanumeric() || character == '_';
    let mut start = 0;
    while let Some(offset) = text[start..].find(query) {
        let position = start + offset;
        let end = position + query.len();
        if text[..position]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_character(c))
            && text[end..]
                .chars()
                .next()
                .is_none_or(|c| !is_word_character(c))
        {
            return true;
        }
        start = position + first.len_utf8();
    }
    false
}

impl SearchState {
    /// Выполнить поиск по дереву.
    ///
    /// Обновляет список [`Self::matches`] и сбрасывает [`Self::current_index`] в `0`.
    /// По умолчанию поиск регистронезависимый и проверяет ключи, отображаемые
    /// значения и пути каждого узла. Текущие параметры берутся из [`Self::options`].
    /// Ошибка некорректного регулярного выражения сохраняется в [`Self::error`].
    ///
    /// # Arguments
    ///
    /// * `root` — корневой узел JSON-дерева.
    /// * `query` — строка поиска.
    pub fn search(&mut self, root: &JsonNode, query: &str) {
        self.search_with_options(root, query, self.options);
    }

    /// Выполнить поиск с указанными параметрами.
    pub fn search_with_options(&mut self, root: &JsonNode, query: &str, options: SearchOptions) {
        self.query = query.to_string();
        self.options = options;
        self.matches.clear();
        self.current_index = 0;
        self.error = None;
        self.pattern = None;
        if query.is_empty()
            || (!options.search_keys && !options.search_values && !options.search_paths)
        {
            return;
        }

        let pattern = match SearchPattern::new(query, options) {
            Ok(pattern) => pattern,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        collect_matches(root, &pattern, options, &mut self.matches);
        self.pattern = Some(pattern);
    }

    /// Перейти к следующему совпадению.
    ///
    /// Если совпадений нет, ничего не делает. Навигация цикличная.
    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.current_index = (self.current_index + 1) % self.matches.len();
        }
    }

    /// Перейти к предыдущему совпадению.
    ///
    /// Если совпадений нет, ничего не делает. Навигация цикличная.
    pub fn prev(&mut self) {
        if !self.matches.is_empty() {
            self.current_index =
                (self.current_index + self.matches.len().saturating_sub(1)) % self.matches.len();
        }
    }

    /// Вернуть путь текущего активного совпадения.
    pub fn current_match_path(&self) -> Option<&str> {
        self.matches.get(self.current_index).map(|s| s.as_str())
    }

    /// Проверить, является ли путь активным совпадением.
    pub fn is_active(&self, path: &str) -> bool {
        self.matches
            .get(self.current_index)
            .is_some_and(|p| p == path)
    }

    /// Проверить, является ли путь любым (не обязательно активным) совпадением.
    pub fn is_match(&self, path: &str) -> bool {
        self.matches.iter().any(|p| p == path)
    }

    /// Проверить текст тем же поисковым запросом и параметрами.
    /// Для запроса `ключ: значение` отдельный текст не может дать совпадение.
    pub fn matches_text(&self, text: &str) -> bool {
        self.pattern
            .as_ref()
            .is_some_and(|pattern| pattern.matches_text(text))
    }

    /// Проверить ключ, значение и путь записи текущим поисковым запросом.
    pub fn matches_fields(&self, key: Option<&str>, value: &str, path: &str) -> bool {
        self.pattern
            .as_ref()
            .is_some_and(|pattern| pattern.matches_fields(key, value, path, self.options))
    }
}

/// Рекурсивно собрать пути всех узлов, соответствующих запросу и параметрам.
fn collect_matches(
    node: &JsonNode,
    pattern: &SearchPattern,
    options: SearchOptions,
    result: &mut Vec<String>,
) {
    if pattern.matches_fields(
        node.key.as_deref(),
        &node.display_value,
        &node.path,
        options,
    ) {
        result.push(node.path.clone());
    }

    for child in &node.children {
        collect_matches(child, pattern, options, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_json;

    #[test]
    fn search_options_filter_scope_case_and_match_kind() {
        let root = parse_json(r#"{"Name":"Alice","role":"admin"}"#).unwrap();
        let mut state = SearchState::default();

        state.search_with_options(
            &root,
            "name",
            SearchOptions {
                search_keys: true,
                search_values: false,
                ..Default::default()
            },
        );
        assert_eq!(state.matches, ["Name"]);

        state.search_with_options(
            &root,
            "ALICE",
            SearchOptions {
                search_keys: false,
                search_values: true,
                case_sensitive: true,
                ..Default::default()
            },
        );
        assert!(state.matches.is_empty());

        state.search_with_options(
            &root,
            "\"Alice\"",
            SearchOptions {
                search_keys: false,
                search_values: true,
                exact_match: true,
                ..Default::default()
            },
        );
        assert_eq!(state.matches, ["Name"]);
    }

    #[test]
    fn search_options_support_paths_and_whole_words() {
        let root = parse_json(r#"{"user":{"username":"alice","role":"admin"}}"#).unwrap();
        let mut state = SearchState::default();

        state.search_with_options(
            &root,
            "user",
            SearchOptions {
                search_keys: false,
                search_values: false,
                search_paths: true,
                ..Default::default()
            },
        );
        assert_eq!(state.matches, ["user", "user.role", "user.username"]);

        state.search_with_options(
            &root,
            "user",
            SearchOptions {
                search_keys: false,
                search_values: true,
                search_paths: false,
                whole_word: true,
                ..Default::default()
            },
        );
        assert!(state.matches.is_empty());
    }

    #[test]
    fn regex_search_supports_paths_and_case_sensitivity() {
        let root = parse_json(r#"{"profile":{"user_name":"Alice"}}"#).unwrap();
        let mut state = SearchState::default();
        let options = SearchOptions {
            search_keys: false,
            search_values: false,
            search_paths: true,
            case_sensitive: true,
            use_regex: true,
            ..Default::default()
        };

        state.search_with_options(&root, r"^profile\.user_[a-z]+$", options);

        assert_eq!(state.matches, ["profile.user_name"]);
        assert!(state.matches_text("profile.user_name"));
        assert!(!state.matches_text("PROFILE.user_name"));
    }

    #[test]
    fn invalid_regex_clears_old_matches_and_exposes_the_error() {
        let root = parse_json(r#"{"name":"Alice"}"#).unwrap();
        let mut state = SearchState::default();
        state.search(&root, "Alice");
        assert!(!state.matches.is_empty());

        state.search_with_options(
            &root,
            "[",
            SearchOptions {
                use_regex: true,
                ..Default::default()
            },
        );

        assert!(state.matches.is_empty());
        assert!(state.current_match_path().is_none());
        assert!(state.error.is_some());
        assert!(!state.matches_text("Alice"));
    }

    #[test]
    fn escaped_regex_literal_matches_metacharacters_as_text() {
        let root = parse_json(r#"{"value":"a.b[1]"}"#).unwrap();
        let mut state = SearchState::default();
        let literal = escape_regex_literal("a.b[1]");

        state.search_with_options(
            &root,
            &literal,
            SearchOptions {
                search_keys: false,
                search_values: true,
                search_paths: false,
                use_regex: true,
                ..Default::default()
            },
        );

        assert_eq!(state.matches, ["value"]);
    }

    #[test]
    fn whole_word_search_supports_phrases_and_overlapping_candidates() {
        let root =
            parse_json(r#"{"phrase":"hello world","overlap":"xa a a","other":"hello worlds"}"#)
                .unwrap();
        let mut state = SearchState::default();
        let options = SearchOptions {
            search_keys: false,
            search_paths: false,
            whole_word: true,
            ..Default::default()
        };
        state.search_with_options(&root, "hello world", options);
        assert_eq!(state.matches, ["phrase"]);
        state.search_with_options(&root, "a a", options);
        assert_eq!(state.matches, ["overlap"]);
    }

    #[test]
    fn key_value_search_matches_both_fields_on_the_same_node() {
        let root = parse_json(
            r#"{"user":{"name":"Alice Smith"},"role":"admin","endpoint":"https://example.test"}"#,
        )
        .unwrap();
        let mut state = SearchState::default();

        state.search(&root, " NAME : ali ");
        assert_eq!(state.matches, ["user.name"]);

        state.search(&root, "user: admin");
        assert!(state.matches.is_empty());

        state.search(&root, "https://example");
        assert_eq!(state.matches, ["endpoint"]);
    }

    #[test]
    fn key_value_search_respects_search_scopes_and_regex_options() {
        let root = parse_json(r#"{"name":"Alice","nickname":"Alicia"}"#).unwrap();
        let mut state = SearchState::default();

        state.search_with_options(
            &root,
            "name: Alice",
            SearchOptions {
                search_values: false,
                ..Default::default()
            },
        );
        assert!(state.matches.is_empty());

        state.search_with_options(
            &root,
            r"^name$: .*Alice.*",
            SearchOptions {
                use_regex: true,
                ..Default::default()
            },
        );
        assert_eq!(state.matches, ["name"]);
    }
}
