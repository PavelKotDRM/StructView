use super::*;

impl StructViewApp {
    pub(in crate::app) fn retain_valid_selected_paths(&mut self) {
        if let Some(root) = &self.root {
            self.selected_paths
                .retain(|path| find_node(root, path).is_some());
        } else {
            self.selected_paths.clear();
        }
    }

    /// Пересчитать результаты поиска по текущему запросу.
    ///
    /// Вызывается после правки дерева, чтобы подсветка оставалась актуальной.
    pub(in crate::app) fn refresh_search(&mut self) {
        self.search_scroll_target = None;
        if self.visualization == VisualizationMode::Structure {
            self.structure_view
                .sync_search(&mut self.search, &self.search_query_buf);
            return;
        }
        let Some(root) = &self.root else {
            return;
        };

        let query = self.search_query_buf.clone();
        self.search.search(root, &query);
    }

    /// Сбросить производные модели после изменения документа.
    pub(in crate::app) fn invalidate_visualization_cache(&mut self) {
        self.visualization_cache = VisualizationCache::default();
        self.graph_calculation = crate::app::views::GraphCalculationState::default();
    }

    /// Запланировать прокрутку к текущему совпадению, если оно существует.
    pub(in crate::app) fn request_search_scroll(&mut self) {
        if self.visualization == VisualizationMode::Structure {
            self.structure_view.reveal_search_match(&self.search);
            return;
        }
        self.search_scroll_target = self.search.current_match_path().map(str::to_owned);
    }
}
