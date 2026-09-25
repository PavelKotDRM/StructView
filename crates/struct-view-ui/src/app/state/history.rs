use super::*;

impl StructViewApp {
    pub(super) fn clear_history(&mut self) {
        self.undo_history.clear();
        self.redo_history.clear();
        self.pending_inline_edit = None;
        self.delete_requested = false;
        self.undo_requested = false;
        self.redo_requested = false;
    }

    pub(in crate::app) fn can_undo(&self) -> bool {
        self.mode == AppMode::Edit
            && self.comparison.is_none()
            && self.field_dialog.is_none()
            && self.root.is_some()
            && (!self.undo_history.is_empty() || self.pending_inline_edit.is_some())
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        self.mode == AppMode::Edit
            && self.comparison.is_none()
            && self.field_dialog.is_none()
            && self.root.is_some()
            && self.pending_inline_edit.is_none()
            && !self.redo_history.is_empty()
    }

    pub(in crate::app) fn undo(&mut self) {
        if self.mode != AppMode::Edit || self.comparison.is_some() || self.field_dialog.is_some() {
            return;
        }
        self.finalize_pending_inline_edit();
        if !self.can_undo() {
            return;
        }
        let Some(snapshot) = self.undo_history.pop() else {
            return;
        };
        let Some(current) = self.root.replace(snapshot) else {
            return;
        };
        push_limited_snapshot(&mut self.redo_history, current);
        self.refresh_after_history_navigation(TextKey::ActionUndone);
    }

    pub(in crate::app) fn redo(&mut self) {
        if !self.can_redo() {
            return;
        }
        let Some(snapshot) = self.redo_history.pop() else {
            return;
        };
        let Some(current) = self.root.replace(snapshot) else {
            return;
        };
        push_limited_snapshot(&mut self.undo_history, current);
        self.refresh_after_history_navigation(TextKey::ActionRedone);
    }

    pub(super) fn push_undo_snapshot(&mut self, snapshot: JsonNode) {
        push_limited_snapshot(&mut self.undo_history, snapshot);
        self.redo_history.clear();
    }

    pub(in crate::app) fn handle_inline_edit_events(
        &mut self,
        events: Vec<InlineEditEvent>,
    ) -> bool {
        let mut restored_invalid_edit = false;
        let mut finished_paths = BTreeSet::new();

        for event in &events {
            if event.finished
                && self
                    .pending_inline_edit
                    .as_ref()
                    .is_some_and(|pending| pending.path == event.path)
            {
                restored_invalid_edit |= self.finish_pending_inline_edit(event.valid);
                finished_paths.insert(event.path.clone());
            }
        }

        for event in events {
            if !event.changed || finished_paths.contains(&event.path) {
                continue;
            }
            if self
                .pending_inline_edit
                .as_ref()
                .is_some_and(|pending| pending.path != event.path)
            {
                restored_invalid_edit |= self.finish_pending_inline_edit(true);
            }
            if self.pending_inline_edit.is_none() {
                self.begin_inline_edit(&event);
            }
            if event.finished {
                restored_invalid_edit |= self.finish_pending_inline_edit(event.valid);
            }
        }

        restored_invalid_edit
    }

    fn begin_inline_edit(&mut self, event: &InlineEditEvent) {
        let Some(mut root_before) = self.root.clone() else {
            return;
        };
        let Some(node) = find_node_mut(&mut root_before, &event.path) else {
            return;
        };
        node.value_type = event.before_value_type.clone();
        node.display_value = event.before_display_value.clone();
        self.pending_inline_edit = Some(PendingInlineEdit {
            path: event.path.clone(),
            root_before,
        });
    }

    fn finish_pending_inline_edit(&mut self, valid: bool) -> bool {
        let Some(pending) = self.pending_inline_edit.take() else {
            return false;
        };
        let mut valid = valid;
        if valid {
            let result = self
                .root
                .as_mut()
                .and_then(|root| find_node_mut(root, &pending.path))
                .ok_or_else(|| "Не удалось найти inline-правку для завершения".to_string())
                .and_then(|node| {
                    let edited = node.display_value.clone();
                    apply_primitive_edit(node, &edited)
                });
            if let Err(error) = result {
                self.show_toast(&error);
                valid = false;
            }
        }
        let Some(before_node) = find_node(&pending.root_before, &pending.path) else {
            return false;
        };

        if !valid {
            if let Some(root) = self.root.as_mut()
                && let Some(node) = find_node_mut(root, &pending.path)
            {
                node.value_type = before_node.value_type.clone();
                node.display_value = before_node.display_value.clone();
            }
            return true;
        }

        let changed = self
            .root
            .as_ref()
            .and_then(|root| find_node(root, &pending.path))
            .is_some_and(|after_node| {
                after_node.value_type != before_node.value_type
                    || after_node.display_value != before_node.display_value
            });
        if changed {
            self.push_undo_snapshot(pending.root_before);
        }
        false
    }

    pub(in crate::app) fn finalize_pending_inline_edit(&mut self) {
        if self.pending_inline_edit.is_some() {
            self.finish_pending_inline_edit(true);
            self.visible_rows_dirty = true;
            self.invalidate_visualization_cache();
            self.refresh_search();
        }
    }

    fn refresh_after_history_navigation(&mut self, message: TextKey) {
        self.pending_inline_edit = None;
        self.selected_paths.clear();
        self.visible_rows_dirty = true;
        self.invalidate_visualization_cache();
        self.refresh_search();
        self.show_toast(self.locale.text(message));
    }
}
