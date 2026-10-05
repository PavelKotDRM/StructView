use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use super::diagram::{DiagramAction, export_controls, view_controls};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use struct_view_core::parser::{DataFormat, ParseError};
use struct_view_core::search::{SearchOptions, SearchState};
use struct_view_core::structure::{Document, Kind, Node};

use super::super::i18n::{Locale, TextKey};

mod export;
mod layout;
#[cfg(test)]
mod tests;

use layout::{Direction, Layout};

type ParseResult = Result<(String, Result<Document, ParseError>), String>;

const KEY_COLOR: Color32 = Color32::from_rgb(255, 218, 115);

pub(in crate::app) fn show_legend(ui: &mut Ui, locale: Locale) {
    for (kind, label) in [
        (Kind::Object, TextKey::LegendObject),
        (Kind::Array, TextKey::LegendArray),
        (Kind::String, TextKey::LegendString),
        (Kind::Number, TextKey::LegendNumber),
        (Kind::Bool, TextKey::LegendBool),
        (Kind::Null, TextKey::LegendNull),
        (Kind::Date, TextKey::LegendDate),
        (Kind::Reference, TextKey::LegendAlias),
    ] {
        legend_entry(ui, kind_color(kind), locale.text(label));
    }
    legend_entry(ui, KEY_COLOR, locale.text(TextKey::LegendKey));
}

fn legend_entry(ui: &mut Ui, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
        ui.painter().rect_filled(rect, 3.0, color);
        ui.painter().rect_stroke(
            rect,
            3.0,
            ui.visuals().widgets.noninteractive.bg_stroke,
            StrokeKind::Inside,
        );
        ui.label(label);
    });
}

pub(in crate::app) struct StructureView {
    source: String,
    source_open: bool,
    source_changed: bool,
    format: Option<DataFormat>,
    imported_path: Option<PathBuf>,
    origin: Option<PathBuf>,
    pending: Option<Receiver<ParseResult>>,
    export_pending: Option<Receiver<Result<(), String>>>,
    document: Option<Document>,
    error: Option<String>,
    notice: Option<String>,
    collapsed: HashSet<usize>,
    limits: HashMap<usize, usize>,
    direction: Direction,
    layout: Layout,
    dirty: bool,
    zoom: f32,
    pan: Vec2,
    canvas_size: Vec2,
    fit: bool,
    selected: usize,
    path: HashSet<usize>,
    query: String,
    matches: Vec<usize>,
    matched_paths: HashSet<usize>,
    match_index: usize,
    search_options: SearchOptions,
    search_dirty: bool,
    all_selected: bool,
    center_selected: bool,
}

impl Default for StructureView {
    fn default() -> Self {
        Self {
            source: String::new(),
            source_open: false,
            source_changed: false,
            format: None,
            imported_path: None,
            origin: None,
            pending: None,
            export_pending: None,
            document: None,
            error: None,
            notice: None,
            collapsed: HashSet::new(),
            limits: HashMap::new(),
            direction: Direction::default(),
            layout: Layout::default(),
            dirty: true,
            zoom: 1.0,
            pan: Vec2::ZERO,
            canvas_size: Vec2::ZERO,
            fit: true,
            selected: 0,
            path: HashSet::new(),
            query: String::new(),
            matches: Vec::new(),
            matched_paths: HashSet::new(),
            match_index: 0,
            search_options: SearchOptions::default(),
            search_dirty: true,
            all_selected: false,
            center_selected: false,
        }
    }
}

