use super::*;

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use struct_view_core::parser::JsonNode;

mod export;
pub(in crate::app) use export::{GraphExportFormat, GraphExportStyle, export_graph_image};

pub(super) const GRAPH_DIM_FACTOR: f32 = 0.18;
const GRAPH_EDGE_LABEL_CHAR_WIDTH: f32 = 8.0;
const GRAPH_EDGE_LABEL_HEIGHT: f32 = 16.0;

#[derive(Default)]
pub(in crate::app) struct GraphCalculationState {
    receiver: Option<Receiver<GraphCalculationResult>>,
    result: Option<GraphCalculationResult>,
    error: Option<String>,
    progress: Arc<Mutex<GraphProgress>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphStage {
    Entities,
    Layout,
    Preliminary,
    Conflicts,
    Sequential,
    Labels,
}

impl GraphStage {
    fn text_key(self) -> TextKey {
        match self {
            Self::Entities => TextKey::GraphStageEntities,
            Self::Layout => TextKey::GraphStageLayout,
            Self::Preliminary => TextKey::GraphStagePreliminary,
            Self::Conflicts => TextKey::GraphStageConflicts,
            Self::Sequential => TextKey::GraphStageSequential,
            Self::Labels => TextKey::GraphStageLabels,
        }
    }
}

struct GraphProgress {
    started: Instant,
    stage_started: Instant,
    stage: Option<GraphStage>,
    completed: usize,
    total: usize,
    workers: usize,
    timings: Vec<(GraphStage, Duration)>,
    finished: Option<Duration>,
}

impl Default for GraphProgress {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            stage_started: now,
            stage: None,
            completed: 0,
            total: 0,
            workers: 1,
            timings: Vec::new(),
            finished: None,
        }
    }
}

impl GraphProgress {
    fn begin(&mut self, stage: GraphStage, total: usize, workers: usize) {
        self.end_stage();
        self.stage = Some(stage);
        self.stage_started = Instant::now();
        self.completed = 0;
        self.total = total;
        self.workers = workers;
    }

    fn end_stage(&mut self) {
        if let Some(stage) = self.stage.take() {
            self.timings.push((stage, self.stage_started.elapsed()));
        }
    }

    fn finish(&mut self) {
        self.end_stage();
        self.finished = Some(self.started.elapsed());
    }
}

type GraphProgressTracker = Arc<Mutex<GraphProgress>>;

fn begin_graph_stage(
    progress: Option<&GraphProgressTracker>,
    stage: GraphStage,
    total: usize,
    workers: usize,
) {
    if let Some(progress) = progress {
        progress
            .lock()
            .expect("graph progress lock poisoned")
            .begin(stage, total, workers);
    }
}

fn advance_graph_progress(progress: Option<&GraphProgressTracker>) {
    if let Some(progress) = progress {
        progress
            .lock()
            .expect("graph progress lock poisoned")
            .completed += 1;
    }
}

impl GraphCalculationState {
    pub(in crate::app) fn ensure_started(&mut self, root: &JsonNode, ctx: &egui::Context) {
        if self.receiver.is_some() || self.result.is_some() || self.error.is_some() {
            return;
        }

        let root = root.clone();
        *self.progress.lock().expect("graph progress lock poisoned") = GraphProgress::default();
        let progress = Arc::clone(&self.progress);
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("struct-view-graph-layout".to_string())
            .spawn(move || {
                let result = build_graph_calculation(root, &progress);
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.receiver = Some(receiver);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(in crate::app) fn poll(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => {
                self.receiver = None;
                self.result = Some(result);
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                self.error = Some("The graph calculation worker terminated unexpectedly".into());
                ctx.request_repaint();
            }
        }
    }

    pub(in crate::app) fn result(&self) -> Option<&GraphCalculationResult> {
        self.result.as_ref()
    }

    pub(in crate::app) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(in crate::app) fn has_started(&self) -> bool {
        self.receiver.is_some() || self.result.is_some() || self.error.is_some()
    }

    pub(in crate::app) fn show_progress(&self, ui: &mut egui::Ui, locale: Locale) {
        let progress = self.progress.lock().expect("graph progress lock poisoned");
        let elapsed = progress
            .finished
            .unwrap_or_else(|| progress.started.elapsed());
        ui.label(format!(
            "{}: {:.2} s",
            locale.text(TextKey::GraphElapsed),
            elapsed.as_secs_f64()
        ));
        if let Some(stage) = progress.stage {
            ui.label(format!(
                "{} — {:.2} s",
                locale.text(stage.text_key()),
                progress.stage_started.elapsed().as_secs_f64()
            ));
            ui.label(format!(
                "{}: {}",
                locale.text(TextKey::GraphWorkers),
                progress.workers
            ));
            if progress.total > 0 {
                ui.add(
                    egui::ProgressBar::new(progress.completed as f32 / progress.total as f32)
                        .text(format!("{} / {}", progress.completed, progress.total)),
                );
            }
        }
        if let Some(&(stage, duration)) = progress
            .timings
            .iter()
            .max_by_key(|(_, duration)| *duration)
        {
            ui.label(format!(
                "{}: {} ({:.2} s)",
                locale.text(TextKey::GraphSlowestStage),
                locale.text(stage.text_key()),
                duration.as_secs_f64()
            ));
        }
        if !progress.timings.is_empty() {
            ui.menu_button(locale.text(TextKey::GraphTimingDetails), |ui| {
                for (stage, duration) in &progress.timings {
                    ui.label(format!(
                        "{}: {:.2} s",
                        locale.text(stage.text_key()),
                        duration.as_secs_f64()
                    ));
                }
            });
        }
    }
}

impl GraphCalculationState {
    pub(in crate::app) fn show_activity(&self, ui: &mut egui::Ui, locale: Locale) {
        let progress = self.progress.lock().expect("graph progress lock poisoned");
        if let Some(stage) = progress.stage {
            ui.label(locale.text(stage.text_key()));
            if progress.total > 0 {
                ui.add(
                    egui::ProgressBar::new(progress.completed as f32 / progress.total as f32)
                        .text(format!("{} / {}", progress.completed, progress.total)),
                );
            }
        }
    }
}

pub(in crate::app) struct GraphCalculationResult {
    pub(in crate::app) graph: RelationshipGraph,
    pub(in crate::app) routing: GraphRoutingLayout,
}

pub(in crate::app) struct GraphRoutingLayout {
    graph_fingerprint: u64,
    node_positions: Vec<Pos2>,
    edge_paths: Vec<Vec<Pos2>>,
    edge_labels: Vec<Option<GraphEdgeLabelLayout>>,
    partition_labels: Option<Vec<String>>,
    content_size: Vec2,
}

fn build_graph_calculation(
    root: JsonNode,
    progress: &GraphProgressTracker,
) -> GraphCalculationResult {
    begin_graph_stage(Some(progress), GraphStage::Entities, 0, 1);
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout_with_progress(&graph, Some(progress));
    progress
        .lock()
        .expect("graph progress lock poisoned")
        .finish();
    GraphCalculationResult { graph, routing }
}

#[derive(Clone)]
struct GraphInteractionState {
    zoom: f32,
    selected: HashSet<usize>,
    marquee_start: Option<Pos2>,
    marquee_base: HashSet<usize>,
}

impl Default for GraphInteractionState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            selected: HashSet::new(),
            marquee_start: None,
            marquee_base: HashSet::new(),
        }
    }
}

impl GraphInteractionState {
    fn select_node(&mut self, index: usize, additive: bool) {
        if additive {
            if !self.selected.insert(index) {
                self.selected.remove(&index);
            }
        } else if self.selected.len() == 1 && self.selected.contains(&index) {
            self.selected.clear();
        } else {
            self.selected.clear();
            self.selected.insert(index);
        }
    }
}

