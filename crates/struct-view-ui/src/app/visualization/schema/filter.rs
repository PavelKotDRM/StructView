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
                    && row
                        .key
                        .as_deref()
                        .is_some_and(|key| search.matches_text(key));
                let path_match = search.options.search_paths && search.matches_text(&row.path);
                let value_match = search.options.search_values
                    && [
                        row.type_name.as_str(),
                        row.constraints.as_str(),
                        row.reference.as_deref().unwrap_or_default(),
                    ]
                    .iter()
                    .any(|text| search.matches_text(text));
                (key_match || path_match || value_match).then_some(index)
            })
            .collect(),
    )
}