impl StructureView {
    pub(in crate::app) fn show(
        &mut self,
        ui: &mut Ui,
        locale: Locale,
        current_path: Option<&Path>,
    ) {
        if self.imported_path.as_deref() != current_path {
            self.imported_path = current_path.map(Path::to_path_buf);
            if let Some(path) = current_path {
                self.open(path.to_path_buf());
            }
        }
        let dropped = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        match dropped.as_slice() {
            [] => {}
            [path] => self.open(path.clone()),
            _ => self.error = Some(locale.text(TextKey::StructureSingleFile).into()),
        }
        self.poll(ui, locale);
        self.ensure_layout();
        self.show_source_window(ui.ctx(), locale);
        if self.pending.is_some() || self.export_pending.is_some() {
            ui.spinner();
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(notice) = &self.notice {
            ui.label(notice);
        }
        if self.source_changed && self.document.is_some() {
            ui.label(locale.text(TextKey::StructureStale));
        }
        self.canvas(ui, locale);
    }

    fn show_source_window(&mut self, ctx: &egui::Context, locale: Locale) {
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

    fn start(&mut self, work: impl FnOnce() -> ParseResult + Send + 'static) {
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

    fn open(&mut self, path: PathBuf) {
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

    fn poll(&mut self, ui: &Ui, locale: Locale) {
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

    fn reset(&mut self, doc: &Document) {
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
                        1.0 / super::diagram::ZOOM_STEP,
                        (self.canvas_size * 0.5).to_pos2(),
                    ),
                    DiagramAction::ZoomIn => self.zoom_by(
                        super::diagram::ZOOM_STEP,
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

    #[cfg(test)]
    fn search(&mut self) {
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

    fn select(&mut self, id: usize, reveal: bool) {
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

    fn toggle(&mut self, id: usize) {
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

    fn zoom_by(&mut self, factor: f32, anchor: Pos2) {
        super::diagram::zoom_at(&mut self.zoom, &mut self.pan, factor, anchor);
    }

    fn ensure_layout(&mut self) {
        if !self.dirty {
            return;
        }
        if let Some(doc) = &self.document {
            self.layout = layout::build(doc, &self.collapsed, &self.limits, self.direction);
        }
        self.dirty = false;
    }

    fn canvas(&mut self, ui: &mut Ui, locale: Locale) {
        let (_, viewport) = ui.allocate_space(ui.available_size().max(Vec2::splat(1.0)));
        let response = ui.interact(
            viewport,
            ui.id().with("structure-canvas"),
            Sense::click_and_drag(),
        );
        self.canvas_size = viewport.size();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                locale.text(TextKey::StructureView),
            )
        });
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.dragged() {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if response.contains_pointer() {
            let factor = super::diagram::wheel_zoom(ui);
            if let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
                && factor != 1.0
            {
                self.zoom_by(factor, pointer - viewport.min.to_vec2());
            }
        }
        if response.has_focus() {
            self.keyboard(ui);
        }
        self.ensure_layout();
        if self.fit {
            super::diagram::fit_to(
                &mut self.zoom,
                &mut self.pan,
                viewport.size(),
                self.layout.size,
            );
            self.fit = false;
        }
        if self.center_selected {
            if let Some((_, rect)) = self
                .layout
                .nodes
                .iter()
                .find(|(id, _)| *id == self.selected)
            {
                self.pan = viewport.size() * 0.5 - rect.center().to_vec2() * self.zoom;
            }
            self.center_selected = false;
        }
        let painter = ui.painter_at(viewport);
        let transform = |point: Pos2| viewport.min + self.pan + point.to_vec2() * self.zoom;
        for &(parent, child, points) in &self.layout.edges {
            let points = points.map(transform);
            if !Rect::from_points(&points).expand(3.0).intersects(viewport) {
                continue;
            }
            let highlighted = self.all_selected
                || (self.path.contains(&parent) && self.path.contains(&child))
                || (self.matched_paths.contains(&parent) && self.matched_paths.contains(&child));
            painter.add(egui::Shape::line(
                points.to_vec(),
                Stroke::new(
                    if highlighted { 2.5 } else { 1.3 },
                    if highlighted {
                        Color32::from_rgb(240, 170, 40)
                    } else {
                        ui.visuals().text_color()
                    },
                ),
            ));
        }
        let Some(doc) = &self.document else {
            return;
        };
        let mut clicked = None;
        let mut focused = None;
        for &(id, rect) in &self.layout.nodes {
            let rect = Rect::from_min_max(transform(rect.min), transform(rect.max));
            if !rect.intersects(viewport) {
                continue;
            }
            let node = &doc.nodes[id];
            let color = kind_color(node.kind);
            painter.rect_filled(
                rect,
                if node.kind == Kind::Array {
                    14.0 * self.zoom
                } else {
                    5.0 * self.zoom
                },
                Color32::from_rgb(23, 29, 40),
            );
            let highlight =
                self.all_selected || self.path.contains(&id) || self.matched_paths.contains(&id);
            painter.rect_stroke(
                rect,
                5.0 * self.zoom,
                Stroke::new(
                    if id == self.selected { 3.0 } else { 1.5 },
                    if highlight {
                        Color32::from_rgb(255, 198, 64)
                    } else {
                        color
                    },
                ),
                StrokeKind::Inside,
            );
            let heading = format!(
                "{} {}",
                if node.children.is_empty() {
                    ""
                } else if self.collapsed.contains(&id) {
                    "+"
                } else {
                    "-"
                },
                shorten(&node.key, 24)
            );
            painter.text(
                rect.center_top() + Vec2::new(0.0, 8.0 * self.zoom),
                Align2::CENTER_TOP,
                heading,
                FontId::proportional(14.0 * self.zoom),
                KEY_COLOR,
            );
            painter.text(
                rect.center_bottom() - Vec2::new(0.0, 10.0 * self.zoom),
                Align2::CENTER_BOTTOM,
                node_label(
                    node,
                    self.collapsed.contains(&id),
                    self.limits.get(&id).copied().unwrap_or(100),
                ),
                FontId::proportional(12.0 * self.zoom),
                color,
            );
            let hit = ui.interact(
                rect.intersect(viewport),
                ui.id().with(("structure-node", id)),
                Sense::click(),
            );
            hit.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    true,
                    self.all_selected || id == self.selected,
                    format!(
                        "{}: {} ({}, {})",
                        node.key,
                        node.value,
                        kind_name(node.kind, locale),
                        node.path
                    ),
                )
            });
            if !node.children.is_empty() {
                ui.ctx().accesskit_node_builder(hit.id, |item| {
                    item.set_expanded(!self.collapsed.contains(&id));
                    item.set_description(locale.text(TextKey::StructureToggle));
                });
            }
            let hit = hit.on_hover_text(format!("{}: {}\n{}", node.key, node.value, node.path));
            if hit.has_focus() {
                focused = Some(id);
            }
            if hit.clicked() && !super::diagram::keyboard_activate(ui) {
                clicked = Some(id);
                response.request_focus();
            }
        }
        if let Some(id) = clicked {
            self.select(id, false);
            self.toggle(id);
            ui.ctx().request_repaint();
        }
        if let Some(id) = focused {
            self.select(id, false);
            self.keyboard(ui);
            if ui.input(|i| {
                [
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::Enter,
                    egui::Key::Space,
                ]
                .iter()
                .any(|&key| i.key_pressed(key))
            }) {
                response.request_focus();
            }
        }
    }

    pub(in crate::app) fn status_summary(&mut self, locale: Locale) -> Option<Vec<String>> {
        self.ensure_layout();
        let doc = self.document.as_ref()?;
        let mut fields = Vec::new();
        if let Some(path) = &self.origin
            && let Some(name) = path.file_name()
        {
            fields.push(format!("📄 {}", name.to_string_lossy()));
        }
        fields.extend([
            doc.format.to_string(),
            format!(
                "{} / {} {}",
                doc.nodes.len(),
                self.layout.nodes.len(),
                locale.text(TextKey::StructureNodes)
            ),
            format!("{:.0}%", self.zoom * 100.0),
        ]);
        Some(fields)
    }

    pub(in crate::app) fn status_selection(&self) -> Option<(String, String, String)> {
        let doc = self.document.as_ref()?;
        let node = &doc.nodes[self.selected];
        let path = if node.path.is_empty() {
            "/"
        } else {
            &node.path
        };
        Some((
            path.to_string(),
            format!("{}: {}", node.key, shorten(&node.value, 100)),
            format!("{}: {}\n{}", node.key, node.value, path),
        ))
    }

    fn keyboard(&mut self, ui: &Ui) {
        let pressed = |key| ui.input(|i| i.key_pressed(key));
        if pressed(egui::Key::Home) {
            self.fit = true;
        }
        self.zoom_by(
            super::diagram::keyboard_zoom(ui),
            (self.canvas_size * 0.5).to_pos2(),
        );
        if super::diagram::keyboard_activate(ui) {
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

    fn export(&mut self, png: bool, dark: bool) {
        self.ensure_layout();
        let Some(doc) = &self.document else {
            return;
        };
        let extension = if png { "png" } else { "svg" };
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(extension, &[extension])
            .set_file_name(format!("structure.{extension}"))
            .save_file()
        else {
            return;
        };
        path.set_extension(extension);
        let svg = export::styled_svg(doc, &self.layout, &self.collapsed, &self.limits, dark);
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("struct-view-structure-export".into())
            .spawn(move || {
                let result = export::write(&path, &svg, png).map_err(|e| e.to_string());
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.export_pending = Some(receiver);
                self.error = None;
                self.notice = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

fn parse_text(source: String, format: Option<DataFormat>) -> ParseResult {
    let result = struct_view_core::structure::parse(&source, format);
    Ok((source, result))
}

fn shorten(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let value: String = chars
        .by_ref()
        .take(max)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if chars.next().is_some() {
        format!("{value}...")
    } else {
        value
    }
}

fn kind_color(kind: Kind) -> Color32 {
    match kind {
        Kind::Object => Color32::from_rgb(125, 190, 255),
        Kind::Array => Color32::from_rgb(201, 164, 255),
        Kind::String => Color32::from_rgb(130, 224, 155),
        Kind::Number => Color32::from_rgb(255, 202, 105),
        Kind::Bool => Color32::from_rgb(255, 153, 153),
        Kind::Null => Color32::from_rgb(210, 215, 225),
        Kind::Date => Color32::from_rgb(106, 224, 224),
        Kind::Reference => Color32::from_rgb(255, 160, 221),
    }
}

fn kind_name(kind: Kind, locale: Locale) -> &'static str {
    locale.text(match kind {
        Kind::Object => TextKey::TypeObject,
        Kind::Array => TextKey::TypeArray,
        Kind::String => TextKey::TypeString,
        Kind::Number => TextKey::TypeNumber,
        Kind::Bool => TextKey::TypeBoolean,
        Kind::Null => TextKey::TypeNull,
        Kind::Date => TextKey::TypeDateTime,
        Kind::Reference => TextKey::StructureReference,
    })
}

fn node_label(node: &Node, collapsed: bool, limit: usize) -> String {
    if matches!(node.kind, Kind::Object | Kind::Array) {
        let (open, close) = if node.kind == Kind::Object {
            ("{", "}")
        } else {
            ("[", "]")
        };
        let anchor = if node.value.starts_with('&') {
            node.value.split_whitespace().next().unwrap_or("")
        } else {
            ""
        };
        format!(
            "{open}{}{close} {anchor}{}",
            node.children.len(),
            if collapsed {
                " (+)"
            } else if node.children.len() > limit {
                " (...)"
            } else {
                ""
            }
        )
    } else {
        shorten(&node.value, 26)
    }
}
