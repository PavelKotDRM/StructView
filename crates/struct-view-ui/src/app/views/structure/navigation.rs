use super::*;

impl StructureView {
    pub(in crate::app) fn set_all_expanded(&mut self, expanded: bool) {
        if let Some(doc) = &self.document {
            self.collapsed = if expanded {
                HashSet::new()
            } else {
                doc.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| !node.children.is_empty())
                    .map(|(id, _)| id)
                    .collect()
            };
            self.select(0, false);
            self.dirty = true;
            self.fit = true;
            self.center_selected = false;
        }
    }

    pub(super) fn select(&mut self, id: usize, reveal: bool) {
        self.all_selected = false;
        self.selected = id;
        self.path.clear();
        if let Some(doc) = &self.document {
            let mut cursor = Some(id);
            while let Some(id) = cursor {
                self.path.insert(id);
                if let Some(parent) = doc.nodes[id].parent
                    && reveal
                {
                    self.collapsed.remove(&parent);
                    let index = doc.nodes[parent]
                        .children
                        .iter()
                        .position(|&child| child == id)
                        .unwrap_or(0);
                    let limit = self.limits.entry(parent).or_insert(100);
                    *limit = (*limit).max(index + 1);
                }
                cursor = doc.nodes[id].parent;
            }
        }
        if reveal {
            self.dirty = true;
            self.center_selected = true;
        }
    }

    pub(super) fn toggle(&mut self, id: usize) {
        if self
            .document
            .as_ref()
            .is_none_or(|doc| doc.nodes[id].children.is_empty())
        {
            return;
        }
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.select(id, false);
        self.dirty = true;
        self.center_selected = true;
    }

    pub(super) fn zoom_by(&mut self, factor: f32, anchor: Pos2) {
        super::super::diagram::zoom_at(&mut self.zoom, &mut self.pan, factor, anchor);
    }

    pub(super) fn ensure_layout(&mut self) {
        if !self.dirty {
            return;
        }
        if let Some(doc) = &self.document {
            self.layout = layout::build(doc, &self.collapsed, &self.limits, self.direction);
        }
        self.dirty = false;
    }

    pub(super) fn keyboard(&mut self, ui: &Ui) {
        let pressed = |key| ui.input(|i| i.key_pressed(key));
        if pressed(egui::Key::Home) {
            self.fit = true;
        }
        self.zoom_by(
            super::super::diagram::keyboard_zoom(ui),
            (self.canvas_size * 0.5).to_pos2(),
        );
        if super::super::diagram::keyboard_activate(ui) {
            self.toggle(self.selected);
        }
        if pressed(egui::Key::ArrowDown) || pressed(egui::Key::ArrowUp) {
            let index = self
                .layout
                .nodes
                .iter()
                .position(|(id, _)| *id == self.selected)
                .unwrap_or(0);
            let next = if pressed(egui::Key::ArrowDown) {
                (index + 1).min(self.layout.nodes.len().saturating_sub(1))
            } else {
                index.saturating_sub(1)
            };
            if let Some(&(id, _)) = self.layout.nodes.get(next) {
                self.select(id, true);
            }
        }
        if pressed(egui::Key::ArrowLeft) {
            if !self.collapsed.contains(&self.selected)
                && self
                    .document
                    .as_ref()
                    .is_some_and(|d| !d.nodes[self.selected].children.is_empty())
            {
                self.toggle(self.selected);
            } else if let Some(parent) = self
                .document
                .as_ref()
                .and_then(|d| d.nodes[self.selected].parent)
            {
                self.select(parent, true);
            }
        }
        if pressed(egui::Key::ArrowRight) {
            if self.collapsed.contains(&self.selected) {
                self.toggle(self.selected);
            } else if let Some(child) = self
                .document
                .as_ref()
                .and_then(|d| d.nodes[self.selected].children.first().copied())
            {
                self.select(child, true);
            }
        }
    }
}
