use super::*;
use egui::text::{LayoutJob, TextFormat};

impl StructureView {
    pub(super) fn show_source_window(&mut self, ctx: &egui::Context, locale: Locale) {
        let mut open = self.source_open;
        let format = self
            .document
            .as_ref()
            .map(|document| document.format)
            .or(self.format)
            .unwrap_or(DataFormat::Json);
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
                                    self.source_changed = true;
                                    self.source_preview = None;
                                    self.unsaved = true;
                                    self.set_editing(false);
                                }
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
                                        let previews = selected
                                            .iter()
                                            .map(|&id| self.selected_source_preview(id, format))
                                            .collect();
                                        self.source_preview = Some((selected.clone(), previews));
                                    }
                                    if let Some((_, previews)) = &self.source_preview {
                                        let paths = self
                                            .document
                                            .as_ref()
                                            .map(|document| {
                                                selected
                                                    .iter()
                                                    .filter_map(|&id| document.nodes.get(id))
                                                    .map(|node| node.path.clone())
                                                    .collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default();
                                        for (index, preview) in previews.iter().enumerate() {
                                            if let Some(path) = paths.get(index) {
                                                ui.label(
                                                    egui::RichText::new(path).weak().monospace(),
                                                );
                                            }
                                            match preview {
                                                Ok(preview) => {
                                                    let mut preview = preview.clone();
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

    fn source_layouter(
        format: DataFormat,
    ) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> {
        move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let colors = super::super::super::theme::SyntaxColors::new(ui.visuals());
            let mut job = Self::source_syntax_job(
                text.as_str(),
                format,
                colors,
                ui.style().text_styles[&egui::TextStyle::Monospace].size,
            );
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
    }

    pub(super) fn selected_source_preview(
        &self,
        selected: usize,
        format: DataFormat,
    ) -> Result<String, String> {
        let (path, node) = self.selected_source_node_at(selected)?;
        let preview_node = if path.is_empty() {
            node
        } else {
            struct_view_core::parser::JsonNode {
                key: None,
                yaml_key: None,
                value_type: struct_view_core::parser::JsonValueType::Object,
                display_value: "{1}".to_string(),
                children: vec![node],
                expanded: true,
                path: String::new(),
            }
        };
        struct_view_core::parser::serialize_node(&preview_node, format, false)
    }

    pub(in crate::app) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "JSON / JSON5 / YAML / TOML",
                &["json", "json5", "yaml", "yml", "toml"],
            )
            .pick_file()
        {
            self.open(path);
        }
    }

    pub(super) fn source_syntax_job(
        text: &str,
        format: DataFormat,
        colors: super::super::super::theme::SyntaxColors,
        font_size: f32,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = egui::FontId::monospace(font_size);
        let mut index = 0;

        while index < text.len() {
            let character = text[index..].chars().next().unwrap_or_default();
            let start = index;
            let color = if Self::is_comment_start(text, index, format) {
                index = Self::comment_end(text, index, format);
                colors.comment
            } else if matches!(character, '"' | '\'') {
                index = Self::quoted_end(text, index, character);
                let next = text[index..].trim_start();
                if next.starts_with(':') || (format == DataFormat::Toml && next.starts_with('=')) {
                    colors.key
                } else {
                    colors.string
                }
            } else if character.is_ascii_digit()
                || (character == '-'
                    && text[index + character.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(|next| next.is_ascii_digit()))
            {
                index = Self::consume_while(text, index, |character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '+' | '-')
                });
                colors.number
            } else if Self::is_word_start(character) {
                index = Self::consume_while(text, index, Self::is_word_continue);
                let word = &text[start..index];
                let next = text[index..].trim_start();
                if next.starts_with(':') || (format == DataFormat::Toml && next.starts_with('=')) {
                    colors.key
                } else {
                    match word {
                        "true" | "false" | "yes" | "no" | "on" | "off" => colors.boolean,
                        "null" | "Null" | "NULL" | "~" => colors.null,
                        _ => colors.string,
                    }
                }
            } else {
                index += character.len_utf8();
                colors.key
            };

            job.append(
                &text[start..index],
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                },
            );
        }

        job
    }

    fn is_comment_start(text: &str, index: usize, format: DataFormat) -> bool {
        let remaining = &text[index..];
        match format {
            DataFormat::Yaml | DataFormat::Toml => remaining.starts_with('#'),
            DataFormat::Json5 => {
                remaining.starts_with('#')
                    || remaining.starts_with("//")
                    || remaining.starts_with("/*")
            }
            _ => false,
        }
    }

    fn comment_end(text: &str, start: usize, format: DataFormat) -> usize {
        let remaining = &text[start..];
        if format == DataFormat::Json5 && remaining.starts_with("/*") {
            return remaining
                .find("*/")
                .map_or(text.len(), |offset| start + offset + 2);
        }
        remaining
            .find('\n')
            .map_or(text.len(), |offset| start + offset)
    }

    fn quoted_end(text: &str, start: usize, quote: char) -> usize {
        let mut escaped = false;
        for (offset, character) in text[start + quote.len_utf8()..].char_indices() {
            let index = start + quote.len_utf8() + offset;
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                return index + character.len_utf8();
            }
        }
        text.len()
    }

    fn consume_while(text: &str, start: usize, predicate: impl Fn(char) -> bool) -> usize {
        text[start..]
            .char_indices()
            .take_while(|(_, character)| predicate(*character))
            .last()
            .map_or(start, |(offset, character)| {
                start + offset + character.len_utf8()
            })
    }

    fn is_word_start(character: char) -> bool {
        character.is_alphabetic() || matches!(character, '_' | '$' | '~')
    }

    fn is_word_continue(character: char) -> bool {
        character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '$' | '~')
    }

    pub(in crate::app) fn has_unsaved_changes(&self) -> bool {
        self.unsaved
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
            Err(error) => {
                self.pending_origin = None;
                self.error = Some(error.to_string());
            }
        }
    }

    pub(super) fn open(&mut self, path: PathBuf) {
        self.source_changed = true;
        self.unsaved = false;
        self.pending_origin = Some(path.clone());
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
                            if let Some(path) = self.pending_origin.take() {
                                self.origin = Some(path);
                            }
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
                            self.pending_origin = None;
                            self.error = Some(error);
                            self.source_open = true;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.pending_origin = None;
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
                !self.source.is_empty()
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
                !self.source.is_empty(),
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
                self.source_changed |= changed;
                if changed {
                    self.set_editing(false);
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
