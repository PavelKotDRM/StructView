use struct_view_core::search::SearchState;

use super::SchemaDiagram;

/// Вернуть строки схемы, подходящие под текущий поисковый запрос.
pub(in crate::app) fn schema_visible_indices(
    diagram: &SchemaDiagram,
    search: &SearchState,
) -> Option<Vec<usize>> {
    if search.query.is_empty() {
        return None;
    }

    Some(
        diagram
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let key_match = search.options.search_keys
                    && text_matches(
                        &row.path,
                        &search.query,
                        search.options.case_sensitive,
                        search.options.exact_match,
                    );
                let value_match = search.options.search_values
                    && [
                        row.type_name.as_str(),
                        row.constraints.as_str(),
                        row.reference.as_deref().unwrap_or_default(),
                    ]
                    .iter()
                    .any(|text| {
                        text_matches(
                            text,
                            &search.query,
                            search.options.case_sensitive,
                            search.options.exact_match,
                        )
                    });
                (key_match || value_match).then_some(index)
            })
            .collect(),
    )
}

fn text_matches(text: &str, query: &str, case_sensitive: bool, exact_match: bool) -> bool {
    let (text, query) = if case_sensitive {
        (text.to_string(), query.to_string())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    if exact_match {
        text == query
    } else {
        text.contains(&query)
    }
}
