//! # Модуль поиска по JSON-дереву
//!
//! Реализует полнотекстовый поиск по ключам и значениям узлов дерева [`JsonNode`].
//! Найденные совпадения сохраняются как список путей, по которым можно
//! навигировать (Next / Previous).

use crate::parser::JsonNode;

/// Параметры поиска по дереву данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    /// Искать в именах полей.
    pub search_keys: bool,
    /// Искать в отображаемых значениях.
    pub search_values: bool,
    /// Учитывать регистр символов.
    pub case_sensitive: bool,
    /// Требовать полного совпадения вместо поиска по подстроке.
    pub exact_match: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            search_keys: true,
            search_values: true,
            case_sensitive: false,
            exact_match: false,
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
}

impl SearchState {
    /// Выполнить поиск по дереву.
    ///
    /// Обновляет список [`Self::matches`] и сбрасывает [`Self::current_index`] в `0`.
    /// По умолчанию поиск регистронезависимый и проверяет ключи и отображаемые
    /// значения каждого узла. Текущие параметры берутся из [`Self::options`].
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
        if query.is_empty() || (!options.search_keys && !options.search_values) {
            return;
        }
        collect_matches(root, query, options, &mut self.matches);
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
}

/// Рекурсивно собрать пути всех узлов, соответствующих запросу и параметрам.
fn collect_matches(node: &JsonNode, query: &str, options: SearchOptions, result: &mut Vec<String>) {
    let key_match = options.search_keys
        && node
            .key
            .as_deref()
            .is_some_and(|key| text_matches(key, query, options));
    let value_match = options.search_values && text_matches(&node.display_value, query, options);

    if key_match || value_match {
        result.push(node.path.clone());
    }

    for child in &node.children {
        collect_matches(child, query, options, result);
    }
}

/// Проверить совпадение текста с учётом регистра и выбранного вида совпадения.
fn text_matches(text: &str, query: &str, options: SearchOptions) -> bool {
    let (text, query) = if options.case_sensitive {
        (text.to_string(), query.to_string())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };

    if options.exact_match {
        text == query
    } else {
        text.contains(&query)
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
}