/// Отрисовать граф идентификаторов и зависимостей.
pub(in crate::app) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    search: &SearchState,
    locale: Locale,
) -> Option<(GraphExportFormat, GraphExportStyle)> {
    let colors = SyntaxColors::new(ui.visuals());
    let mut export_requested = None;
    let interaction_id = ui.make_persistent_id(("graph-interaction", routing.graph_fingerprint));
    let mut interaction = ui
        .ctx()
        .data(|data| data.get_temp::<GraphInteractionState>(interaction_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphNodes),
            graph.nodes.len()
        ));
        ui.separator();
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphEdges),
            graph.edges.len()
        ));
    });

    if graph.nodes.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::GraphNoEntities));
        });
        return None;
    }
    if graph.edges.is_empty() {
        ui.label(RichText::new(locale.text(TextKey::GraphNoRelationships)).weak());
    }
    let mut fit_requested = false;
    ui.horizontal_wrapped(|ui| {
        ui.menu_button(locale.text(TextKey::GraphExport), |ui| {
            for format in [GraphExportFormat::Svg, GraphExportFormat::Png] {
                ui.menu_button(format.label(), |ui| {
                    for style in [
                        GraphExportStyle::LightTransparent,
                        GraphExportStyle::DarkOpaque,
                    ] {
                        if ui.button(locale.text(style.text_key())).clicked() {
                            export_requested = Some((format, style));
                            ui.close();
                        }
                    }
                });
            }
        });
        if ui
            .button("-")
            .on_hover_text(locale.text(TextKey::GraphZoomOut))
            .clicked()
        {
            interaction.zoom = (interaction.zoom / 1.2).clamp(0.1, 3.0);
        }
        ui.label(format!("{:.0}%", interaction.zoom * 100.0));
        if ui
            .button("+")
            .on_hover_text(locale.text(TextKey::GraphZoomIn))
            .clicked()
        {
            interaction.zoom = (interaction.zoom * 1.2).clamp(0.1, 3.0);
        }
        if ui.button("100%").clicked() {
            interaction.zoom = 1.0;
        }
        if ui.button(locale.text(TextKey::GraphFit)).clicked() {
            fit_requested = true;
        }
        ui.separator();
        if ui.button(locale.text(TextKey::GraphSelectAll)).clicked() {
            interaction.selected = (0..graph.nodes.len()).collect();
        }
        if ui
            .button(locale.text(TextKey::GraphClearSelection))
            .clicked()
        {
            interaction.selected.clear();
        }
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphSelected),
            interaction.selected.len()
        ));
    });
    if fit_requested {
        let viewport = ui.available_size();
        interaction.zoom = ((viewport.x - 16.0).max(1.0) / routing.content_size.x)
            .min((viewport.y - 16.0).max(1.0) / routing.content_size.y)
            .min(3.0);
    }
    let zoom = interaction.zoom;
    let matching_paths = search
        .matches
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let active_path = search.current_match_path();

    let mut scroll_area = egui::ScrollArea::both().scroll_source(egui::scroll_area::ScrollSource {
        drag: egui::scroll_area::DragScroll::Never,
        ..Default::default()
    });
    if fit_requested {
        scroll_area = scroll_area.scroll_offset(Vec2::ZERO);
    }
    scroll_area.auto_shrink([false; 2]).show(ui, |ui| {
        let viewport = ui.available_size_before_wrap();
        let canvas_size = Vec2::new(
            viewport.x.max(routing.content_size.x * zoom),
            viewport.y.max(routing.content_size.y * zoom),
        );
        let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
        let canvas = response.rect;
        let canvas_offset = canvas.min.to_vec2();
        let transform = |point: Pos2| Pos2::ZERO + point.to_vec2() * zoom + canvas_offset;
        let positions = routing
            .node_positions
            .iter()
            .map(|position| transform(*position))
            .collect::<Vec<_>>();
        if let Some(partition_labels) = &routing.partition_labels {
            let header_font = FontId::proportional(13.0 * zoom);
            for (partition, label) in partition_labels.iter().enumerate() {
                let center_x = canvas.left()
                    + (24.0 + partition as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0) * zoom;
                painter.text(
                    Pos2::new(center_x, canvas.top() + 10.0 * zoom),
                    Align2::CENTER_CENTER,
                    label,
                    header_font.clone(),
                    colors.key,
                );
            }
        }
        let mut hovered_node = None;
        for (index, node) in graph.nodes.iter().enumerate() {
            let rect = egui::Rect::from_center_size(positions[index], GRAPH_NODE_SIZE * zoom);
            let response = ui
                .interact(
                    rect,
                    ui.make_persistent_id(("graph-node", &node.path)),
                    Sense::click(),
                )
                .on_hover_text(format!("{}\n{}\n{}", node.label, node.id, node.path));
            if response.hovered() {
                hovered_node = Some(index);
            }
            if response.clicked() {
                interaction.select_node(
                    index,
                    ui.input(|input| input.modifiers.command || input.modifiers.ctrl),
                );
            }
        }
        if response.drag_started() {
            let start = ui.input(|input| input.pointer.press_origin());
            if let Some(start) = start
                && !positions.iter().any(|center| {
                    egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE * zoom).contains(start)
                })
            {
                interaction.marquee_start = Some(Pos2::ZERO + (start - canvas.min) / zoom);
                interaction.marquee_base =
                    if ui.input(|input| input.modifiers.command || input.modifiers.ctrl) {
                        interaction.selected.clone()
                    } else {
                        HashSet::new()
                    };
            }
        }
        if let Some(start) = interaction.marquee_start
            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
        {
            let end = Pos2::ZERO + (pointer - canvas.min) / zoom;
            let selection_rect = egui::Rect::from_two_pos(start, end);
            interaction.selected = interaction.marquee_base.clone();
            for (index, position) in routing.node_positions.iter().enumerate() {
                if selection_rect
                    .intersects(egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE))
                {
                    interaction.selected.insert(index);
                }
            }
            let rect = egui::Rect::from_two_pos(transform(start), transform(end));
            painter.rect_filled(rect, 0, ui.visuals().selection.bg_fill.gamma_multiply(0.25));
            painter.rect_stroke(
                rect,
                0,
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }
        if response.drag_stopped() {
            interaction.marquee_start = None;
        }
        if response.clicked() {
            interaction.selected.clear();
        }
        let focused = |index: usize| {
            hovered_node.map_or_else(
                || interaction.selected.contains(&index),
                |hovered| hovered == index,
            )
        };
        let has_focus = hovered_node.is_some() || !interaction.selected.is_empty();

        let edge_colors = graph
            .edges
            .iter()
            .map(|edge| graph_edge_color(&edge.label, colors))
            .collect::<Vec<_>>();
        let mut neighbor_colors = vec![None; graph.nodes.len()];
        if has_focus {
            for (edge, color) in graph.edges.iter().zip(&edge_colors) {
                if focused(edge.source) {
                    neighbor_colors[edge.target].get_or_insert(*color);
                }
                if focused(edge.target) {
                    neighbor_colors[edge.source].get_or_insert(*color);
                }
            }
        }

        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let edge_path = &routing.edge_paths[edge_index];
            let is_connected = !has_focus || focused(edge.source) || focused(edge.target);
            let edge_color = if has_focus && !is_connected {
                edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                edge_colors[edge_index]
            };
            let edge_width = if !has_focus {
                1.5
            } else if is_connected {
                2.5
            } else {
                1.0
            };
            let stroke = Stroke::new(edge_width * zoom, edge_color);
            for segment in edge_path.windows(2) {
                painter.line_segment([transform(segment[0]), transform(segment[1])], stroke);
            }
            let final_segment = edge_path
                .windows(2)
                .last()
                .expect("graph edges must connect distinct nodes");
            if graph.directed {
                let direction = (final_segment[1] - final_segment[0]).normalized();
                draw_arrow_head(
                    &painter,
                    transform(final_segment[1]),
                    direction,
                    stroke,
                    zoom,
                );
            }
        }
        for (edge_index, label) in routing.edge_labels.iter().enumerate() {
            if let Some(label) = label
                && let Some([start, end]) = label.leader
            {
                let edge = &graph.edges[edge_index];
                let edge_color = if has_focus && !focused(edge.source) && !focused(edge.target) {
                    edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    edge_colors[edge_index]
                };
                painter.extend(egui::Shape::dashed_line(
                    &[transform(start), transform(end)],
                    Stroke::new(zoom, edge_color.gamma_multiply(0.6)),
                    3.0 * zoom,
                    3.0 * zoom,
                ));
            }
        }
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let edge_color = if has_focus && !focused(edge.source) && !focused(edge.target) {
                edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                edge_colors[edge_index]
            };
            if let Some(label) = &routing.edge_labels[edge_index] {
                let label_rect = egui::Rect::from_min_max(
                    transform(label.background.min),
                    transform(label.background.max),
                );
                painter.rect_filled(
                    label_rect,
                    egui::CornerRadius::same(3),
                    ui.visuals().panel_fill,
                );
                painter.text(
                    transform(label.position),
                    label.alignment,
                    label.text.as_str(),
                    FontId::proportional(12.0 * zoom),
                    edge_color,
                );
                if let Some((number, anchor)) = label.reference {
                    for center in [
                        transform(anchor),
                        transform(Pos2::new(
                            label.background.right() - 11.0,
                            label.background.center().y,
                        )),
                    ] {
                        painter.circle_filled(center, 8.0 * zoom, ui.visuals().panel_fill);
                        painter.circle_stroke(center, 8.0 * zoom, Stroke::new(zoom, edge_color));
                        painter.text(
                            center,
                            Align2::CENTER_CENTER,
                            number.to_string(),
                            FontId::proportional(10.0 * zoom),
                            edge_color,
                        );
                    }
                }
                ui.interact(
                    label_rect,
                    ui.make_persistent_id(("graph-edge-label", edge_index)),
                    Sense::hover(),
                )
                .on_hover_ui(|ui| show_graph_edge_information(ui, graph, edge_index, locale));
            }
        }

        for (index, node) in graph.nodes.iter().enumerate() {
            let center = positions[index];
            let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE * zoom);
            let active_match = node
                .search_paths
                .iter()
                .any(|path| active_path == Some(path.as_str()));
            let is_match = active_match
                || node
                    .search_paths
                    .iter()
                    .any(|path| matching_paths.contains(path.as_str()));
            let is_selected = interaction.selected.contains(&index);
            let is_focused = focused(index);
            let neighbor_color = neighbor_colors[index];
            let is_dimmed = has_focus && !is_focused && neighbor_color.is_none() && !is_selected;
            let base_stroke = if active_match {
                Stroke::new(2.5, colors.active_match)
            } else if is_match {
                Stroke::new(2.0, colors.matched)
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke
            };
            let node_stroke = if is_selected || is_focused {
                Stroke::new(3.0, colors.active_match)
            } else if let Some(color) = neighbor_color {
                Stroke::new(
                    2.5,
                    if active_match {
                        colors.active_match
                    } else if is_match {
                        colors.matched
                    } else {
                        color
                    },
                )
            } else if is_dimmed {
                Stroke::new(
                    base_stroke.width,
                    base_stroke.color.gamma_multiply(GRAPH_DIM_FACTOR),
                )
            } else {
                base_stroke
            };
            let node_fill = if is_focused || is_selected {
                ui.visuals().selection.bg_fill
            } else if is_dimmed {
                ui.visuals().faint_bg_color.gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                ui.visuals().faint_bg_color
            };
            painter.rect_filled(rect, egui::CornerRadius::same(6), node_fill);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(6),
                Stroke::new(node_stroke.width * zoom, node_stroke.color),
                egui::StrokeKind::Inside,
            );
            if is_selected {
                let marker = Pos2::new(rect.right() - 11.0 * zoom, rect.top() + 11.0 * zoom);
                painter.circle_stroke(
                    marker,
                    6.0 * zoom,
                    Stroke::new(1.5 * zoom, ui.visuals().panel_fill),
                );
                painter.circle_filled(marker, 4.0 * zoom, colors.active_match);
            }
            let label_font = FontId::proportional(15.0 * zoom);
            let label = shorten_to_width(
                &painter,
                &node.label,
                24,
                &label_font,
                (GRAPH_NODE_SIZE.x - 16.0) * zoom,
                if is_dimmed {
                    colors.key.gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    colors.key
                },
            );
            painter.text(
                Pos2::new(center.x, center.y - 9.0 * zoom),
                Align2::CENTER_CENTER,
                label,
                label_font,
                if is_dimmed {
                    colors.key.gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    colors.key
                },
            );
            let id_color = if is_dimmed {
                ui.visuals()
                    .weak_text_color()
                    .gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                ui.visuals().weak_text_color()
            };
            let id_font = FontId::monospace(11.0 * zoom);
            let id = shorten_to_width(
                &painter,
                &node.id,
                26,
                &id_font,
                (GRAPH_NODE_SIZE.x - 16.0) * zoom,
                id_color,
            );
            painter.text(
                Pos2::new(center.x, center.y + 13.0 * zoom),
                Align2::CENTER_CENTER,
                id,
                id_font,
                id_color,
            );
        }
        if interaction.marquee_start.is_none()
            && !ui.input(|input| input.pointer.any_down())
            && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
            && ui.clip_rect().contains(pointer)
            && canvas.contains(pointer)
            && !positions.iter().any(|center| {
                egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE * zoom).contains(pointer)
            })
            && !routing.edge_labels.iter().flatten().any(|label| {
                egui::Rect::from_min_max(
                    transform(label.background.min),
                    transform(label.background.max),
                )
                .contains(pointer)
            })
        {
            let local_pointer = Pos2::ZERO + (pointer - canvas.min) / zoom;
            let edges = graph_edges_at_pointer(&routing.edge_paths, local_pointer, 6.0 / zoom);
            if !edges.is_empty() {
                ui.interact(
                    egui::Rect::from_center_size(pointer, Vec2::splat(2.0)),
                    ui.make_persistent_id(("graph-edge-hover", routing.graph_fingerprint, &edges)),
                    Sense::hover(),
                )
                .on_hover_ui(|ui| {
                    for (index, &edge_index) in edges.iter().enumerate() {
                        if index > 0 {
                            ui.separator();
                        }
                        show_graph_edge_information(ui, graph, edge_index, locale);
                    }
                });
            }
        }
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(interaction_id, interaction));
    export_requested
}

fn graph_edges_at_pointer(routes: &[Vec<Pos2>], pointer: Pos2, tolerance: f32) -> Vec<usize> {
    let distances = routes
        .iter()
        .map(|route| {
            route
                .windows(2)
                .map(|segment| point_to_segment_distance(pointer, segment[0], segment[1]))
                .fold(f32::INFINITY, f32::min)
        })
        .collect::<Vec<_>>();
    let nearest = distances.iter().copied().fold(f32::INFINITY, f32::min);
    distances
        .iter()
        .enumerate()
        .filter_map(|(index, &distance)| {
            (distance <= tolerance && distance <= nearest + tolerance / 6.0).then_some(index)
        })
        .collect()
}

fn show_graph_edge_information(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    edge_index: usize,
    locale: Locale,
) {
    let edge = &graph.edges[edge_index];
    ui.label(locale.text(if graph.directed {
        TextKey::GraphDirectedLink
    } else {
        TextKey::GraphUndirectedLink
    }));
    if !edge.label.is_empty() {
        ui.label(&edge.label);
    }
    for (key, index) in [
        (TextKey::GraphLinkSource, edge.source),
        (TextKey::GraphLinkTarget, edge.target),
    ] {
        let node = &graph.nodes[index];
        ui.label(format!(
            "{}: {} ({})",
            locale.text(key),
            node.label,
            node.id
        ));
        ui.weak(&node.path);
    }
}

#[cfg(test)]
pub(super) fn build_graph_routing_layout(graph: &RelationshipGraph) -> GraphRoutingLayout {
    build_graph_routing_layout_with_progress(graph, None)
}

