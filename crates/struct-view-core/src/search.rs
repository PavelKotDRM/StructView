//! # Structured-data tree search
//!
//! Searches [`JsonNode`] keys, displayed values, and paths, then stores matching
//! node paths for navigation. Queries can use literal matching, regular
//! expressions, or a `key: value` pair.

use crate::parser::JsonNode;

mod matcher;

use matcher::SearchPattern;

/// Escape a string so it is interpreted literally in a regular expression.
///
/// This is useful when constructing a regular expression that contains
/// user-provided literal text.
///
/// # Examples
///
/// ```
/// use struct_view_core::search::escape_regex_literal;
///
/// let literal = escape_regex_literal("a.b");
/// assert_eq!(literal, r"a\.b");
/// ```
pub fn escape_regex_literal(text: &str) -> String {
    regex::escape(text)
}

/// Controls which node fields are searched and how a query is matched.
///
/// By default, keys, displayed values, and paths are searched using
/// case-insensitive substring matching. Regular expressions use the syntax
/// supported by the [`regex`] crate. For regular-expression searches,
/// [`Self::exact_match`] and [`Self::whole_word`] do not apply; use regular
/// expression anchors or boundaries instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    /// Search object keys. Array elements have no key.
    pub search_keys: bool,
    /// Search each node's displayed value.
    pub search_values: bool,
    /// Search each node's path.
    pub search_paths: bool,
    /// Distinguish uppercase and lowercase characters in literal and regex searches.
    pub case_sensitive: bool,
    /// Require the entire field to equal a literal query instead of matching a substring.
    ///
    /// This option is ignored when [`Self::use_regex`] is enabled.
    pub exact_match: bool,
    /// Require a literal query to be bounded by non-word characters or field boundaries.
    ///
    /// Word characters are Unicode alphanumeric characters and underscores.
    /// This option is ignored when [`Self::use_regex`] is enabled.
    pub whole_word: bool,
    /// Interpret the query as a regular expression.
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

/// The current query, its matching node paths, and navigation state.
///
/// Call [`Self::search`] or [`Self::search_with_options`] to update this state.
/// Matches are stored in tree traversal order, and navigation wraps around at
/// either end of the list.
#[derive(Debug, Default, Clone)]
pub struct SearchState {
    /// The most recently submitted query.
    pub query: String,
    /// Paths of nodes matching the most recent valid query.
    pub matches: Vec<String>,
    /// Index of the active path in [`Self::matches`], or `0` when there are no matches.
    pub current_index: usize,
    /// Options used for the most recent search.
    pub options: SearchOptions,
    /// Error from compiling the most recent query, if any.
    ///
    /// For example, this is set when a regular-expression query is invalid.
    pub error: Option<String>,
    pattern: Option<SearchPattern>,
}

impl SearchState {
    /// Search the tree using the current [`Self::options`].
    ///
    /// Replaces [`Self::matches`], resets [`Self::current_index`] to `0`, and
    /// clears any previous [`Self::error`]. An empty query or disabled search
    /// scopes produce no matches. If a regular-expression query cannot be
    /// compiled, matches remain empty and the error is stored in [`Self::error`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root of the tree to search.
    /// * `query` - Query string. A `key: value` query requires whitespace
    ///   immediately before or after the colon; both parts must match the same
    ///   node's key and displayed value.
    ///
    /// # Examples
    ///
    /// ```
    /// use struct_view_core::parser::parse_json;
    /// use struct_view_core::search::SearchState;
    ///
    /// let root = parse_json(r#"{"user":{"name":"Ada"}}"#).unwrap();
    /// let mut search = SearchState::default();
    /// search.search(&root, "name: Ada");
    ///
    /// assert_eq!(search.matches, ["user.name"]);
    /// ```
    pub fn search(&mut self, root: &JsonNode, query: &str) {
        self.search_with_options(root, query, self.options);
    }

