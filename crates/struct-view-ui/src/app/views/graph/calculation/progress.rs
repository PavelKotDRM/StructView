use super::*;
use crate::app::headless::GraphProgressSnapshot;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app::views::graph) enum GraphStage {
    Entities,
    Layout,
    Preliminary,
    Conflicts,
    Sequential,
    Labels,
    Rendering,
}

impl GraphStage {
    fn label(self) -> &'static str {
        match self {
            Self::Entities => "Extracting entities and relationships",
            Self::Layout => "Preparing graph layout",
            Self::Preliminary => "Calculating preliminary routes",
            Self::Conflicts => "Checking and resolving route conflicts",
            Self::Sequential => "Routing links sequentially",
            Self::Labels => "Placing relationship labels",
            Self::Rendering => "Rendering graph image",
        }
    }

    pub(in crate::app::views::graph) fn text_key(self) -> TextKey {
        match self {
            Self::Entities => TextKey::GraphStageEntities,
            Self::Layout => TextKey::GraphStageLayout,
            Self::Preliminary => TextKey::GraphStagePreliminary,
            Self::Conflicts => TextKey::GraphStageConflicts,
            Self::Sequential => TextKey::GraphStageSequential,
            Self::Labels => TextKey::GraphStageLabels,
            Self::Rendering => TextKey::GraphStageRendering,
        }
    }
}

pub(in crate::app::views::graph) struct GraphProgress {
    pub(in crate::app::views::graph) started: Instant,
    pub(in crate::app::views::graph) stage_started: Instant,
    pub(in crate::app::views::graph) stage: Option<GraphStage>,
    pub(in crate::app::views::graph) completed: usize,
    pub(in crate::app::views::graph) total: usize,
    pub(in crate::app::views::graph) workers: usize,
    pub(in crate::app::views::graph) timings: Vec<(GraphStage, Duration)>,
    pub(in crate::app::views::graph) finished: Option<Duration>,
}

impl GraphProgressSnapshot {
    pub(in crate::app::views::graph) fn from(progress: &GraphProgress) -> Self {
        Self {
            stage: progress.stage.map(GraphStage::label),
            completed: progress.completed,
            total: progress.total,
            workers: progress.workers,
            elapsed: progress
                .finished
                .unwrap_or_else(|| progress.started.elapsed())
                .as_secs_f64(),
            stage_elapsed: progress
                .stage
                .map_or(0.0, |_| progress.stage_started.elapsed().as_secs_f64()),
            timings: progress
                .timings
                .iter()
                .map(|(stage, duration)| (stage.label(), duration.as_secs_f64()))
                .collect(),
            finished: progress.finished.is_some(),
        }
    }
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
    pub(in crate::app::views::graph) fn begin(
        &mut self,
        stage: GraphStage,
        total: usize,
        workers: usize,
    ) {
        self.end_stage();
        self.finished = None;
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

    pub(in crate::app::views::graph) fn finish(&mut self) {
        self.end_stage();
        self.finished = Some(self.started.elapsed());
    }
}

pub(in crate::app::views::graph) type GraphProgressTracker = Arc<Mutex<GraphProgress>>;

pub(in crate::app::views::graph) fn begin_graph_stage(
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

pub(in crate::app::views::graph) fn advance_graph_progress(
    progress: Option<&GraphProgressTracker>,
) {
    if let Some(progress) = progress {
        progress
            .lock()
            .expect("graph progress lock poisoned")
            .completed += 1;
    }
}