fn graph_node_positions(graph: &RelationshipGraph) -> Vec<Pos2> {
    let count = graph.nodes.len();
    let mut outgoing = vec![Vec::new(); count];
    let mut incoming = vec![Vec::new(); count];
    let mut neighbors = vec![Vec::new(); count];
    for edge in &graph.edges {
        outgoing[edge.source].push(edge.target);
        incoming[edge.target].push(edge.source);
        neighbors[edge.source].push(edge.target);
        neighbors[edge.target].push(edge.source);
    }
    for list in outgoing
        .iter_mut()
        .chain(incoming.iter_mut())
        .chain(neighbors.iter_mut())
    {
        list.sort_unstable();
        list.dedup();
    }

    // Collapse directed cycles before assigning left-to-right layers (iterative Kosaraju).
    let mut visited = vec![false; count];
    let mut order = Vec::with_capacity(count);
    for start in 0..count {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if *next < outgoing[*node].len() {
                let target = outgoing[*node][*next];
                *next += 1;
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    let mut component = vec![usize::MAX; count];
    let mut component_count = 0;
    for &start in order.iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        component[start] = component_count;
        while let Some(node) = stack.pop() {
            for &target in &incoming[node] {
                if component[target] == usize::MAX {
                    component[target] = component_count;
                    stack.push(target);
                }
            }
        }
        component_count += 1;
    }
    let mut component_layers = vec![0; component_count];
    // Kosaraju numbers components in topological order.
    let mut component_edges = vec![Vec::new(); component_count];
    for edge in &graph.edges {
        let source = component[edge.source];
        let target = component[edge.target];
        if source != target {
            component_edges[source].push(target);
        }
    }
    for source in 0..component_count {
        for &target in &component_edges[source] {
            component_layers[target] = component_layers[target].max(component_layers[source] + 1);
        }
    }
    let mut layers = component
        .iter()
        .map(|&index| component_layers[index])
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    visited.fill(false);
    for start in 0..count {
        if visited[start] {
            continue;
        }
        let mut group = Vec::new();
        let mut queue = std::collections::VecDeque::from([start]);
        visited[start] = true;
        if !graph.directed {
            layers[start] = 0;
        }
        while let Some(node) = queue.pop_front() {
            group.push(node);
            for &target in &neighbors[node] {
                if !visited[target] {
                    visited[target] = true;
                    if !graph.directed {
                        layers[target] = layers[node] + 1;
                    }
                    queue.push_back(target);
                }
            }
        }
        group.sort_unstable();
        groups.push(group);
    }
    let partitioned = graph
        .partition_names
        .as_ref()
        .filter(|names| !names.is_empty());
    if let Some(names) = partitioned {
        for (index, node) in graph.nodes.iter().enumerate() {
            layers[index] = node.partition.unwrap_or(0).min(names.len() - 1);
        }
    }
    let mut positions = vec![Pos2::ZERO; count];
    let mut row_offset = 0;
    let mut row_indices = vec![0; count];
    for group in groups {
        let columns = group.iter().map(|&node| layers[node]).max().unwrap_or(0) + 1;
        let mut rows = vec![Vec::new(); columns];
        for node in group {
            rows[layers[node]].push(node);
        }
        for column in &rows {
            for (row, &node) in column.iter().enumerate() {
                row_indices[node] = row;
            }
        }
        // Alternating barycenter sweeps reduce crossings without changing layer membership.
        for sweep in 0..4 {
            let column_order: Vec<_> = if sweep % 2 == 0 {
                (0..columns).collect()
            } else {
                (0..columns).rev().collect()
            };
            for column in column_order {
                let mut scores = HashMap::new();
                for &node in &rows[column] {
                    let mut sum = 0.0;
                    let mut adjacent_count = 0;
                    for &neighbor in &neighbors[node] {
                        let adjacent = if sweep % 2 == 0 {
                            layers[neighbor] < column
                        } else {
                            layers[neighbor] > column
                        };
                        if adjacent {
                            sum += row_indices[neighbor] as f32;
                            adjacent_count += 1;
                        }
                    }
                    scores.insert(
                        node,
                        if adjacent_count == 0 {
                            row_indices[node] as f32
                        } else {
                            sum / adjacent_count as f32
                        },
                    );
                }
                rows[column].sort_by(|&left, &right| {
                    scores[&left]
                        .total_cmp(&scores[&right])
                        .then(left.cmp(&right))
                });
                for (row, &node) in rows[column].iter().enumerate() {
                    row_indices[node] = row;
                }
            }
        }
        let height = rows.iter().map(Vec::len).max().unwrap_or(0);
        for (column, nodes) in rows.iter().enumerate() {
            for (row, &node) in nodes.iter().enumerate() {
                positions[node] = Pos2::new(
                    24.0 + column as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                    24.0 + (row_offset + row) as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                );
            }
        }
        row_offset += height + 1;
    }
    positions
}

fn build_graph_routing_layout_with_progress(
    graph: &RelationshipGraph,
    progress: Option<&GraphProgressTracker>,
) -> GraphRoutingLayout {
    begin_graph_stage(progress, GraphStage::Layout, 0, 1);
    let node_positions = graph_node_positions(graph);
    let mut content_size = node_positions
        .iter()
        .fold(Vec2::splat(48.0), |size, point| {
            Vec2::new(
                size.x
                    .max(point.x + GRAPH_STEP.x - GRAPH_NODE_SIZE.x / 2.0 + 24.0),
                size.y
                    .max(point.y + GRAPH_STEP.y - GRAPH_NODE_SIZE.y / 2.0 + 24.0),
            )
        });
    let routing_grid = GraphRoutingGrid::new(&node_positions);
    let edge_endpoints = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect::<Vec<_>>();
    let edge_ports = graph_edge_ports(&node_positions, &edge_endpoints);
    let routed_edges = route_graph_edges_with_progress(
        &routing_grid,
        &edge_endpoints,
        &edge_ports,
        graph_routing_worker_count(graph.edges.len()),
        progress,
    );
    for point in routed_edges.iter().flatten() {
        content_size.x = content_size.x.max(point.x + 24.0);
        content_size.y = content_size.y.max(point.y + 24.0);
    }
    let node_rects = node_positions
        .iter()
        .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
        .collect::<Vec<_>>();
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, content_size);
    let mut occupied_label_rects = Vec::with_capacity(graph.edges.len());
    begin_graph_stage(progress, GraphStage::Labels, graph.edges.len(), 1);
    let mut edge_labels = graph
        .edges
        .iter()
        .zip(&routed_edges)
        .map(|(edge, points)| {
            let label = layout_graph_edge_label(
                &edge.label,
                points,
                canvas,
                &node_rects,
                &occupied_label_rects,
                &routed_edges,
            )
            .or_else(|| {
                graph_edge_label_callout(&edge.label, points, canvas, &occupied_label_rects)
            })
            .inspect(|label| {
                occupied_label_rects.push(label.background);
                content_size.x = content_size.x.max(label.background.right() + 24.0);
                content_size.y = content_size.y.max(label.background.bottom() + 24.0);
            });
            advance_graph_progress(progress);
            label
        })
        .collect::<Vec<_>>();
    resolve_graph_label_leaders(&mut edge_labels, &node_rects, &routed_edges);

    GraphRoutingLayout {
        graph_fingerprint: relationship_graph_fingerprint(graph),
        node_positions,
        edge_paths: routed_edges,
        edge_labels,
        partition_labels: graph.partition_names.clone(),
        content_size,
    }
}

const GRAPH_PARALLEL_EDGE_THRESHOLD: usize = 64;
const GRAPH_MAX_ROUTING_WORKERS: usize = 8;

fn graph_routing_worker_count(edge_count: usize) -> usize {
    if edge_count < GRAPH_PARALLEL_EDGE_THRESHOLD {
        return 1;
    }
    match thread::available_parallelism() {
        Ok(count) => count
            .get()
            .saturating_sub(1)
            .clamp(1, GRAPH_MAX_ROUTING_WORKERS),
        Err(error) => {
            eprintln!("Cannot determine graph routing parallelism: {error}; using one worker");
            1
        }
    }
}

#[cfg(test)]
fn route_graph_edges(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
) -> Vec<Vec<Pos2>> {
    route_graph_edges_with_progress(grid, endpoints, ports, worker_count, None)
}

fn route_graph_edges_with_progress(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    worker_count: usize,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    if endpoints.is_empty() {
        return Vec::new();
    }
    let workers = worker_count
        .clamp(1, GRAPH_MAX_ROUTING_WORKERS)
        .min(endpoints.len());
    if workers == 1 {
        begin_graph_stage(progress, GraphStage::Sequential, endpoints.len(), 1);
        let mut routes = Vec::with_capacity(endpoints.len());
        for (&(source, target), &ports) in endpoints.iter().zip(ports) {
            routes.push(grid.route_edge_with_ports(source, target, ports, &routes));
            advance_graph_progress(progress);
        }
        return routes;
    }

    let chunk_size = endpoints.len().div_ceil(workers);
    begin_graph_stage(progress, GraphStage::Preliminary, endpoints.len(), workers);
    let mut candidates = vec![Vec::new(); endpoints.len()];
    thread::scope(|scope| {
        for (chunk_index, chunk) in candidates.chunks_mut(chunk_size).enumerate() {
            let start = chunk_index * chunk_size;
            scope.spawn(move || {
                for (offset, candidate) in chunk.iter_mut().enumerate() {
                    let index = start + offset;
                    let (source, target) = endpoints[index];
                    *candidate = grid.route_edge_with_ports(source, target, ports[index], &[]);
                    advance_graph_progress(progress);
                }
            });
        }
    });

    resolve_graph_route_conflicts(grid, endpoints, ports, candidates, workers, progress)
}

fn resolve_graph_route_conflicts(
    grid: &GraphRoutingGrid,
    endpoints: &[(usize, usize)],
    ports: &[GraphEdgePorts],
    candidates: Vec<Vec<Pos2>>,
    workers: usize,
    progress: Option<&GraphProgressTracker>,
) -> Vec<Vec<Pos2>> {
    let mut routes = Vec::with_capacity(endpoints.len());
    begin_graph_stage(progress, GraphStage::Conflicts, endpoints.len(), workers);
    // Fixed wave boundaries keep the result independent of worker count and completion order.
    for batch_start in (0..endpoints.len()).step_by(GRAPH_MAX_ROUTING_WORKERS) {
        let batch_end = (batch_start + GRAPH_MAX_ROUTING_WORKERS).min(endpoints.len());
        while routes.len() < batch_end {
            let start = routes.len();
            let mut wave_end = start + 1;
            while wave_end < batch_end
                && !graph_route_conflicts(&candidates[wave_end], &candidates[start..wave_end])
            {
                wave_end += 1;
            }
            let mut proposals = vec![Vec::new(); wave_end - start];
            let chunk_size = proposals.len().div_ceil(workers);
            let prepare = |offset: usize, chunk: &mut [Vec<Pos2>]| {
                for (local_index, proposal) in chunk.iter_mut().enumerate() {
                    let index = start + offset + local_index;
                    *proposal = if graph_route_conflicts(&candidates[index], &routes) {
                        let (source, target) = endpoints[index];
                        grid.route_edge_with_ports(source, target, ports[index], &routes)
                    } else {
                        candidates[index].clone()
                    };
                }
            };
            if proposals.len() == 1 {
                prepare(0, &mut proposals);
            } else {
                thread::scope(|scope| {
                    for (chunk_index, chunk) in proposals.chunks_mut(chunk_size).enumerate() {
                        let prepare = &prepare;
                        scope.spawn(move || prepare(chunk_index * chunk_size, chunk));
                    }
                });
            }
            // At least the first proposal is valid for the snapshot. Recompute the remaining
            // suffix if a newly accepted route invalidates a speculative proposal.
            for proposal in proposals {
                if graph_route_conflicts(&proposal, &routes[start..]) {
                    break;
                }
                routes.push(proposal);
                advance_graph_progress(progress);
            }
        }
    }
    routes
}

fn graph_route_conflicts(candidate: &[Pos2], routes: &[Vec<Pos2>]) -> bool {
    candidate.windows(2).any(|segment| {
        routes
            .iter()
            .flat_map(|route| route.windows(2))
            .any(|other| {
                segments_within_clearance(
                    segment[0],
                    segment[1],
                    other[0],
                    other[1],
                    GRAPH_EDGE_CLEARANCE,
                )
            })
    })
}

struct GraphEdgeLabelLayout {
    text: String,
    position: Pos2,
    alignment: Align2,
    background: egui::Rect,
    leader: Option<[Pos2; 2]>,
    reference: Option<(usize, Pos2)>,
}

