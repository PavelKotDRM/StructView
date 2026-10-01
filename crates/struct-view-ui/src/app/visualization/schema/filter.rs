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
                let matches = [
                    row.type_name.as_str(),
                    row.constraints.as_str(),
                    row.reference.as_deref().unwrap_or_default(),
                ]
                .iter()
                .any(|value| search.matches_fields(row.key.as_deref(), value, &row.path));
                matches.then_some(index)
            })
            .collect(),
    )
}
