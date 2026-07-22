//! # Модуль поиска по JSON-дереву
//!
//! Реализует полнотекстовый поиск по ключам и значениям узлов дерева [`JsonNode`].
//! Найденные совпадения сохраняются как список путей, по которым можно
//! навигировать (Next / Previous).

use crate::parser::JsonNode;

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
}

impl SearchState {
    /// Выполнить поиск по дереву.
    ///
    /// Обновляет список [`Self::matches`] и сбрасывает [`Self::current_index`] в `0`.
    /// Поиск регистронезависимый; проверяются ключ и отображаемое значение каждого узла.
    ///
    /// # Arguments
    ///
    /// * `root` — корневой узел JSON-дерева.
    /// * `query` — строка поиска.
    pub fn search(&mut self, root: &JsonNode, query: &str) {
        self.query = query.to_string();
        self.matches.clear();
        self.current_index = 0;
        if query.is_empty() {
            return;
        }
        collect_matches(root, &query.to_lowercase(), &mut self.matches);
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

/// Рекурсивно собрать пути всех узлов, ключ или значение которых содержит `query`.
fn collect_matches(node: &JsonNode, query: &str, result: &mut Vec<String>) {
    let key_match = node
        .key
        .as_deref()
        .is_some_and(|k| k.to_lowercase().contains(query));
    let value_match = node.display_value.to_lowercase().contains(query);

    if key_match || value_match {
        result.push(node.path.clone());
    }

    for child in &node.children {
        collect_matches(child, query, result);
    }
}