fn relationship_graph_fingerprint(graph: &RelationshipGraph) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for node in &graph.nodes {
        node.id.hash(&mut hasher);
        node.label.hash(&mut hasher);
        node.path.hash(&mut hasher);
        node.partition.hash(&mut hasher);
    }
    for edge in &graph.edges {
        edge.source.hash(&mut hasher);
        edge.target.hash(&mut hasher);
        edge.label.hash(&mut hasher);
    }
    graph.directed.hash(&mut hasher);
    graph.partition_names.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn graph_edge_color(label: &str, colors: SyntaxColors) -> egui::Color32 {
    let hash = label.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let palette = [
        colors.number,
        colors.string,
        colors.boolean,
        colors.metadata,
        colors.comment,
        colors.null,
        colors.matched,
        colors.error,
    ];
    palette[(hash % palette.len() as u64) as usize]
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum GraphNodeSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl GraphNodeSide {
    fn for_direction(direction: Vec2) -> Self {
        let half_size = GRAPH_NODE_SIZE / 2.0;
        let horizontal_distance = if direction.x.abs() > 0.0 {
            half_size.x / direction.x.abs()
        } else {
            f32::INFINITY
        };
        let vertical_distance = if direction.y.abs() > 0.0 {
            half_size.y / direction.y.abs()
        } else {
            f32::INFINITY
        };
        if horizontal_distance <= vertical_distance {
            if direction.x > 0.0 {
                Self::Right
            } else {
                Self::Left
            }
        } else if direction.y > 0.0 {
            Self::Bottom
        } else {
            Self::Top
        }
    }

    fn max_lane_offset(self) -> f32 {
        match self {
            Self::Left | Self::Right => GRAPH_NODE_SIZE.y / 2.0 - 10.0,
            Self::Top | Self::Bottom => GRAPH_NODE_SIZE.x / 2.0 - 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GraphEdgePorts {
    pub(super) source_side: GraphNodeSide,
    pub(super) target_side: GraphNodeSide,
    pub(super) source_offset: f32,
    pub(super) target_offset: f32,
}

struct GraphPortEndpoint {
    edge_index: usize,
    is_source: bool,
    opposite_coordinate: f32,
}

pub(super) fn graph_edge_ports(
    node_positions: &[Pos2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let mut groups: HashMap<(usize, GraphNodeSide), Vec<GraphPortEndpoint>> = HashMap::new();
    let mut ports = Vec::with_capacity(edge_endpoints.len());

    for (edge_index, &(source, target)) in edge_endpoints.iter().enumerate() {
        let direction = (node_positions[target] - node_positions[source]).normalized();
        let source_side = GraphNodeSide::for_direction(direction);
        let target_side = GraphNodeSide::for_direction(-direction);
        ports.push(GraphEdgePorts {
            source_side,
            target_side,
            source_offset: 0.0,
            target_offset: 0.0,
        });
        let opposite_coordinate = |node: usize, side: GraphNodeSide| match side {
            GraphNodeSide::Left | GraphNodeSide::Right => node_positions[node].y,
            GraphNodeSide::Top | GraphNodeSide::Bottom => node_positions[node].x,
        };
        groups
            .entry((source, source_side))
            .or_default()
            .push(GraphPortEndpoint {
                edge_index,
                is_source: true,
                opposite_coordinate: opposite_coordinate(target, source_side),
            });
        groups
            .entry((target, target_side))
            .or_default()
            .push(GraphPortEndpoint {
                edge_index,
                is_source: false,
                opposite_coordinate: opposite_coordinate(source, target_side),
            });
    }

    for ((_, side), mut endpoints) in groups {
        endpoints.sort_by(|left, right| {
            left.opposite_coordinate
                .total_cmp(&right.opposite_coordinate)
                .then_with(|| left.edge_index.cmp(&right.edge_index))
        });
        let lane_spacing = (2.0 * side.max_lane_offset()
            / endpoints.len().saturating_sub(1).max(1) as f32)
            .min(16.0);
        for (lane, endpoint) in endpoints.iter().enumerate() {
            let offset = (lane as f32 - (endpoints.len() - 1) as f32 / 2.0) * lane_spacing;
            if endpoint.is_source {
                ports[endpoint.edge_index].source_offset = offset;
            } else {
                ports[endpoint.edge_index].target_offset = offset;
            }
        }
    }

    ports
}

pub(super) const GRAPH_ROUTE_CLEARANCE: f32 = 18.0;
pub(super) const GRAPH_EDGE_CLEARANCE: f32 = 8.0;
const GRAPH_ROUTE_TURN_PENALTY: f32 = 48.0;
const GRAPH_EDGE_ROUTE_PENALTY: f32 = 500.0;
const GRAPH_EDGE_CROSSING_PENALTY: f32 = 10_000.0;
const GRAPH_ROUTE_DIRECTIONS: usize = 3;
const GRAPH_ROUTE_HORIZONTAL: usize = 0;
const GRAPH_ROUTE_VERTICAL: usize = 1;
const GRAPH_ROUTE_NO_DIRECTION: usize = 2;

#[derive(Clone, Copy)]
struct GraphRoutePort {
    card: Pos2,
    route: Pos2,
}

#[derive(Debug)]
struct GraphRouteQueueEntry {
    estimated_total: f32,
    distance: f32,
    state: usize,
}

impl PartialEq for GraphRouteQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.estimated_total.total_cmp(&other.estimated_total) == Ordering::Equal
            && self.distance.total_cmp(&other.distance) == Ordering::Equal
            && self.state == other.state
    }
}

impl Eq for GraphRouteQueueEntry {}

impl PartialOrd for GraphRouteQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GraphRouteQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .total_cmp(&self.estimated_total)
            .then_with(|| other.distance.total_cmp(&self.distance))
            .then_with(|| other.state.cmp(&self.state))
    }
}

pub(super) struct GraphRoutingGrid {
    node_positions: Vec<Pos2>,
    node_rects: Vec<egui::Rect>,
    obstacles: Vec<egui::Rect>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
}

impl GraphRoutingGrid {
    pub(super) fn new(node_positions: &[Pos2]) -> Self {
        let node_rects = node_positions
            .iter()
            .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
            .collect::<Vec<_>>();
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(GRAPH_ROUTE_CLEARANCE))
            .collect::<Vec<_>>();
        let mut x_coordinates = Vec::with_capacity(node_positions.len() * 3);
        let mut y_coordinates = Vec::with_capacity(node_positions.len() * 3);

        for (center, obstacle) in node_positions.iter().zip(&obstacles) {
            x_coordinates.extend([obstacle.left(), center.x, obstacle.right()]);
            y_coordinates.extend([obstacle.top(), center.y, obstacle.bottom()]);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Self {
            node_positions: node_positions.to_vec(),
            node_rects,
            obstacles,
            x_coordinates,
            y_coordinates,
        }
    }

    #[cfg(test)]
    pub(super) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        let direction = (self.node_positions[target] - self.node_positions[source]).normalized();
        self.route_edge_with_ports(
            source,
            target,
            GraphEdgePorts {
                source_side: GraphNodeSide::for_direction(direction),
                target_side: GraphNodeSide::for_direction(-direction),
                source_offset: 0.0,
                target_offset: 0.0,
            },
            &[],
        )
    }

    pub(super) fn route_edge_with_ports(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let source_port = self.route_port(source, edge_ports.source_side, edge_ports.source_offset);
        let target_port = self.route_port(target, edge_ports.target_side, edge_ports.target_offset);
        let line_start = source_port.card;
        let line_end = target_port.card;
        let crosses_another_node = self.obstacles.iter().enumerate().any(|(index, obstacle)| {
            index != source
                && index != target
                && segment_intersects_rect(line_start, line_end, *obstacle)
        });
        let overlaps_another_edge =
            routed_edges
                .iter()
                .flat_map(|route| route.windows(2))
                .any(|segment| {
                    segments_within_clearance(
                        line_start,
                        line_end,
                        segment[0],
                        segment[1],
                        GRAPH_EDGE_CLEARANCE,
                    )
                });
        if !crosses_another_node && !overlaps_another_edge {
            return vec![line_start, line_end];
        }

        let mut x_coordinates = self.x_coordinates.clone();
        x_coordinates.extend([source_port.route.x, target_port.route.x]);
        let mut y_coordinates = self.y_coordinates.clone();
        y_coordinates.extend([source_port.route.y, target_port.route.y]);
        // Existing obstacle boundaries alone cannot separate detoured parallel edges.
        let track_spacing = GRAPH_EDGE_CLEARANCE + 2.0;
        for point in routed_edges.iter().flatten() {
            x_coordinates.extend([point.x - track_spacing, point.x + track_spacing]);
            y_coordinates.extend([point.y - track_spacing, point.y + track_spacing]);
        }
        x_coordinates.retain(|coordinate| *coordinate >= 0.0);
        y_coordinates.retain(|coordinate| *coordinate >= 0.0);
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);
        let route = self.find_orthogonal_path(
            source_port.route,
            target_port.route,
            &x_coordinates,
            &y_coordinates,
            routed_edges,
        );

        let mut points = Vec::with_capacity(route.len() + 2);
        points.push(source_port.card);
        points.extend(route);
        points.push(target_port.card);
        Self::simplify_graph_route(points)
    }

    fn simplify_graph_route(points: Vec<Pos2>) -> Vec<Pos2> {
        let mut simplified: Vec<Pos2> = Vec::with_capacity(points.len());
        for point in points {
            if simplified.last() == Some(&point) {
                continue;
            }
            while simplified.len() >= 2 {
                let start = simplified[simplified.len() - 2];
                let middle = simplified[simplified.len() - 1];
                let first = middle - start;
                let second = point - middle;
                if cross_product(first, second) != 0.0 || first.dot(second) <= 0.0 {
                    break;
                }
                simplified.pop();
            }
            simplified.push(point);
        }
        simplified
    }

    fn route_port(&self, node: usize, side: GraphNodeSide, offset: f32) -> GraphRoutePort {
        let center = self.node_positions[node];
        let card = self.node_rects[node];
        let route = self.obstacles[node];
        match side {
            GraphNodeSide::Left => GraphRoutePort {
                card: Pos2::new(card.left(), center.y + offset),
                route: Pos2::new(route.left(), center.y + offset),
            },
            GraphNodeSide::Right => GraphRoutePort {
                card: Pos2::new(card.right(), center.y + offset),
                route: Pos2::new(route.right(), center.y + offset),
            },
            GraphNodeSide::Top => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.top()),
                route: Pos2::new(center.x + offset, route.top()),
            },
            GraphNodeSide::Bottom => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.bottom()),
                route: Pos2::new(center.x + offset, route.bottom()),
            },
        }
    }

    fn find_orthogonal_path(
        &self,
        source: Pos2,
        target: Pos2,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        let state_count = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let mut distances = vec![f32::INFINITY; state_count];
        let mut previous_states = vec![usize::MAX; state_count];
        let segment_index = GraphRouteSegmentIndex::new(routed_edges);
        let mut segment_costs = HashMap::new();
        let goal_vertex = Self::vertex_index(target, x_coordinates, y_coordinates);
        let start_vertex = Self::vertex_index(source, x_coordinates, y_coordinates);
        let mut queue = BinaryHeap::new();
        let start_state = start_vertex * GRAPH_ROUTE_DIRECTIONS + GRAPH_ROUTE_NO_DIRECTION;
        distances[start_state] = 0.0;
        queue.push(GraphRouteQueueEntry {
            estimated_total: route_heuristic(source, &[target]),
            distance: 0.0,
            state: start_state,
        });

        let mut goal_state = None;
        while let Some(entry) = queue.pop() {
            if entry.distance > distances[entry.state] {
                continue;
            }

            let current_vertex = entry.state / GRAPH_ROUTE_DIRECTIONS;
            if current_vertex == goal_vertex {
                goal_state = Some(entry.state);
                break;
            }

            let column = current_vertex % width;
            let row = current_vertex / width;
            let current_point = Self::point_at(current_vertex, x_coordinates, y_coordinates);
            let mut neighbors = [None; 4];
            if column > 0 {
                neighbors[0] = Some((
                    current_vertex - 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column] - x_coordinates[column - 1],
                ));
            }
            if column + 1 < width {
                neighbors[1] = Some((
                    current_vertex + 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column + 1] - x_coordinates[column],
                ));
            }
            if row > 0 {
                neighbors[2] = Some((
                    current_vertex - width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row] - y_coordinates[row - 1],
                ));
            }
            if row + 1 < y_coordinates.len() {
                neighbors[3] = Some((
                    current_vertex + width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row + 1] - y_coordinates[row],
                ));
            }

            for (next_vertex, direction, segment_length) in neighbors.into_iter().flatten() {
                let next_point = Self::point_at(next_vertex, x_coordinates, y_coordinates);
                if self.obstacles.iter().any(|obstacle| {
                    segment_crosses_rect_interior(current_point, next_point, *obstacle)
                }) {
                    continue;
                }
                let edge_key = (
                    current_vertex.min(next_vertex),
                    current_vertex.max(next_vertex),
                );
                let edge_overlap_penalty = *segment_costs
                    .entry(edge_key)
                    .or_insert_with(|| segment_index.penalty(current_point, next_point));

                let turn_penalty = if entry.state % GRAPH_ROUTE_DIRECTIONS
                    != GRAPH_ROUTE_NO_DIRECTION
                    && entry.state % GRAPH_ROUTE_DIRECTIONS != direction
                {
                    GRAPH_ROUTE_TURN_PENALTY
                } else {
                    0.0
                };
                let next_distance =
                    entry.distance + segment_length + turn_penalty + edge_overlap_penalty;
                let next_state = next_vertex * GRAPH_ROUTE_DIRECTIONS + direction;
                if next_distance >= distances[next_state] {
                    continue;
                }

                distances[next_state] = next_distance;
                previous_states[next_state] = entry.state;
                queue.push(GraphRouteQueueEntry {
                    estimated_total: next_distance + route_heuristic(next_point, &[target]),
                    distance: next_distance,
                    state: next_state,
                });
            }
        }

        let mut state = goal_state.expect("graph nodes must have an obstacle-free route");
        let mut route = Vec::new();
        loop {
            route.push(Self::point_at(
                state / GRAPH_ROUTE_DIRECTIONS,
                x_coordinates,
                y_coordinates,
            ));
            let previous = previous_states[state];
            if previous == usize::MAX {
                break;
            }
            state = previous;
        }
        route.reverse();
        route
    }

    fn vertex_index(point: Pos2, x_coordinates: &[f32], y_coordinates: &[f32]) -> usize {
        let column = x_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.x))
            .expect("graph route x coordinates must include every port");
        let row = y_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.y))
            .expect("graph route y coordinates must include every port");
        row * x_coordinates.len() + column
    }

    fn point_at(vertex: usize, x_coordinates: &[f32], y_coordinates: &[f32]) -> Pos2 {
        let width = x_coordinates.len();
        Pos2::new(x_coordinates[vertex % width], y_coordinates[vertex / width])
    }
}