    /// Search the tree using the supplied options.
    ///
    /// Replaces the query, options, and matches, resets the active index to
    /// `0`, and clears any previous error. When `use_regex` is enabled, an
    /// invalid expression leaves the match list empty and its compilation
    /// error available in [`Self::error`]. If all searchable scopes are
    /// disabled or the query is empty, no matching is performed.
    ///
    /// A query of the form `key: value` (with whitespace on at least one side
    /// of the colon) requires both parts to match the same node. This form
    /// requires both key and value search scopes to be enabled.
    ///
    /// # Arguments
    ///
    /// * `root` - Root of the tree to search.
    /// * `query` - Literal query, regular expression, or key-value query.
    /// * `options` - Scopes and matching behavior for this search.
    ///
    /// # Examples
    ///
    /// ```
    /// use struct_view_core::parser::parse_json;
    /// use struct_view_core::search::{SearchOptions, SearchState};
    ///
    /// let root = parse_json(r#"{"user_name":"Ada"}"#).unwrap();
    /// let mut search = SearchState::default();
    /// search.search_with_options(
    ///     &root,
    ///     r"^user_[a-z]+$",
    ///     SearchOptions {
    ///         search_values: false,
    ///         search_paths: false,
    ///         use_regex: true,
    ///         ..Default::default()
    ///     },
    /// );
    ///
    /// assert_eq!(search.matches, ["user_name"]);
    /// ```
    pub fn search_with_options(&mut self, root: &JsonNode, query: &str, options: SearchOptions) {
        self.search_fields(query, options, tree_fields(root));
    }

    /// Search arbitrary source-ordered fields with the same options as the tree search.
    pub fn search_fields<'a>(
        &mut self,
        query: &str,
        options: SearchOptions,
        fields: impl IntoIterator<Item = (Option<&'a str>, &'a str, &'a str)>,
    ) {
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
        for (key, value, path) in fields {
            if pattern.matches_fields(key, value, path, options) {
                self.matches.push(path.to_string());
            }
        }
        self.pattern = Some(pattern);
    }

    /// Advance the active index to the next match.
    ///
    /// Navigation wraps from the last match to the first. Does nothing when
    /// [`Self::matches`] is empty.
    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.current_index = (self.current_index + 1) % self.matches.len();
        }
    }

    /// Move the active index to the previous match.
    ///
    /// Navigation wraps from the first match to the last. Does nothing when
    /// [`Self::matches`] is empty.
    pub fn prev(&mut self) {
        if !self.matches.is_empty() {
            self.current_index =
                (self.current_index + self.matches.len().saturating_sub(1)) % self.matches.len();
        }
    }

    /// Return the path at the active index, if a match exists.
    ///
    /// Returns `None` when [`Self::matches`] is empty or the index is out of range.
    pub fn current_match_path(&self) -> Option<&str> {
        self.matches.get(self.current_index).map(|s| s.as_str())
    }

    /// Check whether `path` is the active match.
    pub fn is_active(&self, path: &str) -> bool {
        self.matches
            .get(self.current_index)
            .is_some_and(|p| p == path)
    }

    /// Check whether `path` appears anywhere in the match list.
    pub fn is_match(&self, path: &str) -> bool {
        self.matches.iter().any(|p| p == path)
    }

    /// Test `text` against the most recently compiled query.
    ///
    /// Returns `false` if no valid query has been compiled. A key-value query
    /// cannot match a standalone text value; use [`Self::matches_fields`]
    /// instead.
    pub fn matches_text(&self, text: &str) -> bool {
        self.pattern
            .as_ref()
            .is_some_and(|pattern| pattern.matches_text(text))
    }

    /// Test a node's key, displayed value, and path against the current query.
    ///
    /// Only fields enabled by the most recently used [`SearchOptions`] are
    /// considered. For a key-value query, both the key and displayed value
    /// must match on this same node. Returns `false` if no valid query has
    /// been compiled.
    pub fn matches_fields(&self, key: Option<&str>, value: &str, path: &str) -> bool {
        self.pattern
            .as_ref()
            .is_some_and(|pattern| pattern.matches_fields(key, value, path, self.options))
    }
}

/// Collect the paths of all nodes matching the pattern and enabled scopes.
fn tree_fields(root: &JsonNode) -> impl Iterator<Item = (Option<&str>, &str, &str)> {
    let mut stack = vec![root];
    std::iter::from_fn(move || {
        let node = stack.pop()?;
        stack.extend(node.children.iter().rev());
        Some((
            node.key.as_deref(),
            node.display_value.as_str(),
            node.path.as_str(),
        ))
    })
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
