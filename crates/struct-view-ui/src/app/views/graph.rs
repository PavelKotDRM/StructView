use super::*;

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use struct_view_core::graph::EdgeDirection;
use struct_view_core::parser::JsonNode;

mod canvas;
mod export;
mod labels;
mod layout;
mod routing;

pub(in crate::app) use canvas::show_graph;
#[cfg(test)]
use canvas::{GraphInteractionState, graph_edges_at_pointer};
pub(in crate::app) use export::render_graph_image;
pub(in crate::app) use export::{GraphExportFormat, GraphExportStyle, export_graph_image};
#[cfg(test)]
use labels::aligned_label_rect;
use labels::{
    GraphEdgeLabelLayout, graph_edge_label_callout, graph_edge_label_callout_text,
    graph_edge_label_layout, layout_graph_edge_label, place_edge_label,
    resolve_graph_label_leaders, shorten_to_width,
};
#[cfg(test)]
pub(super) use layout::build_graph_routing_layout;
use layout::build_graph_routing_layout_with_progress;
#[cfg(test)]
use layout::graph_node_positions;
#[cfg(not(test))]
use routing::segments_within_clearance;
#[cfg(test)]
pub(super) use routing::{
    GRAPH_EDGE_CLEARANCE, GRAPH_ROUTE_CLEARANCE, box_border_offset, segment_crosses_rect_interior,
    segments_within_clearance,
};
pub(super) use routing::{GraphRoutingGrid, graph_edge_color, graph_edge_ports};
use routing::{
    arrow_head_wings, closest_point_on_segment, draw_arrow_head, edge_arrowheads,
    graph_routing_worker_count, point_to_segment_distance, relationship_graph_fingerprint,
    route_graph_edges_with_progress, segment_intersects_rect, segments_intersect,
};

pub(super) const GRAPH_DIM_FACTOR: f32 = 0.18;

pub(in crate::app) fn headless_graph_image(
    root: &JsonNode,
    png: bool,
    dark: bool,
) -> Result<Vec<u8>, String> {
    let graph = try_build_relationship_graph(root)?;
    if graph.nodes.is_empty() {
        return Err("No graph entities found".to_string());
    }
    let routing = build_graph_routing_layout_with_progress(&graph, None);
    render_graph_image(
        &graph,
        &routing,
        if png {
            GraphExportFormat::Png
        } else {
            GraphExportFormat::Svg
        },
        if dark {
            GraphExportStyle::DarkOpaque
        } else {
            GraphExportStyle::LightTransparent
        },
    )
    .map_err(|error| format!("Graph export error: {error}"))
}
const GRAPH_EDGE_LABEL_CHAR_WIDTH: f32 = 8.0;
const GRAPH_EDGE_LABEL_HEIGHT: f32 = 16.0;

#[derive(Default)]
pub(in crate::app) struct GraphCalculationState {
    receiver: Option<Receiver<Result<GraphCalculationResult, String>>>,
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
            Ok(Ok(result)) => {
                self.receiver = None;
                self.result = Some(result);
                ctx.request_repaint();
            }
            Ok(Err(error)) => {
                self.receiver = None;
                self.error = Some(error);
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
) -> Result<GraphCalculationResult, String> {
    begin_graph_stage(Some(progress), GraphStage::Entities, 0, 1);
    let graph = match try_build_relationship_graph(&root) {
        Ok(graph) => graph,
        Err(error) => {
            progress
                .lock()
                .expect("graph progress lock poisoned")
                .finish();
            return Err(error);
        }
    };
    let routing = build_graph_routing_layout_with_progress(&graph, Some(progress));
    progress
        .lock()
        .expect("graph progress lock poisoned")
        .finish();
    Ok(GraphCalculationResult { graph, routing })
}