fn sort_unique_coordinates(coordinates: &mut Vec<f32>) {
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn route_heuristic(point: Pos2, goals: &[Pos2]) -> f32 {
    goals
        .iter()
        .map(|goal| (point.x - goal.x).abs() + (point.y - goal.y).abs())
        .fold(f32::INFINITY, f32::min)
}

fn segment_intersects_rect(start: Pos2, end: Pos2, rect: egui::Rect) -> bool {
    let direction = end - start;
    let mut first_intersection: f32 = 0.0;
    let mut last_intersection: f32 = 1.0;
    for (origin, delta, minimum, maximum) in [
        (start.x, direction.x, rect.left(), rect.right()),
        (start.y, direction.y, rect.top(), rect.bottom()),
    ] {
        if delta == 0.0 {
            if origin <= minimum || origin >= maximum {
                return false;
            }
            continue;
        }

        let first = (minimum - origin) / delta;
        let last = (maximum - origin) / delta;
        first_intersection = first_intersection.max(first.min(last));
        last_intersection = last_intersection.min(first.max(last));
        if first_intersection >= last_intersection {
            return false;
        }
    }
    first_intersection < last_intersection
}

struct GraphRouteSegmentIndex {
    segments: Vec<[Pos2; 2]>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl GraphRouteSegmentIndex {
    fn cells(start: Pos2, end: Pos2) -> impl Iterator<Item = (i32, i32)> {
        let bounds = egui::Rect::from_two_pos(start, end).expand(GRAPH_EDGE_CLEARANCE);
        let cell_size = 128.0;
        let left = (bounds.left() / cell_size).floor() as i32;
        let right = (bounds.right() / cell_size).floor() as i32;
        let top = (bounds.top() / cell_size).floor() as i32;
        let bottom = (bounds.bottom() / cell_size).floor() as i32;
        (top..=bottom).flat_map(move |row| (left..=right).map(move |column| (column, row)))
    }

    fn new(routes: &[Vec<Pos2>]) -> Self {
        let segments = routes
            .iter()
            .flat_map(|route| route.windows(2))
            .map(|segment| [segment[0], segment[1]])
            .collect::<Vec<_>>();
        let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (index, segment) in segments.iter().enumerate() {
            for cell in Self::cells(segment[0], segment[1]) {
                buckets.entry(cell).or_default().push(index);
            }
        }
        Self { segments, buckets }
    }

    fn penalty(&self, start: Pos2, end: Pos2) -> f32 {
        let mut indices = Self::cells(start, end)
            .filter_map(|cell| self.buckets.get(&cell))
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .map(|index| segment_pair_penalty(start, end, self.segments[index]))
            .sum()
    }
}

fn segment_pair_penalty(start: Pos2, end: Pos2, segment: [Pos2; 2]) -> f32 {
    if segments_intersect(start, end, segment[0], segment[1])
        || segments_within_clearance(start, end, segment[0], segment[1], 1.0)
    {
        GRAPH_EDGE_CROSSING_PENALTY
    } else if segments_within_clearance(start, end, segment[0], segment[1], GRAPH_EDGE_CLEARANCE) {
        GRAPH_EDGE_ROUTE_PENALTY
    } else {
        0.0
    }
}

pub(super) fn segments_within_clearance(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
    clearance: f32,
) -> bool {
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return true;
    }
    [
        point_to_segment_distance(first_start, second_start, second_end),
        point_to_segment_distance(first_end, second_start, second_end),
        point_to_segment_distance(second_start, first_start, first_end),
        point_to_segment_distance(second_end, first_start, first_end),
    ]
    .into_iter()
    .any(|distance| distance <= clearance)
}

fn segments_intersect(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
) -> bool {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let denominator = cross_product(first_direction, second_direction);
    if denominator == 0.0 {
        return false;
    }

    let between_starts = second_start - first_start;
    let first_position = cross_product(between_starts, second_direction) / denominator;
    let second_position = cross_product(between_starts, first_direction) / denominator;
    (0.0..=1.0).contains(&first_position) && (0.0..=1.0).contains(&second_position)
}

fn cross_product(first: Vec2, second: Vec2) -> f32 {
    first.x * second.y - first.y * second.x
}

fn point_to_segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    point.distance(closest_point_on_segment(point, start, end))
}

fn closest_point_on_segment(point: Pos2, start: Pos2, end: Pos2) -> Pos2 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return start;
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    start + segment * projection
}

pub(super) fn segment_crosses_rect_interior(start: Pos2, end: Pos2, rect: egui::Rect) -> bool {
    if start.y == end.y {
        start.y > rect.top()
            && start.y < rect.bottom()
            && start.x.min(end.x) < rect.right()
            && start.x.max(end.x) > rect.left()
    } else {
        start.x > rect.left()
            && start.x < rect.right()
            && start.y.min(end.y) < rect.bottom()
            && start.y.max(end.y) > rect.top()
    }
}

fn edge_label_placement(points: &[Pos2], node_rects: &[egui::Rect]) -> (Pos2, Align2, f32) {
    if points.len() == 2 {
        let line_start = points[0];
        let line_end = points[1];
        return (
            Pos2::new(
                (line_start.x + line_end.x) / 2.0,
                (line_start.y + line_end.y) / 2.0 - 8.0,
            ),
            Align2::CENTER_CENTER,
            (line_end - line_start).length() - 8.0,
        );
    }

    let longest_horizontal = points
        .windows(2)
        .filter(|segment| segment[0].y == segment[1].y)
        .max_by(|left, right| {
            (left[0].x - left[1].x)
                .abs()
                .total_cmp(&(right[0].x - right[1].x).abs())
        });
    if let Some(segment) = longest_horizontal
        && (segment[0].x - segment[1].x).abs() >= 32.0
    {
        return (
            Pos2::new((segment[0].x + segment[1].x) / 2.0, segment[0].y - 8.0),
            Align2::CENTER_CENTER,
            (segment[0].x - segment[1].x).abs() - 8.0,
        );
    }

    let segment = points
        .windows(2)
        .filter(|segment| segment[0].x == segment[1].x)
        .max_by(|left, right| {
            (left[0].y - left[1].y)
                .abs()
                .total_cmp(&(right[0].y - right[1].y).abs())
        })
        .expect("a detoured graph route must contain a vertical segment");
    let midpoint = (segment[0].y + segment[1].y) / 2.0;
    let left_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.right() <= segment[0].x
        })
        .map(|rect| segment[0].x - rect.right())
        .fold(f32::INFINITY, f32::min);
    let right_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.left() >= segment[0].x
        })
        .map(|rect| rect.left() - segment[0].x)
        .fold(f32::INFINITY, f32::min);
    let (position, alignment, available_space) = if right_space >= left_space {
        (
            Pos2::new(segment[0].x + 6.0, midpoint),
            Align2::LEFT_CENTER,
            right_space,
        )
    } else {
        (
            Pos2::new(segment[0].x - 6.0, midpoint),
            Align2::RIGHT_CENTER,
            left_space,
        )
    };
    let width = (available_space - 12.0).clamp(0.0, GRAPH_NODE_SIZE.x - 16.0);
    (position, alignment, width)
}

