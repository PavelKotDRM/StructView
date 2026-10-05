use regex::{Regex, RegexBuilder};
use std::borrow::Cow;

use super::SearchOptions;

#[derive(Debug, Clone)]
pub(super) enum SearchMatcher {
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
pub(super) enum SearchPattern {
    Text(SearchMatcher),
    KeyValue {
        key: SearchMatcher,
        value: SearchMatcher,
    },
}

impl SearchPattern {
    pub(super) fn new(query: &str, options: SearchOptions) -> Result<Self, String> {
        if let Some((key, value)) = split_key_value_query(query) {
            return Ok(Self::KeyValue {
                key: SearchMatcher::new(key, options)?,
                value: SearchMatcher::new(value, options)?,
            });
        }

        SearchMatcher::new(query, options).map(Self::Text)
    }

    pub(super) fn matches_text(&self, text: &str) -> bool {
        match self {
            Self::Text(pattern) => pattern.is_match(text),
            Self::KeyValue { .. } => false,
        }
    }

    pub(super) fn matches_fields(
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
