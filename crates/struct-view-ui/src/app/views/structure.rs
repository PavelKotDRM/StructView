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

mod canvas;
mod export;
mod layout;
mod navigation;
mod search;
mod source;
mod status;
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
