use super::*;

mod progress;
pub(super) use progress::{
    GraphProgress, GraphProgressTracker, GraphStage, advance_graph_progress, begin_graph_stage,
};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::app::headless::GraphProgressSnapshot;
use struct_view_core::parser::JsonNode;

pub(in crate::app) fn headless_graph_image(
    root: &JsonNode,
    png: bool,
    dark: bool,
) -> Result<Vec<u8>, String> {
    headless_graph_image_with_progress(root, png, dark, None)
}

pub(in crate::app) fn headless_graph_image_with_progress(
    root: &JsonNode,
    png: bool,
    dark: bool,
    report_progress: Option<std::sync::Arc<dyn Fn(GraphProgressSnapshot) + Send + Sync>>,
) -> Result<Vec<u8>, String> {
    let progress = Arc::new(Mutex::new(GraphProgress::default()));
    let reporter = if let Some(report_progress) = report_progress {
        let progress = Arc::clone(&progress);
        Some(
            thread::Builder::new()
                .name("struct-view-graph-progress".to_string())
                .spawn(move || {
                    loop {
                        let snapshot = {
                            let progress = progress.lock().expect("graph progress lock poisoned");
                            GraphProgressSnapshot::from(&progress)
                        };
                        let finished = snapshot.finished;
                        report_progress(snapshot);
                        if finished {
                            break;
                        }
                        thread::sleep(Duration::from_millis(250));
                    }
                })
                .map_err(|error| format!("Cannot start graph progress reporter: {error}"))?,
        )
    } else {
        None
    };
    let result = calculate_graph(
        root.clone(),
        GraphRoutingWorkerSetting::Automatic,
        RoutingSearchBackend::Builtin,
        &progress,
    );
    let calculation = match result {
        Ok(calculation) => calculation,
        Err(error) => {
            progress
                .lock()
                .expect("graph progress lock poisoned")
                .finish();
            if let Some(reporter) = reporter {
                reporter
                    .join()
                    .map_err(|_| "Graph progress reporter terminated unexpectedly".to_string())?;
            }
            return Err(error);
        }
    };
    begin_graph_stage(Some(&progress), GraphStage::Rendering, 0, 1);
    let rendered = render_graph_image(
        &calculation.graph,
        &calculation.routing,
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
        Locale::English,
    )
    .map_err(|error| format!("Graph export error: {error}"));
    progress
        .lock()
        .expect("graph progress lock poisoned")
        .finish();
    if let Some(reporter) = reporter {
        reporter
            .join()
            .map_err(|_| "Graph progress reporter terminated unexpectedly".to_string())?;
    }
    rendered
}

#[derive(Default)]
pub(in crate::app) struct GraphCalculationState {
    receiver: Option<Receiver<Result<GraphCalculationResult, String>>>,
    result: Option<GraphCalculationResult>,
    error: Option<String>,
    pub(super) progress: Arc<Mutex<GraphProgress>>,
}

impl GraphCalculationState {
    pub(in crate::app) fn ensure_started(
        &mut self,
        root: &JsonNode,
        worker_setting: GraphRoutingWorkerSetting,
        search_backend: RoutingSearchBackend,
        ctx: &egui::Context,
    ) {
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
                let result =
                    build_graph_calculation(root, worker_setting, search_backend, &progress);
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
    pub(super) graph_fingerprint: u64,
    pub(super) node_positions: Vec<Pos2>,
    pub(super) node_sizes: Vec<Vec2>,
    pub(super) link_counts: Vec<(usize, usize)>,
    pub(super) edge_paths: Vec<Vec<Pos2>>,
    pub(super) edge_labels: Vec<Option<GraphEdgeLabelLayout>>,
    pub(super) partition_labels: Option<Vec<String>>,
    pub(super) content_size: Vec2,
}

pub(super) fn build_graph_calculation(
    root: JsonNode,
    worker_setting: GraphRoutingWorkerSetting,
    search_backend: RoutingSearchBackend,
    progress: &GraphProgressTracker,
) -> Result<GraphCalculationResult, String> {
    let result = calculate_graph(root, worker_setting, search_backend, progress);
    progress
        .lock()
        .expect("graph progress lock poisoned")
        .finish();
    result
}

fn calculate_graph(
    root: JsonNode,
    worker_setting: GraphRoutingWorkerSetting,
    search_backend: RoutingSearchBackend,
    progress: &GraphProgressTracker,
) -> Result<GraphCalculationResult, String> {
    begin_graph_stage(Some(progress), GraphStage::Entities, 0, 1);
    let graph = try_build_relationship_graph(&root)?;
    let routing = build_graph_routing_layout_with_progress(
        &graph,
        worker_setting,
        search_backend,
        Some(progress),
    )?;
    Ok(GraphCalculationResult { graph, routing })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_graph_calculation_completes_in_the_background() {
        let nodes = (0..6)
            .map(|id| serde_json::json!({"id": id.to_string()}))
            .collect::<Vec<_>>();
        let edges = (0..6)
            .flat_map(|source| {
                (0..6).map(move |target| {
                    serde_json::json!({
                        "source": source.to_string(), "target": target.to_string()
                    })
                })
            })
            .collect::<Vec<_>>();
        let input = serde_json::json!({"nodes": nodes, "edges": edges}).to_string();
        let root = struct_view_core::parser::parse_json(&input).unwrap();
        let mut state = GraphCalculationState::default();
        let context = egui::Context::default();
        state.ensure_started(
            &root,
            GraphRoutingWorkerSetting::Manual(4),
            RoutingSearchBackend::Builtin,
            &context,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while state.result().is_none() && state.error().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "graph calculation timed out"
            );
            state.poll(&context);
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(state.error().is_none(), "{:?}", state.error());
        assert_eq!(state.result().unwrap().routing.edge_paths.len(), 36);
        assert!(state.progress.lock().unwrap().finished.is_some());
    }

    #[test]
    fn image_progress_remains_active_between_calculation_and_rendering() {
        let root = struct_view_core::parser::parse_json(r#"[{"id":"a"}]"#).unwrap();
        let progress = Arc::new(Mutex::new(GraphProgress::default()));
        calculate_graph(
            root,
            GraphRoutingWorkerSetting::Automatic,
            RoutingSearchBackend::Builtin,
            &progress,
        )
        .unwrap();

        assert!(!GraphProgressSnapshot::from(&progress.lock().unwrap()).finished);
        begin_graph_stage(Some(&progress), GraphStage::Rendering, 0, 1);
        assert!(!GraphProgressSnapshot::from(&progress.lock().unwrap()).finished);
        progress.lock().unwrap().finish();
        let snapshot = GraphProgressSnapshot::from(&progress.lock().unwrap());
        assert!(snapshot.finished);
        assert!(
            snapshot
                .timings
                .iter()
                .any(|(stage, _)| *stage == "Rendering graph image")
        );
    }
}
