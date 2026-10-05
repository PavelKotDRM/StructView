use super::*;

impl StructureView {
    #[cfg(test)]
    pub(super) fn search(&mut self) {
        let mut search = SearchState::default();
        self.search_dirty = true;
        self.sync_search(&mut search, &self.query.clone());
    }

    pub(in crate::app) fn has_document(&self) -> bool {
        self.document.is_some()
    }

    pub(in crate::app) fn sync_search(&mut self, search: &mut SearchState, query: &str) {
        let same_matches = self.document.as_ref().is_some_and(|doc| {
            search.matches.len() == self.matches.len()
                && search
                    .matches
                    .iter()
                    .zip(&self.matches)
                    .all(|(path, &id)| *path == doc.nodes[id].path)
        });
        if !self.search_dirty
            && self.query == query
            && self.search_options == search.options
            && same_matches
        {
            return;
        }
        self.query = query.to_string();
        self.search_options = search.options;
        self.search_dirty = false;
        search.search_fields(
            query,
            search.options,
            self.document
                .iter()
                .flat_map(|doc| doc.nodes.iter())
                .map(|node| {
                    (
                        Some(node.key.as_str()),
                        node.value.as_str(),
                        node.path.as_str(),
                    )
                }),
        );
        self.matches.clear();
        self.matched_paths.clear();
        self.match_index = 0;
        if let Some(doc) = &self.document {
            let matching: HashSet<_> = search.matches.iter().map(String::as_str).collect();
            for (id, node) in doc.nodes.iter().enumerate() {
                if matching.contains(node.path.as_str()) {
                    self.matches.push(id);
                    let mut cursor = Some(id);
                    while let Some(id) = cursor {
                        if !self.matched_paths.insert(id) {
                            break;
                        }
                        cursor = doc.nodes[id].parent;
                    }
                }
            }
        }
        self.reveal_search_match(search);
    }

    pub(in crate::app) fn reveal_search_match(&mut self, search: &SearchState) {
        self.match_index = search.current_index;
        if let Some(&id) = self.matches.get(self.match_index) {
            self.select(id, true);
        }
    }
}