fn shorten_graph_edge_label(label: &str, max_width: f32) -> String {
    let label = single_line_text(label).into_owned();
    let max_chars = (max_width / GRAPH_EDGE_LABEL_CHAR_WIDTH).floor().max(0.0) as usize;
    let max_chars = max_chars.min(18);
    let characters = label.chars().collect::<Vec<_>>();
    if characters.len() <= max_chars {
        return label;
    }
    if max_chars == 0 {
        return String::new();
    }

    let mut shortened = characters
        .into_iter()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

fn layout_graph_edge_label(
    label: &str,
    points: &[Pos2],
    canvas: egui::Rect,
    node_rects: &[egui::Rect],
    occupied_label_rects: &[egui::Rect],
    routed_edges: &[Vec<Pos2>],
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let minimum_width =
        single_line_text(label).chars().count().min(6) as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH;
    let mut candidates = vec![edge_label_placement(points, node_rects)];
    let mut segments = points.windows(2).collect::<Vec<_>>();
    segments.sort_by(|left, right| {
        (right[1] - right[0])
            .length_sq()
            .total_cmp(&(left[1] - left[0]).length_sq())
    });
    for segment in segments {
        let direction = segment[1] - segment[0];
        for fraction in [0.5, 0.25, 0.75] {
            let midpoint = segment[0] + direction * fraction;
            for distance in [18.0, 36.0, 54.0, 90.0, 126.0] {
                if direction.x == 0.0 {
                    candidates.extend([
                        (
                            midpoint + Vec2::new(distance, 0.0),
                            Align2::LEFT_CENTER,
                            160.0,
                        ),
                        (
                            midpoint - Vec2::new(distance, 0.0),
                            Align2::RIGHT_CENTER,
                            160.0,
                        ),
                    ]);
                } else {
                    let normal = Vec2::new(direction.y, -direction.x).normalized() * distance;
                    let width = direction.length() - 8.0;
                    candidates.extend([
                        (midpoint + normal, Align2::CENTER_CENTER, width),
                        (midpoint - normal, Align2::CENTER_CENTER, width),
                    ]);
                }
            }
        }
    }
    for (position, alignment, width) in candidates {
        if width < minimum_width {
            continue;
        }
        let text = shorten_graph_edge_label(label, width);
        if let Some(background) = place_edge_label(
            &text,
            position,
            alignment,
            canvas,
            node_rects,
            occupied_label_rects,
            routed_edges,
        ) {
            return Some(graph_edge_label_layout(
                text, position, alignment, background, points,
            ));
        }
    }
    None
}

fn graph_edge_label_layout(
    text: String,
    position: Pos2,
    alignment: Align2,
    background: egui::Rect,
    points: &[Pos2],
) -> GraphEdgeLabelLayout {
    let center = background.center();
    let anchor = points
        .windows(2)
        .map(|segment| closest_point_on_segment(center, segment[0], segment[1]))
        .min_by(|left, right| {
            left.distance_sq(center)
                .total_cmp(&right.distance_sq(center))
        })
        .expect("graph labels must belong to a route");
    let direction = anchor - center;
    let border_distance = (background.width() / 2.0 / direction.x.abs())
        .min(background.height() / 2.0 / direction.y.abs());
    let leader = (direction.length() > 24.0)
        .then(|| [anchor, center + direction * border_distance.min(1.0)]);
    GraphEdgeLabelLayout {
        text,
        position,
        alignment,
        background,
        leader,
        reference: None,
    }
}

fn resolve_graph_label_leaders(
    labels: &mut [Option<GraphEdgeLabelLayout>],
    node_rects: &[egui::Rect],
    routes: &[Vec<Pos2>],
) {
    let backgrounds = labels
        .iter()
        .flatten()
        .map(|label| label.background)
        .collect::<Vec<_>>();
    let mut accepted: Vec<[Pos2; 2]> = Vec::new();
    let mut reference = 0;
    let mut markers: Vec<egui::Rect> = Vec::new();
    for (index, label) in labels.iter_mut().enumerate() {
        let Some(label) = label else { continue };
        let Some([start, end]) = label.leader else {
            continue;
        };
        let blocked = node_rects
            .iter()
            .any(|rect| segment_intersects_rect(start, end, rect.expand(3.0)))
            || backgrounds.iter().any(|rect| {
                *rect != label.background && segment_intersects_rect(start, end, rect.expand(2.0))
            })
            || routes.iter().enumerate().any(|(route_index, route)| {
                route.windows(2).any(|segment| {
                    if route_index == index {
                        // Touching the owning route at the attachment point is intentional.
                        segments_intersect(
                            start + (end - start).normalized() * 3.0,
                            end,
                            segment[0],
                            segment[1],
                        )
                    } else {
                        segments_within_clearance(start, end, segment[0], segment[1], 3.0)
                    }
                })
            })
            || accepted.iter().any(|&[other_start, other_end]| {
                segments_within_clearance(start, end, other_start, other_end, 3.0)
            });
        if blocked {
            reference += 1;
            label.leader = None;
            let anchor = routes[index]
                .windows(2)
                .flat_map(|segment| {
                    (1..16)
                        .map(|step| segment[0] + (segment[1] - segment[0]) * (step as f32 / 16.0))
                })
                .filter(|point| {
                    let marker = egui::Rect::from_center_size(*point, Vec2::splat(16.0));
                    !node_rects
                        .iter()
                        .chain(&backgrounds)
                        .any(|rect| marker.intersects(*rect))
                        && !markers.iter().any(|rect| marker.intersects(*rect))
                        && !routes.iter().enumerate().any(|(route_index, route)| {
                            route_index != index
                                && route.windows(2).any(|segment| {
                                    segment_intersects_rect(segment[0], segment[1], marker)
                                })
                        })
                })
                .min_by(|left, right| left.distance_sq(start).total_cmp(&right.distance_sq(start)))
                .unwrap_or(start);
            label.reference = Some((reference, anchor));
            markers.push(egui::Rect::from_center_size(anchor, Vec2::splat(18.0)));
        } else {
            accepted.push([start, end]);
        }
    }
}

fn graph_edge_label_callout(
    label: &str,
    points: &[Pos2],
    canvas: egui::Rect,
    occupied_label_rects: &[egui::Rect],
) -> Option<GraphEdgeLabelLayout> {
    if label.trim().is_empty() {
        return None;
    }
    let text = shorten_graph_edge_label(label, 144.0);
    Some(graph_edge_label_callout_text(
        text,
        points,
        canvas,
        occupied_label_rects,
    ))
}

fn graph_edge_label_callout_text(
    text: String,
    points: &[Pos2],
    canvas: egui::Rect,
    occupied_label_rects: &[egui::Rect],
) -> GraphEdgeLabelLayout {
    let size = Vec2::new(
        text.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    // A reserved column beyond all routes guarantees a visible label even in dense graphs.
    let mut position = Pos2::new(
        canvas.right() + 24.0,
        points[points.len() / 2].y.max(size.y / 2.0 + 24.0),
    );
    loop {
        let background = aligned_label_rect(position, size, Align2::LEFT_CENTER).expand(3.0);
        let bottom = occupied_label_rects
            .iter()
            .filter(|rect| background.intersects(rect.expand(2.0)))
            .map(egui::Rect::bottom)
            .max_by(f32::total_cmp);
        if let Some(bottom) = bottom {
            position.y = bottom + size.y / 2.0 + 9.0;
        } else {
            return graph_edge_label_layout(
                text,
                position,
                Align2::LEFT_CENTER,
                background,
                points,
            );
        }
    }
}

fn place_edge_label(
    label: &str,
    position: Pos2,
    alignment: Align2,
    canvas: egui::Rect,
    node_rects: &[egui::Rect],
    occupied_label_rects: &[egui::Rect],
    routed_edges: &[Vec<Pos2>],
) -> Option<egui::Rect> {
    if label.is_empty() {
        return None;
    }

    let size = Vec2::new(
        label.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let label_rect = aligned_label_rect(position, size, alignment).expand(3.0);
    if label_rect.left() < canvas.left()
        || label_rect.right() > canvas.right()
        || label_rect.top() < canvas.top()
        || label_rect.bottom() > canvas.bottom()
        || node_rects
            .iter()
            .any(|rect| label_rect.intersects(rect.expand(2.0)))
        || occupied_label_rects
            .iter()
            .any(|rect| label_rect.intersects(*rect))
        || routed_edges.iter().any(|route| {
            route.windows(2).any(|segment| {
                segment_intersects_rect(segment[0], segment[1], label_rect.expand(2.0))
            }) || route.last().is_some_and(|tip| {
                label_rect.intersects(egui::Rect::from_center_size(*tip, Vec2::splat(20.0)))
            })
        })
    {
        return None;
    }
    Some(label_rect)
}

fn aligned_label_rect(position: Pos2, size: Vec2, alignment: Align2) -> egui::Rect {
    let min = if alignment == Align2::LEFT_CENTER {
        Pos2::new(position.x, position.y - size.y / 2.0)
    } else if alignment == Align2::RIGHT_CENTER {
        Pos2::new(position.x - size.x, position.y - size.y / 2.0)
    } else {
        position - size / 2.0
    };
    let mut rect = egui::Rect::from_min_size(min, size);
    rect.max.x += 24.0;
    rect
}

fn shorten_to_width(
    painter: &egui::Painter,
    text: &str,
    max_chars: usize,
    font_id: &FontId,
    max_width: f32,
    color: egui::Color32,
) -> String {
    let text = single_line_text(text);
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    let needs_ellipsis = characters.next().is_some();
    let candidate = if needs_ellipsis {
        format!("{prefix}…")
    } else {
        prefix.clone()
    };
    if painter
        .layout_no_wrap(candidate.clone(), font_id.clone(), color)
        .size()
        .x
        <= max_width
    {
        return candidate;
    }

    let ellipsis = "…";
    if painter
        .layout_no_wrap(ellipsis.to_string(), font_id.clone(), color)
        .size()
        .x
        > max_width
    {
        return String::new();
    }

    let mut result = String::new();
    for character in prefix.chars() {
        let next = format!("{result}{character}{ellipsis}");
        if painter
            .layout_no_wrap(next, font_id.clone(), color)
            .size()
            .x
            > max_width
        {
            break;
        }
        result.push(character);
    }
    result.push_str(ellipsis);
    result
}

#[cfg(test)]
pub(super) fn box_border_offset(direction: Vec2) -> f32 {
    (GRAPH_NODE_SIZE.x / 2.0 / direction.x.abs()).min(GRAPH_NODE_SIZE.y / 2.0 / direction.y.abs())
}

fn draw_arrow_head(painter: &egui::Painter, tip: Pos2, direction: Vec2, stroke: Stroke, zoom: f32) {
    let arrow_length = 9.0 * zoom;
    for angle_offset in [2.55, -2.55] {
        let wing = tip + Vec2::angled(direction.angle() + angle_offset) * arrow_length;
        painter.line_segment([tip, wing], stroke);
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;

    #[test]
    fn graph_label_leaders_use_references_when_nodes_routes_or_labels_block_them() {
        let own_route = vec![Pos2::new(50.0, -40.0), Pos2::new(50.0, 40.0)];
        let make_label = || {
            let mut label = graph_edge_label_layout(
                "connection".into(),
                Pos2::new(200.0, 0.0),
                Align2::LEFT_CENTER,
                aligned_label_rect(
                    Pos2::new(200.0, 0.0),
                    Vec2::new(80.0, 16.0),
                    Align2::LEFT_CENTER,
                ),
                &own_route,
            );
            label.leader = Some([Pos2::new(50.0, 0.0), Pos2::new(200.0, 0.0)]);
            label
        };
        let mut clear = vec![Some(make_label())];
        resolve_graph_label_leaders(&mut clear, &[], std::slice::from_ref(&own_route));
        assert!(clear[0].as_ref().unwrap().leader.is_some());
        assert!(clear[0].as_ref().unwrap().reference.is_none());
        for blocker in 0..3 {
            let mut labels = vec![Some(make_label())];
            let mut routes = vec![own_route.clone()];
            let mut nodes = Vec::new();
            match blocker {
                0 => nodes.push(egui::Rect::from_center_size(
                    Pos2::new(120.0, 0.0),
                    Vec2::splat(30.0),
                )),
                1 => routes.push(vec![Pos2::new(120.0, -30.0), Pos2::new(120.0, 30.0)]),
                _ => {
                    let mut other = make_label();
                    other.background =
                        egui::Rect::from_center_size(Pos2::new(120.0, 0.0), Vec2::splat(30.0));
                    other.leader = None;
                    labels.push(Some(other));
                    routes.push(own_route.clone());
                }
            }
            resolve_graph_label_leaders(&mut labels, &nodes, &routes);
            let label = labels[0].as_ref().unwrap();
            assert!(label.leader.is_none());
            let (number, anchor) = label.reference.unwrap();
            assert_eq!(number, 1);
            assert_eq!(
                point_to_segment_distance(anchor, own_route[0], own_route[1]),
                0.0
            );
            assert_eq!(label.text, "connection");
        }
    }

    #[test]
    fn crossing_label_leaders_do_not_both_remain_visible() {
        let routes = vec![
            vec![Pos2::new(20.0, 0.0), Pos2::new(40.0, 0.0)],
            vec![Pos2::new(20.0, 100.0), Pos2::new(40.0, 100.0)],
        ];
        let mut labels = routes
            .iter()
            .enumerate()
            .map(|(index, route)| {
                let y = if index == 0 { 100.0 } else { 0.0 };
                let position = Pos2::new(200.0, y);
                let mut label = graph_edge_label_layout(
                    format!("link {index}"),
                    position,
                    Align2::LEFT_CENTER,
                    aligned_label_rect(position, Vec2::new(60.0, 16.0), Align2::LEFT_CENTER),
                    route,
                );
                label.leader = Some([route[1], position]);
                Some(label)
            })
            .collect::<Vec<_>>();
        resolve_graph_label_leaders(&mut labels, &[], &routes);
        assert_eq!(
            labels
                .iter()
                .flatten()
                .filter(|label| label.leader.is_some())
                .count(),
            1
        );
        assert_eq!(
            labels
                .iter()
                .flatten()
                .filter(|label| label.reference.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn graph_connection_hit_testing_handles_bends_crossings_and_zoom() {
        let routes = vec![
            vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(100.0, 0.0),
                Pos2::new(100.0, 100.0),
            ],
            vec![Pos2::new(50.0, -50.0), Pos2::new(50.0, 50.0)],
        ];
        assert_eq!(
            graph_edges_at_pointer(&routes, Pos2::new(50.0, 0.0), 6.0),
            [0, 1]
        );
        assert_eq!(
            graph_edges_at_pointer(&routes, Pos2::new(100.0, 80.0), 6.0),
            [0]
        );
        assert!(graph_edges_at_pointer(&routes, Pos2::new(70.0, 70.0), 6.0).is_empty());
        for zoom in [0.1, 1.0, 3.0] {
            assert_eq!(
                graph_edges_at_pointer(
                    &[routes[0].clone()],
                    Pos2::new(20.0, 5.0 / zoom),
                    6.0 / zoom
                ),
                [0]
            );
            assert!(
                graph_edges_at_pointer(
                    &[routes[0].clone()],
                    Pos2::new(20.0, -7.0 / zoom),
                    6.0 / zoom
                )
                .is_empty()
            );
        }
    }

    #[test]
    fn hovering_graph_connection_line_shows_endpoint_information() {
        let graph = layout_graph(
            r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"}]"#,
        );
        let routing = build_graph_routing_layout(&graph);
        let context = egui::Context::default();
        let render = |time, events| {
            context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000.0, 600.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    show_graph(
                        ui,
                        &graph,
                        &routing,
                        &SearchState::default(),
                        Locale::English,
                    );
                },
            )
        };
        let output = render(0.0, Vec::new());
        let pointer = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, .. } if points[0].distance(points[1]) > 80.0 => {
                    Some(points[0] + (points[1] - points[0]) * 0.25)
                }
                _ => None,
            })
            .expect("Graph must draw a connection");
        output.drop_without_applying_deltas();
        let mut visible = false;
        let mut last_texts = Vec::new();
        for frame in 1..=15 {
            let output = render(
                f64::from(frame) * 0.1,
                if frame == 1 {
                    vec![egui::Event::PointerMoved(pointer)]
                } else {
                    Vec::new()
                },
            );
            let texts = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            visible = texts.contains(&"Directed link")
                && texts.contains(&"depends_on")
                && texts.contains(&"Endpoint A / source: Source (a)")
                && texts.contains(&"Endpoint B / target: Target (b)")
                && texts.contains(&graph.nodes[0].path.as_str())
                && texts.contains(&graph.nodes[1].path.as_str());
            last_texts = texts
                .iter()
                .map(|text| text.to_string())
                .collect::<Vec<_>>();
            output.drop_without_applying_deltas();
            if visible {
                break;
            }
        }
        assert!(
            visible,
            "Hovering a connection must expose its label and both endpoints: {pointer:?} {last_texts:?}"
        );
    }

    #[test]
    fn dense_graph_keeps_every_nonempty_relationship_label_inside_canvas() {
        let mut graph = layout_graph(r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#);
        let edge = graph.edges[0].clone();
        graph.edges = (0..24)
            .map(|index| {
                let mut edge = edge.clone();
                edge.label = format!("relationship_{index:02}");
                edge
            })
            .collect();
        let layout = build_graph_routing_layout(&graph);
        let canvas = egui::Rect::from_min_size(Pos2::ZERO, layout.content_size);
        assert!(layout.edge_labels.iter().all(Option::is_some));
        assert!(
            layout.edge_labels.iter().flatten().any(|label| {
                label.background.left()
                    > layout
                        .node_positions
                        .iter()
                        .map(|point| point.x)
                        .max_by(f32::total_cmp)
                        .unwrap()
                        + GRAPH_STEP.x
                        - GRAPH_NODE_SIZE.x / 2.0
                        + 24.0
            }),
            "Fixture must exercise reserved callout placement"
        );
        let mut occupied = Vec::new();
        for (edge, label) in graph.edges.iter().zip(&layout.edge_labels) {
            let label = label.as_ref().unwrap();
            assert_eq!(label.text, edge.label);
            assert!(canvas.contains_rect(label.background));
            assert!(
                occupied
                    .iter()
                    .all(|rect| !label.background.intersects(*rect))
            );
            occupied.push(label.background);
            for position in &layout.node_positions {
                assert!(!label.background.intersects(
                    egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE).expand(2.0),
                ));
            }
            for route in &layout.edge_paths {
                assert!(route.windows(2).all(|segment| {
                    !segment_intersects_rect(segment[0], segment[1], label.background.expand(2.0))
                }));
                assert!(!label.background.intersects(egui::Rect::from_center_size(
                    *route.last().unwrap(),
                    Vec2::splat(20.0),
                )));
            }
        }
        let context = egui::Context::default();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    layout.content_size + Vec2::splat(200.0),
                )),
                ..Default::default()
            },
            |ui| {
                show_graph(
                    ui,
                    &graph,
                    &layout,
                    &SearchState::default(),
                    Locale::English,
                );
            },
        );
        let rendered = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let last_line = output
            .shapes
            .iter()
            .rposition(|shape| matches!(shape.shape, egui::Shape::LineSegment { .. }));
        let first_relationship = output.shapes.iter().position(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.starts_with("relationship_"))
        });
        output.drop_without_applying_deltas();
        for edge in &graph.edges {
            assert!(
                rendered.contains(&edge.label),
                "Relationship must be drawn: {}",
                edge.label
            );
        }
        assert!(
            last_line.unwrap() < first_relationship.unwrap(),
            "Routes and leaders must be drawn before relationship text"
        );
    }

    #[test]
    fn blocked_and_short_routes_keep_legible_callouts_but_empty_edges_stay_unlabelled() {
        let points = [Pos2::new(40.0, 50.0), Pos2::new(45.0, 50.0)];
        let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
        assert!(
            layout_graph_edge_label("long_relationship", &points, canvas, &[canvas], &[], &[])
                .is_none()
        );
        let first = graph_edge_label_callout("long_relationship", &points, canvas, &[]).unwrap();
        let second =
            graph_edge_label_callout("other_relationship", &points, canvas, &[first.background])
                .unwrap();
        assert_eq!(first.text, "long_relationship");
        assert!(first.background.left() > canvas.right());
        assert!(first.leader.is_some());
        assert!(!first.background.intersects(second.background));
        assert!(graph_edge_label_callout("", &points, canvas, &[]).is_none());
        assert!(layout_graph_edge_label("", &points, canvas, &[], &[], &[]).is_none());
    }

    #[test]
    fn graph_toolbar_zoom_multi_selection_and_marquee_work_together() {
        let graph = layout_graph(
            r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"}]"#,
        );
        let routing = build_graph_routing_layout(&graph);
        let context = egui::Context::default();
        let render = |mut events: Vec<egui::Event>, modifiers: egui::Modifiers| {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut state = GraphInteractionState::default();
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let id =
                        ui.make_persistent_id(("graph-interaction", routing.graph_fingerprint));
                    show_graph(
                        ui,
                        &graph,
                        &routing,
                        &SearchState::default(),
                        Locale::English,
                    );
                    state = ui.ctx().data(|data| data.get_temp(id)).unwrap();
                },
            );
            (output, state)
        };
        let position = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.visual_bounding_rect().center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing graph control or label: {label}"))
        };
        let pointer = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let click = |pos, modifiers| {
            vec![
                egui::Event::PointerMoved(pos),
                pointer(pos, true, modifiers),
                pointer(pos, false, modifiers),
            ]
        };
        let none = egui::Modifiers::NONE;
        let (output, _) = render(Vec::new(), none);
        let plus = position(&output, "+");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(plus, none), none);
        assert!((state.zoom - 1.2).abs() < 0.001);
        output.drop_without_applying_deltas();
        let (output, _) = render(Vec::new(), none);
        let reset = position(&output, "100%");
        let source = position(&output, "Source");
        let target = position(&output, "Target");
        assert!((target.x - source.x - GRAPH_STEP.x * 1.2).abs() < 2.0);
        output.drop_without_applying_deltas();
        let (output, state) = render(click(source, none), none);
        assert_eq!(state.selected.len(), 1, "Scaled node must remain clickable");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(source, none), none);
        assert!(state.selected.is_empty());
        output.drop_without_applying_deltas();
        let (output, state) = render(click(reset, none), none);
        assert_eq!(state.zoom, 1.0);
        output.drop_without_applying_deltas();
        let (output, _) = render(Vec::new(), none);
        let source = position(&output, "Source");
        let target = position(&output, "Target");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(source, none), none);
        assert_eq!(state.selected.len(), 1);
        output.drop_without_applying_deltas();
        let ctrl = egui::Modifiers { ctrl: true, ..none };
        let (output, state) = render(click(target, ctrl), ctrl);
        assert_eq!(state.selected.len(), 2);
        output.drop_without_applying_deltas();
        let (output, state) = render(click(source, ctrl), ctrl);
        assert_eq!(state.selected.len(), 1);
        output.drop_without_applying_deltas();
        let (output, _) = render(Vec::new(), none);
        let clear = position(&output, "Clear selection");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(clear, none), none);
        assert!(state.selected.is_empty());
        output.drop_without_applying_deltas();
        let (output, _) = render(Vec::new(), none);
        let select_all = position(&output, "Select all");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(select_all, none), none);
        assert_eq!(state.selected.len(), graph.nodes.len());
        output.drop_without_applying_deltas();
        let (output, state) = render(click(clear, none), none);
        assert!(state.selected.is_empty());
        output.drop_without_applying_deltas();
        let start = source - Vec2::new(120.0, 40.0);
        let end = target + Vec2::new(120.0, 65.0);
        let (output, _) = render(
            vec![egui::Event::PointerMoved(start), pointer(start, true, none)],
            none,
        );
        output.drop_without_applying_deltas();
        let (output, state) = render(vec![egui::Event::PointerMoved(end)], none);
        assert_eq!(state.selected.len(), 2, "Marquee must select both nodes");
        output.drop_without_applying_deltas();
        let (output, _) = render(vec![pointer(end, false, none)], none);
        output.drop_without_applying_deltas();
        let (output, _) = render(Vec::new(), none);
        let fit = position(&output, "Fit graph");
        output.drop_without_applying_deltas();
        let (output, state) = render(click(fit, none), none);
        assert!(routing.content_size.x * state.zoom <= 1000.0);
        assert!(routing.content_size.y * state.zoom <= 600.0);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn graph_timing_details_submenu_keeps_parent_menu_open() {
        let state = GraphCalculationState::default();
        state
            .progress
            .lock()
            .unwrap()
            .timings
            .push((GraphStage::Layout, Duration::from_secs(2)));
        let context = egui::Context::default();
        let render = |events: Vec<egui::Event>| {
            context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1200.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.menu_button(Locale::English.text(TextKey::GraphTimings), |ui| {
                        state.show_progress(ui, Locale::English);
                    });
                },
            )
        };
        let text_position = |output: &egui::FullOutput, expected: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        (text.galley.job.text == expected)
                            .then(|| text.visual_bounding_rect().center())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("Menu text must be visible: {expected}"))
        };
        let click = |position| {
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let output = render(Vec::new());
        let position = text_position(&output, "Calculation stage timings");
        output.drop_without_applying_deltas();
        let output = render(click(position));
        output.drop_without_applying_deltas();
        let output = render(Vec::new());
        let details = text_position(&output, "Stage details");
        output.drop_without_applying_deltas();
        let output = render(click(details));
        output.drop_without_applying_deltas();
        let output = render(Vec::new());
        text_position(
            &output,
            "Slowest completed stage: Preparing layout and connection ports (2.00 s)",
        );
        text_position(&output, "Stage details");
        text_position(&output, "Preparing layout and connection ports: 2.00 s");
        output.drop_without_applying_deltas();
    }

    fn layout_graph(input: &str) -> RelationshipGraph {
        build_relationship_graph(&struct_view_core::parser::parse_json(input).unwrap())
    }

    #[test]
    fn relationship_layout_follows_direction_instead_of_document_order() {
        let graph = layout_graph(
            r#"[{"id":"end"},{"id":"middle","depends_on":"end"},{"id":"start","depends_on":"middle"}]"#,
        );
        let positions = graph_node_positions(&graph);
        for edge in &graph.edges {
            assert!(positions[edge.source].x < positions[edge.target].x);
            assert_eq!(positions[edge.source].y, positions[edge.target].y);
        }
        assert_eq!(positions, graph_node_positions(&graph));
    }

    #[test]
    fn relationship_layout_handles_cycles_disconnected_and_undirected_graphs() {
        for input in [
            r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"a","parent_id":"c"},{"id":"c"},{"id":"isolated"}]"#,
            r#"{"a":["b","c"],"b":["a"],"c":["a"],"isolated":[]}"#,
            r#"[]"#,
            r#"[{"id":"alone"}]"#,
        ] {
            let graph = layout_graph(input);
            let positions = graph_node_positions(&graph);
            assert_eq!(positions.len(), graph.nodes.len());
            assert_eq!(positions, graph_node_positions(&graph));
            for (index, &position) in positions.iter().enumerate() {
                assert!(position.x.is_finite() && position.y.is_finite());
                let rect = egui::Rect::from_center_size(position, GRAPH_NODE_SIZE);
                for &other in &positions[..index] {
                    assert!(!rect.intersects(egui::Rect::from_center_size(other, GRAPH_NODE_SIZE)));
                }
            }
        }
        let graph = layout_graph(
            r#"[{"id":"a","depends_on":"b"},{"id":"b","depends_on":"a","parent_id":"c"},{"id":"c"}]"#,
        );
        let positions = graph_node_positions(&graph);
        assert_eq!(positions[0].x, positions[1].x);
        assert!(positions[2].x > positions[1].x);
    }

    #[test]
    fn partition_layout_keeps_columns_and_orders_neighbors_to_avoid_crossings() {
        let mut graph = layout_graph(
            r#"[{"id":"a","depends_on":"d"},{"id":"b","depends_on":"c"},{"id":"c"},{"id":"d"}]"#,
        );
        graph.partition_names = Some(vec!["left".into(), "right".into()]);
        for (index, node) in graph.nodes.iter_mut().enumerate() {
            node.partition = Some(usize::from(index >= 2));
        }
        let positions = graph_node_positions(&graph);
        assert_eq!(positions[0].x, positions[1].x);
        assert_eq!(positions[2].x, positions[3].x);
        assert_eq!(positions[0].y, positions[3].y);
        assert_eq!(positions[1].y, positions[2].y);
        assert!(positions[0].x < positions[3].x);
    }

    #[test]
    fn graph_activity_omits_timing_diagnostics() {
        let state = GraphCalculationState::default();
        {
            let mut progress = state.progress.lock().unwrap();
            progress.begin(GraphStage::Conflicts, 10, 4);
            progress.completed = 3;
        }
        let context = egui::Context::default();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            state.show_activity(ui, Locale::English);
        });
        let texts = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Checking and resolving route conflicts"));
        assert!(texts.contains(&"3 / 10"));
        assert!(!texts.iter().any(|text| text.contains("Calculation time")
            || text.contains("Routing workers")
            || text.contains("Slowest")));
        output.drop_without_applying_deltas();
    }

    #[test]
    fn graph_progress_records_all_stages_and_completion() {
        let root =
            struct_view_core::parser::parse_json(r#"[{"id":"a","depends_on":"b"},{"id":"b"}]"#)
                .unwrap();
        let progress = Arc::new(Mutex::new(GraphProgress::default()));
        let result = build_graph_calculation(root, &progress);
        let snapshot = progress.lock().unwrap();
        assert_eq!(
            snapshot
                .timings
                .iter()
                .map(|(stage, _)| *stage)
                .collect::<Vec<_>>(),
            [
                GraphStage::Entities,
                GraphStage::Layout,
                GraphStage::Sequential,
                GraphStage::Labels
            ],
        );
        assert_eq!(snapshot.completed, result.graph.edges.len());
        assert_eq!(snapshot.total, result.graph.edges.len());
        assert!(snapshot.stage.is_none());
        assert!(snapshot.finished.unwrap() >= snapshot.timings.iter().map(|(_, time)| *time).sum());
    }

    #[test]
    fn parallel_graph_progress_counts_completed_edges_and_preserves_routes() {
        let (grid, endpoints, ports) = routing_fixture(16);
        let progress = Arc::new(Mutex::new(GraphProgress::default()));
        let routes = route_graph_edges_with_progress(&grid, &endpoints, &ports, 4, Some(&progress));
        assert_eq!(routes, route_graph_edges(&grid, &endpoints, &ports, 4));
        let mut snapshot = progress.lock().unwrap();
        assert_eq!(snapshot.stage, Some(GraphStage::Conflicts));
        assert_eq!(snapshot.completed, endpoints.len());
        assert_eq!(snapshot.total, endpoints.len());
        assert_eq!(snapshot.workers, 4);
        assert_eq!(snapshot.timings[0].0, GraphStage::Preliminary);
        snapshot.finish();
        assert_eq!(snapshot.timings[1].0, GraphStage::Conflicts);
    }

    #[test]
    fn graph_progress_ui_shows_live_counts_and_retains_slowest_stage() {
        let state = GraphCalculationState::default();
        {
            let mut progress = state.progress.lock().unwrap();
            progress.begin(GraphStage::Preliminary, 20, 4);
            progress.completed = 7;
            progress
                .timings
                .push((GraphStage::Layout, Duration::from_secs(2)));
        }
        let context = egui::Context::default();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            state.show_progress(ui, Locale::English);
        });
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(text.contains(&"7 / 20"));
        assert!(text.contains(&"Routing workers: 4"));
        assert!(
            text.iter()
                .any(|text| text.contains("Calculating preliminary routes"))
        );
        output.drop_without_applying_deltas();
        state.progress.lock().unwrap().finish();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            state.show_progress(ui, Locale::English);
        });
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == "Slowest completed stage: Preparing layout and connection ports (2.00 s)")
        }));
        output.drop_without_applying_deltas();
    }

    fn routing_fixture(
        rows: usize,
    ) -> (GraphRoutingGrid, Vec<(usize, usize)>, Vec<GraphEdgePorts>) {
        let positions = (0..rows * 3)
            .map(|index| {
                Pos2::new(
                    128.0 + (index % 3) as f32 * GRAPH_STEP.x,
                    59.0 + (index / 3) as f32 * GRAPH_STEP.y,
                )
            })
            .collect::<Vec<_>>();
        let endpoints = (0..rows)
            .map(|row| (row * 3, row * 3 + 2))
            .collect::<Vec<_>>();
        let ports = graph_edge_ports(&positions, &endpoints);
        (GraphRoutingGrid::new(&positions), endpoints, ports)
    }

    #[test]
    fn indexed_graph_penalties_match_exhaustive_segment_checks() {
        let routes = vec![
            vec![Pos2::new(-300.0, -20.0), Pos2::new(700.0, -20.0)],
            vec![Pos2::new(128.0, -400.0), Pos2::new(128.0, 800.0)],
            vec![Pos2::new(-300.0, 700.0), Pos2::new(700.0, -300.0)],
            vec![Pos2::ZERO, Pos2::ZERO],
            vec![
                Pos2::new(256.0, 0.0),
                Pos2::new(256.0, 128.0),
                Pos2::new(384.0, 128.0),
            ],
        ];
        let index = GraphRouteSegmentIndex::new(&routes);
        for x in [-308.0, -128.0, 0.0, 120.0, 128.0, 136.0, 256.0, 708.0] {
            for y in [-400.0, -28.0, -20.0, -12.0, 0.0, 128.0, 800.0] {
                let start = Pos2::new(x, y);
                for delta in [
                    Vec2::ZERO,
                    Vec2::new(50.0, 0.0),
                    Vec2::new(0.0, 300.0),
                    Vec2::new(-700.0, 400.0),
                ] {
                    let end = start + delta;
                    let expected: f32 = routes
                        .iter()
                        .flat_map(|route| route.windows(2))
                        .map(|segment| segment_pair_penalty(start, end, [segment[0], segment[1]]))
                        .sum();
                    assert_eq!(index.penalty(start, end), expected, "{start:?} -> {end:?}");
                    assert_eq!(index.penalty(end, start), expected);
                }
            }
        }
    }

    #[test]
    fn dense_parallel_conflict_resolution_is_deterministic_and_preserves_ports() {
        let (grid, _, _) = routing_fixture(4);
        let endpoints = vec![(0, 2); 12];
        let ports = graph_edge_ports(&grid.node_positions, &endpoints);
        let expected = route_graph_edges(&grid, &endpoints, &ports, 2);
        for workers in [3, 4, 8] {
            assert_eq!(
                route_graph_edges(&grid, &endpoints, &ports, workers),
                expected
            );
        }
        let serial = route_graph_edges(&grid, &endpoints, &ports, 1);
        for (index, route) in expected.iter().enumerate() {
            assert_eq!(route.first(), serial[index].first());
            assert_eq!(route.last(), serial[index].last());
            for rect in &grid.node_rects {
                assert!(route.windows(2).all(|segment| {
                    !segment_crosses_rect_interior(segment[0], segment[1], *rect)
                }));
            }
            let conflicts = |route: &[Pos2], previous: &[Vec<Pos2>]| {
                previous
                    .iter()
                    .filter(|other| graph_route_conflicts(route, std::slice::from_ref(*other)))
                    .count()
            };
            assert!(
                conflicts(route, &expected[..index]) <= conflicts(&serial[index], &serial[..index])
            );
        }
    }

    #[test]
    fn parallel_graph_routes_are_deterministic_and_keep_separate_tracks() {
        let (grid, endpoints, ports) = routing_fixture(16);
        let expected = route_graph_edges(&grid, &endpoints, &ports, 2);
        for workers in [3, 4, 8] {
            assert_eq!(
                route_graph_edges(&grid, &endpoints, &ports, workers),
                expected
            );
        }
        assert_eq!(expected.len(), endpoints.len());
        for (index, route) in expected.iter().enumerate() {
            assert!(route.len() > 2);
            assert!(!graph_route_conflicts(route, &expected[..index]));
            for rect in &grid.node_rects {
                assert!(route.windows(2).all(|segment| {
                    !segment_crosses_rect_interior(segment[0], segment[1], *rect)
                }));
            }
        }
    }

    #[test]
    fn parallel_graph_routing_resolves_crossing_parallel_and_reciprocal_edges() {
        let (grid, _, _) = routing_fixture(3);
        for endpoints in [vec![(0, 2), (0, 2), (2, 0)], vec![(0, 8), (2, 6)]] {
            let ports = graph_edge_ports(&grid.node_positions, &endpoints);
            let serial = route_graph_edges(&grid, &endpoints, &ports, 1);
            let parallel = route_graph_edges(&grid, &endpoints, &ports, 4);
            assert_eq!(parallel, serial);
            assert_eq!(route_graph_edges(&grid, &endpoints, &ports, 2), parallel);
            let preliminary = endpoints
                .iter()
                .zip(&ports)
                .map(|(&(source, target), &ports)| {
                    grid.route_edge_with_ports(source, target, ports, &[])
                })
                .collect::<Vec<_>>();
            assert_ne!(
                parallel, preliminary,
                "Conflicting candidates must be rerouted"
            );
            for (index, route) in parallel.iter().enumerate() {
                assert_eq!(
                    graph_route_conflicts(route, &parallel[..index]),
                    graph_route_conflicts(&serial[index], &serial[..index]),
                );
                assert!(route.windows(2).all(|segment| segment[0] != segment[1]));
            }
        }
    }

    #[test]
    fn graph_routing_handles_empty_and_small_graphs_without_parallel_workers() {
        assert_eq!(graph_routing_worker_count(0), 1);
        assert_eq!(
            graph_routing_worker_count(GRAPH_PARALLEL_EDGE_THRESHOLD - 1),
            1
        );
        let (grid, _, _) = routing_fixture(1);
        assert!(route_graph_edges(&grid, &[], &[], 8).is_empty());
        let endpoints = [(0, 2)];
        let ports = graph_edge_ports(&grid.node_positions, &endpoints);
        assert_eq!(
            route_graph_edges(&grid, &endpoints, &ports, 1),
            route_graph_edges(&grid, &endpoints, &ports, 8),
        );
    }

    #[test]
    #[ignore = "Manual routing timing comparison; run with --release --ignored --nocapture"]
    fn graph_routing_parallel_timing() {
        let (grid, endpoints, ports) = routing_fixture(96);
        for workers in [1, 2, 4, 8] {
            let started = std::time::Instant::now();
            let routes = route_graph_edges(&grid, &endpoints, &ports, workers);
            println!(
                "Graph routing: {workers} workers, {:?}, {} edges",
                started.elapsed(),
                routes.len()
            );
            for (index, route) in routes.iter().enumerate() {
                assert!(!graph_route_conflicts(route, &routes[..index]));
            }
        }
        let positions = (0..36)
            .map(|index| {
                Pos2::new(
                    128.0 + (index % 6) as f32 * GRAPH_STEP.x,
                    59.0 + (index / 6) as f32 * GRAPH_STEP.y,
                )
            })
            .collect::<Vec<_>>();
        let endpoints = (0..36)
            .flat_map(|index| [(index, (index + 1) % 36), (index, (index + 7) % 36)])
            .collect::<Vec<_>>();
        let grid = GraphRoutingGrid::new(&positions);
        let ports = graph_edge_ports(&positions, &endpoints);
        for workers in [1, 2, 4, 8] {
            let started = std::time::Instant::now();
            let routes = route_graph_edges(&grid, &endpoints, &ports, workers);
            println!(
                "Dense graph routing: {workers} workers, {:?}, {} edges",
                started.elapsed(),
                routes.len(),
            );
            assert_eq!(routes.len(), endpoints.len());
        }
        let positions = (0..64)
            .flat_map(|group| {
                let top = 59.0 + group as f32 * GRAPH_STEP.y * 3.0;
                [
                    Pos2::new(128.0, top),
                    Pos2::new(468.0, top),
                    Pos2::new(128.0, top + GRAPH_STEP.y),
                    Pos2::new(468.0, top + GRAPH_STEP.y),
                ]
            })
            .collect::<Vec<_>>();
        let endpoints = (0..64)
            .map(|group| (group * 4, group * 4 + 3))
            .chain((0..64).map(|group| (group * 4 + 1, group * 4 + 2)))
            .collect::<Vec<_>>();
        let grid = GraphRoutingGrid::new(&positions);
        let ports = graph_edge_ports(&positions, &endpoints);
        for workers in [2, 4, 8] {
            let progress = Arc::new(Mutex::new(GraphProgress::default()));
            let routes = route_graph_edges_with_progress(
                &grid,
                &endpoints,
                &ports,
                workers,
                Some(&progress),
            );
            let mut snapshot = progress.lock().unwrap();
            snapshot.finish();
            let conflict_time = snapshot
                .timings
                .iter()
                .find(|(stage, _)| *stage == GraphStage::Conflicts)
                .unwrap()
                .1;
            println!(
                "Independent conflict groups: {workers} workers, conflict stage {conflict_time:?}"
            );
            assert_eq!(routes.len(), endpoints.len());
            for (index, route) in routes.iter().enumerate() {
                assert!(!graph_route_conflicts(route, &routes[..index]));
            }
        }
    }

    #[test]
    fn graph_labels_remain_visible_without_covering_routes_or_arrowheads() {
        for input in [
            r#"[{"id":"source","depends_on":"target","parent_id":"target","ref":"target"},{"id":"target"}]"#,
            r#"[{"id":"tl","depends_on":"br"},{"id":"tr","parent_id":"bl"},{"id":"bl"},{"id":"br"}]"#,
            r#"[{"id":"left","depends_on":"right"},{"id":"right","depends_on":"left"}]"#,
        ] {
            let root = struct_view_core::parser::parse_json(input).unwrap();
            let graph = build_relationship_graph(&root);
            let layout = build_graph_routing_layout(&graph);
            assert_eq!(
                layout.edge_labels.iter().flatten().count(),
                graph.edges.len(),
                "Every relationship must retain a label: {input}",
            );
            for label in layout.edge_labels.iter().flatten() {
                for route in &layout.edge_paths {
                    assert!(
                        route.windows(2).all(|segment| {
                            !segment_intersects_rect(
                                segment[0],
                                segment[1],
                                label.background.expand(2.0),
                            )
                        }),
                        "Label {} obscures a route",
                        label.text
                    );
                    assert!(
                        !label.background.intersects(egui::Rect::from_center_size(
                            *route.last().unwrap(),
                            Vec2::splat(20.0),
                        )),
                        "Label {} obscures an arrowhead",
                        label.text
                    );
                }
                for position in &layout.node_positions {
                    assert!(!label.background.intersects(
                        egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE).expand(2.0),
                    ));
                }
            }
        }
    }

    #[test]
    fn displaced_graph_labels_point_to_their_own_route() {
        let points = [egui::pos2(40.0, 100.0), egui::pos2(240.0, 100.0)];
        let blocking_label =
            egui::Rect::from_min_max(egui::pos2(0.0, 70.0), egui::pos2(300.0, 130.0));
        let label = layout_graph_edge_label(
            "depends_on",
            &points,
            egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0)),
            &[],
            &[blocking_label],
            &[points.to_vec()],
        )
        .expect("A displaced relationship label must still be visible");
        let [anchor, end] = label
            .leader
            .expect("Displaced labels must identify their route");
        assert_eq!(anchor.y, 100.0);
        assert!((40.0..=240.0).contains(&anchor.x));
        assert!(label.background.expand(0.001).contains(end));
        assert!(!label.background.shrink(0.001).contains(end));
    }
}
