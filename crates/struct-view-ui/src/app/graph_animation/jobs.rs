use super::realtime::{self, RealtimeState};
use super::{ElementStatus, RealtimeCommand, RealtimeUpdate};
use crate::app::visualization::RelationshipGraph;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const STATE_ID: &str = "graph-animation-jobs";
pub(super) const MAX_JOBS: usize = 64;
#[cfg(test)]
mod tests;
mod view;
pub(in crate::app) use view::show_jobs;

/// Full job snapshots. Progress is local to a stage and its optional route.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobCommand {
    Upsert {
        id: String,
        label: String,
        stage: String,
        status: ElementStatus,
        #[serde(default)]
        nodes: Vec<String>,
        #[serde(default)]
        edge: Option<usize>,
        #[serde(default, deserialize_with = "realtime::deserialize_percent")]
        percent: f64,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        color: Option<[u8; 3]>,
        #[serde(default = "default_smoothing")]
        smoothing_ms: u64,
    },
    Remove {
        id: String,
    },
    Clear,
}

fn default_smoothing() -> u64 {
    300
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JobUpdate {
    pub graph_id: String,
    pub commands: Vec<JobCommand>,
}

#[derive(Clone, Serialize)]
pub(in crate::app) struct Job {
    pub id: String,
    pub label: String,
    pub stage: String,
    pub status: ElementStatus,
    pub nodes: Vec<String>,
    pub edge: Option<usize>,
    pub percent: f64,
    pub reverse: bool,
    pub color: [u8; 3],
    pub smoothing_ms: u64,
}

#[derive(Clone)]
pub(in crate::app) struct JobsState {
    pub graph_id: String,
    pub jobs: BTreeMap<String, Job>,
    pub visuals: RealtimeState,
}

impl JobsState {
    pub(super) fn apply(
        previous: Option<Self>,
        update: JobUpdate,
        graph: &RelationshipGraph,
        now: f64,
    ) -> Result<Self, String> {
        if update.commands.is_empty() || update.commands.len() > MAX_JOBS {
            return Err(format!("commands must contain 1..={MAX_JOBS} entries"));
        }
        let mut state =
            if let Some(previous) = previous.filter(|state| state.graph_id == update.graph_id) {
                previous
            } else {
                Self {
                    visuals: RealtimeState::apply(
                        None,
                        RealtimeUpdate {
                            graph_id: update.graph_id.clone(),
                            commands: vec![RealtimeCommand::Clear],
                        },
                        graph,
                        now,
                    )?,
                    graph_id: update.graph_id,
                    jobs: BTreeMap::new(),
                }
            };
        for command in update.commands {
            let mut commands = Vec::new();
            match command {
                JobCommand::Upsert {
                    id,
                    label,
                    stage,
                    status,
                    nodes,
                    edge,
                    percent,
                    reverse,
                    color,
                    smoothing_ms,
                } => {
                    realtime::validate_id(&id)?;
                    realtime::validate_metadata(&Some(label.clone()), &Some(stage.clone()))?;
                    if !(16..=10_000).contains(&smoothing_ms) {
                        return Err("smoothing_ms must be 16..=10000".into());
                    }
                    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                        return Err("percent must be a finite number in 0..=100".into());
                    }
                    if nodes.is_empty() && edge.is_none() {
                        return Err("job needs nodes or an edge".into());
                    }
                    let previous = state.jobs.get(&id);
                    let same_stage = previous.is_some_and(|job| {
                        job.stage == stage && job.edge == edge && job.reverse == reverse
                    });
                    let percent = if same_stage {
                        previous.map_or(percent, |job| job.percent.max(percent))
                    } else {
                        percent
                    };
                    if !same_stage && previous.is_some_and(|job| job.edge.is_some()) {
                        commands.push(RealtimeCommand::RemoveMarker { id: id.clone() });
                    }
                    let color = color
                        .or_else(|| previous.map(|job| job.color))
                        .unwrap_or_else(|| job_color(&id));
                    if let Some(edge) = edge {
                        commands.push(RealtimeCommand::Marker {
                            id: id.clone(),
                            edge,
                            percent,
                            smoothing_ms,
                            color,
                            reverse,
                            label: Some(label.clone()),
                            stage: Some(stage.clone()),
                        });
                    }
                    commands.push(RealtimeCommand::Element {
                        id: id.clone(),
                        nodes: nodes.clone(),
                        edges: edge.into_iter().collect(),
                        status: Some(status),
                        color: Some(color),
                        pulse: Some(status == ElementStatus::Running),
                        label: Some(label.clone()),
                        stage: Some(stage.clone()),
                    });
                    state.jobs.insert(
                        id.clone(),
                        Job {
                            id,
                            label,
                            stage,
                            status,
                            nodes,
                            edge,
                            percent,
                            reverse,
                            color,
                            smoothing_ms,
                        },
                    );
                }
                JobCommand::Remove { id } => {
                    let job = state
                        .jobs
                        .remove(&id)
                        .ok_or_else(|| format!("Unknown job ID: {id}"))?;
                    commands.push(RealtimeCommand::RemoveElement { id: id.clone() });
                    if job.edge.is_some() {
                        commands.push(RealtimeCommand::RemoveMarker { id });
                    }
                }
                JobCommand::Clear => {
                    state.jobs.clear();
                    commands.push(RealtimeCommand::Clear);
                }
            }
            if state.jobs.len() > MAX_JOBS {
                return Err(format!("At most {MAX_JOBS} jobs may exist"));
            }
            state.visuals = RealtimeState::apply(
                Some(state.visuals),
                RealtimeUpdate {
                    graph_id: state.graph_id.clone(),
                    commands,
                },
                graph,
                now,
            )?;
        }
        Ok(state)
    }

    pub(super) fn descriptor(&mut self, now: f64) -> Value {
        self.visuals.advance(now);
        json!({
            "graph_id": self.graph_id,
            "jobs": self.jobs.values().map(|job| {
                let mut value = serde_json::to_value(job).expect("validated job serializes");
                let current = if job.edge.is_some() {
                    self.visuals.marker_percent(&job.id).expect("route job has a marker")
                } else {
                    job.percent
                };
                value["current_percent"] = json!(current);
                value
            }).collect::<Vec<_>>()
        })
    }
}

fn job_color(id: &str) -> [u8; 3] {
    let palette = [
        [40, 160, 255],
        [255, 160, 40],
        [175, 110, 255],
        [40, 190, 160],
    ];
    let hash = id.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte.into())
    });
    palette[hash % palette.len()]
}

pub(in crate::app) fn state(ctx: &egui::Context) -> Option<JobsState> {
    ctx.data(|data| data.get_temp(egui::Id::new(STATE_ID)))
}

pub(in crate::app) fn store(ctx: &egui::Context, state: JobsState) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(STATE_ID), state));
}

pub(in crate::app) fn clear(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<JobsState>(egui::Id::new(STATE_ID)));
}
