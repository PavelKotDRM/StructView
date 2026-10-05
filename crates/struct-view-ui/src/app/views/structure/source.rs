use super::*;

impl StructureView {
    pub(super) fn show_source_window(&mut self, ctx: &egui::Context, locale: Locale) {
        let mut open = self.source_open;
        egui::Window::new(locale.text(TextKey::StructureSource))
            .id(egui::Id::new("structure-source-window"))
            .open(&mut open)
            .default_size([600.0, 240.0])
            .show(ctx, |ui| {
                ui.label(locale.text(TextKey::StructureDisk));
                if let Some(path) = &self.origin {
                    ui.label(path.display().to_string());
                }
                ui.add_enabled_ui(self.pending.is_none(), |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height().max(100.0))
                        .id_salt("structure-source-scroll")
                        .show(ui, |ui| {
                            if ui
                                .add(
                                    egui::TextEdit::multiline(&mut self.source)
                                        .code_editor()
                                        .desired_rows(8)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(locale.text(TextKey::StructureSource)),
                                )
                                .changed()
                            {
                                self.source_changed = true;
                            }
                        });
                });
            });
        self.source_open = open;
    }

    pub(in crate::app) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON / YAML / TOML", &["json", "yaml", "yml", "toml"])
            .pick_file()
        {
            self.open(path);
        }
    }

    pub(super) fn start(&mut self, work: impl FnOnce() -> ParseResult + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("struct-view-data-structure".into())
            .spawn(move || {
                let _ = sender.send(work());
            }) {
            Ok(_) => {
                self.pending = Some(receiver);
                self.error = None;
                self.notice = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn open(&mut self, path: PathBuf) {
        self.source_changed = true;
        self.origin = Some(path.clone());
        self.format = DataFormat::from_path(&path);
        let format = self.format;
        self.start(move || {
            let source =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            parse_text(source, format)
        });
    }

    pub(super) fn poll(&mut self, ui: &Ui, locale: Locale) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok((source, result)) => {
                            self.source = source;
                            match result {
                                Ok(document) => {
                                    self.reset(&document);
                                    self.document = Some(document);
                                    self.source_open = false;
                                    self.source_changed = false;
                                }
                                Err(error) => {
                                    self.source_changed = true;
                                    self.error = Some(error.to_string());
                                    self.source_open = true;
                                }
                            }
                        }
                        Err(error) => {
                            self.error = Some(error);
                            self.source_open = true;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.error = Some(locale.text(TextKey::BackgroundOperationFailed).into());
                }
                Err(TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(40))
                }
            }
        }
        if let Some(receiver) = &self.export_pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.export_pending = None;
                    match result {
                        Ok(()) => self.notice = Some(locale.text(TextKey::GraphExported).into()),
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.export_pending = None;
                    self.error = Some(locale.text(TextKey::BackgroundOperationFailed).into());
                }
                Err(TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(40))
                }
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
                self.source_changed |= ui
                    .selectable_value(&mut self.format, None, locale.text(TextKey::StructureAuto))
                    .changed();
                for format in [DataFormat::Json, DataFormat::Yaml, DataFormat::Toml] {
                    self.source_changed |= ui
                        .selectable_value(&mut self.format, Some(format), format.to_string())
                        .changed();
                }
            });
        });
    }

    pub(in crate::app) fn view_menu(&mut self, ui: &mut Ui, locale: Locale) {
        ui.checkbox(&mut self.source_open, locale.text(TextKey::StructureSource));
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
                    DiagramAction::SelectAll => self.all_selected = true,
                    DiagramAction::ClearSelection => {
                        self.all_selected = false;
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
