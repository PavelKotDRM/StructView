use super::*;

mod loading;
mod syntax;
#[cfg(test)]
mod tests;

impl StructureView {
    pub(super) fn show_source_window(&mut self, ctx: &egui::Context, locale: Locale) {
        let mut open = self.source_open;
        let format = self.source_format();
        egui::Window::new(locale.text(TextKey::StructureSource))
            .id(egui::Id::new("structure-source-window"))
            .open(&mut open)
            .default_size([600.0, 240.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            !self.source_show_full,
                            locale.text(TextKey::StructureSelectedSource),
                        )
                        .clicked()
                    {
                        self.source_show_full = false;
                    }
                    if ui
                        .selectable_label(
                            self.source_show_full,
                            locale.text(TextKey::StructureFullSource),
                        )
                        .clicked()
                    {
                        self.source_show_full = true;
                    }
                });
                ui.label(locale.text(TextKey::StructureDisk));
                if let Some(path) = &self.origin {
                    ui.label(path.display().to_string());
                }
                ui.add_enabled_ui(self.pending.is_none(), |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height().max(100.0))
                        .id_salt("structure-source-scroll")
                        .show(ui, |ui| {
                            if self.source_show_full {
                                let mut layouter = Self::source_layouter(format);
                                if ui
                                    .add(
                                        egui::TextEdit::multiline(&mut self.source)
                                            .code_editor()
                                            .desired_rows(8)
                                            .desired_width(f32::INFINITY)
                                            .layouter(&mut layouter)
                                            .hint_text(locale.text(TextKey::StructureSource)),
                                    )
                                    .changed()
                                {
                                    self.mark_source_changed();
                                    self.unsaved = true;
                                }
                            } else if self.source_changed {
                                ui.label(locale.text(TextKey::StructureStale));
                            } else {
                                let mut selected =
                                    self.selected_nodes.iter().copied().collect::<Vec<_>>();
                                selected.sort_unstable();
                                if selected.is_empty() {
                                    ui.label(locale.text(TextKey::SelectStructure));
                                } else {
                                    if self.source_preview.as_ref().is_none_or(
                                        |(cached_selection, _)| *cached_selection != selected,
                                    ) {
                                        let previews =
                                            self.selected_source_previews(&selected, format);
                                        self.source_preview = Some((selected.clone(), previews));
                                    }
                                    if let Some((_, previews)) = &self.source_preview {
                                        for (index, preview) in previews.iter().enumerate() {
                                            if let Some(node) =
                                                self.document.as_ref().and_then(|document| {
                                                    document.nodes.get(selected[index])
                                                })
                                            {
                                                ui.label(
                                                    egui::RichText::new(&node.path)
                                                        .weak()
                                                        .monospace(),
                                                );
                                            }
                                            match preview {
                                                Ok(preview) => {
                                                    let mut preview = preview.as_str();
                                                    let mut layouter =
                                                        Self::source_layouter(format);
                                                    ui.add(
                                                        egui::TextEdit::multiline(&mut preview)
                                                            .code_editor()
                                                            .desired_rows(4)
                                                            .desired_width(f32::INFINITY)
                                                            .layouter(&mut layouter)
                                                            .interactive(false),
                                                    );
                                                }
                                                Err(error) => {
                                                    ui.colored_label(
                                                        ui.visuals().error_fg_color,
                                                        error,
                                                    );
                                                }
                                            }
                                            if index + 1 < previews.len() {
                                                ui.separator();
                                            }
                                        }
                                    }
                                }
                            }
                        });
                });
            });
        self.source_open = open;
    }

    fn source_format(&self) -> DataFormat {
        self.format
            .or_else(|| self.document.as_ref().map(|document| document.format))
            .unwrap_or(DataFormat::Json)
    }

    fn mark_source_changed(&mut self) {
        self.source_changed = true;
        self.source_preview = None;
        self.set_editing(false);
    }

    fn can_save_source(&self) -> bool {
        self.pending.is_none() && (self.origin.is_some() || self.unsaved || !self.source.is_empty())
    }

    pub(super) fn selected_source_previews(
        &self,
        selected: &[usize],
        format: DataFormat,
    ) -> Vec<Result<String, String>> {
        let root = match self.source_root() {
            Ok(root) => root,
            Err(error) => return vec![Err(error); selected.len()],
        };
        selected
            .iter()
            .map(|&id| {
                let node = self.selected_source_node_in(id, &root)?;
                if node.path.is_empty() {
                    struct_view_core::parser::serialize_node(node, format, false)
                } else {
                    let preview_node = struct_view_core::parser::JsonNode {
                        key: None,
                        yaml_key: None,
                        value_type: JsonValueType::Object,
                        display_value: "{1}".to_string(),
                        children: vec![node.clone()],
                        expanded: true,
                        path: String::new(),
                    };
                    struct_view_core::parser::serialize_node(&preview_node, format, false)
                }
            })
            .collect()
    }

    pub(in crate::app) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "JSON / JSON5 / YAML / TOML",
                &["json", "json5", "yaml", "yml", "toml"],
            )
            .pick_file()
        {
            self.requested_open = Some(path);
        }
    }

    pub(in crate::app) fn has_unsaved_changes(&self) -> bool {
        self.unsaved
    }

    pub(in crate::app) fn source_fingerprint(&self) -> u64 {
        crate::app::state::content_fingerprint(&self.source)
    }

    pub(in crate::app) fn save_changes(&mut self, locale: Locale) -> bool {
        self.save_source(locale)
    }

    fn save_source(&mut self, locale: Locale) -> bool {
        if let Some(path) = self.origin.clone() {
            self.write_source(&path, locale)
        } else {
            self.save_source_as(locale)
        }
    }

    fn save_source_as(&mut self, locale: Locale) -> bool {
        let mut dialog = rfd::FileDialog::new().add_filter(
            "JSON / JSON5 / YAML / TOML",
            &["json", "json5", "yaml", "yml", "toml"],
        );
        if let Some(format) = self.format {
            dialog = dialog.set_file_name(format!("data.{}", format.extension()));
        }
        if let Some(path) = dialog.save_file() {
            self.write_source(&path, locale)
        } else {
            false
        }
    }

    pub(super) fn write_source(&mut self, path: &Path, locale: Locale) -> bool {
        match struct_view_core::files::write_text_atomic(path, &self.source) {
            Ok(()) => {
                self.origin = Some(path.to_path_buf());
                self.unsaved = false;
                self.notice = Some(locale.text(TextKey::FileSaved).to_string());
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    pub(super) fn reset(&mut self, doc: &Document) {
        self.collapsed.clear();
        self.limits.clear();
        for (id, node) in doc.nodes.iter().enumerate() {
            let mut depth = 0;
            let mut parent = node.parent;
            while let Some(id) = parent {
                depth += 1;
                parent = doc.nodes[id].parent;
            }
            if !node.children.is_empty() && (depth >= 2 || node.children.len() > 12) {
                self.collapsed.insert(id);
            }
        }
        self.selected = 0;
        self.selected_nodes.clear();
        self.selected_nodes.insert(0);
        self.source_preview = None;
        self.path = HashSet::from([0]);
        self.query.clear();
        self.matches.clear();
        self.matched_paths.clear();
        self.search_dirty = true;
        self.all_selected = false;
        self.dirty = true;
        self.fit = true;
        self.center_selected = false;
    }

    pub(in crate::app) fn file_menu(&mut self, ui: &mut Ui, locale: Locale) {
        if ui
            .add_enabled(
                self.can_save_source()
                    && (self.origin.is_some() || self.unsaved || self.source_changed),
                egui::Button::new(locale.text(TextKey::Save)),
            )
            .clicked()
        {
            self.save_source(locale);
            ui.close();
        }
        if ui
            .add_enabled(
                self.can_save_source(),
                egui::Button::new(locale.text(TextKey::SaveAs)),
            )
            .clicked()
        {
            self.save_source_as(locale);
            ui.close();
        }
        if ui
            .add_enabled(
                self.pending.is_none(),
                egui::Button::new(locale.text(TextKey::StructureParse)),
            )
            .clicked()
        {
            let source = self.source.clone();
            let format = self.format;
            self.start(move || parse_text(source, format));
            ui.close();
        }
    }

    pub(in crate::app) fn export_menu(&mut self, ui: &mut Ui, locale: Locale) {
        ui.add_enabled_ui(
            self.document.is_some() && self.export_pending.is_none(),
            |ui| {
                if let Some((png, dark)) = export_controls(ui, locale) {
                    self.export(png, dark);
                }
            },
        );
    }

    pub(in crate::app) fn settings_menu(&mut self, ui: &mut Ui, locale: Locale) {
        ui.menu_button(locale.text(TextKey::StructureFormat), |ui| {
            ui.add_enabled_ui(self.pending.is_none(), |ui| {
                let mut changed = ui
                    .selectable_value(&mut self.format, None, locale.text(TextKey::StructureAuto))
                    .changed();
                for format in [
                    DataFormat::Json,
                    DataFormat::Json5,
                    DataFormat::Yaml,
                    DataFormat::Toml,
                ] {
                    changed |= ui
                        .selectable_value(&mut self.format, Some(format), format.to_string())
                        .changed();
                }
                if changed {
                    self.mark_source_changed();
                }
            });
        });
    }

    pub(in crate::app) fn view_menu(&mut self, ui: &mut Ui, locale: Locale) {
        ui.checkbox(&mut self.source_open, locale.text(TextKey::StructureSource));
        if self.editing {
            ui.add_enabled_ui(self.can_edit(), |ui| {
                if self
                    .document
                    .as_ref()
                    .and_then(|doc| doc.nodes.get(self.selected))
                    .is_some_and(|node| {
                        node.parent.is_some() && self.selected_nodes.contains(&self.selected)
                    })
                {
                    if ui.button(locale.text(TextKey::EditField)).clicked() {
                        self.open_edit_dialog();
                    }
                    if ui
                        .button(locale.text(TextKey::DeleteSelectedStructures))
                        .clicked()
                    {
                        self.delete_selected(locale);
                    }
                }
                if let Some(node) = self
                    .document
                    .as_ref()
                    .and_then(|doc| doc.nodes.get(self.selected))
                    .filter(|_| self.selected_nodes.contains(&self.selected))
                    && matches!(node.kind, Kind::Object | Kind::Array)
                {
                    let label = if node.kind == Kind::Object {
                        TextKey::AddField
                    } else {
                        TextKey::AddElement
                    };
                    if ui.button(locale.text(label)).clicked() {
                        self.open_add_dialog();
                    }
                }
            });
        }
        ui.add_enabled_ui(self.document.is_some(), |ui| {
            ui.menu_button(locale.text(TextKey::StructureLayout), |ui| {
                for (direction, key) in [
                    (Direction::Vertical, TextKey::StructureVertical),
                    (Direction::Horizontal, TextKey::StructureHorizontal),
                    (Direction::Compact, TextKey::StructureCompact),
                ] {
                    if ui
                        .selectable_value(&mut self.direction, direction, locale.text(key))
                        .changed()
                    {
                        self.dirty = true;
                        self.fit = true;
                        ui.close();
                    }
                }
            });
            for action in view_controls(ui, locale, self.zoom) {
                match action {
                    DiagramAction::ZoomOut => self.zoom_by(
                        1.0 / super::super::diagram::ZOOM_STEP,
                        (self.canvas_size * 0.5).to_pos2(),
                    ),
                    DiagramAction::ZoomIn => self.zoom_by(
                        super::super::diagram::ZOOM_STEP,
                        (self.canvas_size * 0.5).to_pos2(),
                    ),
                    DiagramAction::ActualSize => {
                        self.zoom_by(1.0 / self.zoom, (self.canvas_size * 0.5).to_pos2())
                    }
                    DiagramAction::Fit => self.fit = true,
                    DiagramAction::SelectAll => {
                        self.all_selected = true;
                        self.selected_nodes = (0..self
                            .document
                            .as_ref()
                            .map_or(0, |document| document.nodes.len()))
                            .collect();
                        self.source_preview = None;
                    }
                    DiagramAction::ClearSelection => {
                        self.all_selected = false;
                        self.selected_nodes.clear();
                        self.source_preview = None;
                        self.path.clear();
                    }
                }
            }
            if ui.button(locale.text(TextKey::StructureToggle)).clicked() {
                self.toggle(self.selected);
            }
            if let Some(doc) = &self.document {
                let count = doc.nodes[self.selected].children.len();
                let limit = *self.limits.get(&self.selected).unwrap_or(&100);
                if !self.collapsed.contains(&self.selected)
                    && count > limit
                    && ui
                        .button(format!(
                            "{} ({limit}/{count})",
                            locale.text(TextKey::StructureMore)
                        ))
                        .clicked()
                {
                    self.limits.insert(self.selected, (limit + 100).min(count));
                    self.dirty = true;
                    self.center_selected = true;
                }
            }
        });
    }
}
