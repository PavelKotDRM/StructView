use super::{AnimationCanvas, MAX_STEPS, default_color, point_on_path};
use crate::app::i18n::{Locale, TextKey};
use crate::app::visualization::RelationshipGraph;
use egui::{Color32, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const STATE_ID: &str = "graph-realtime-animation";
const MAX_ITEMS: usize = 256;

fn short_text(text: &str, limit: usize) -> String {
    if text.chars().count() > limit {
        format!("{}…", text.chars().take(limit).collect::<String>())
    } else {
        text.to_owned()
    }
}

type StatusAssignments = Vec<(
    String,
    Option<String>,
    Option<String>,
    Option<ElementStatus>,
)>;
type MarkerProgress = Vec<(String, Option<String>, Option<String>, f64, f64)>;

fn default_smoothing() -> u64 {
    300
}

pub(super) fn deserialize_percent<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let number = serde_json::Number::deserialize(deserializer)?;
    number
        .as_f64()
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
        .ok_or_else(|| serde::de::Error::custom("percent must be a finite number in 0..=100"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementStatus {
    Waiting,
    Running,
    Success,
    Error,
}

impl ElementStatus {
    fn appearance(self) -> ([u8; 3], bool, TextKey) {
        match self {
            Self::Waiting => ([150, 160, 180], false, TextKey::GraphStatusWaiting),
            Self::Running => ([40, 160, 255], true, TextKey::GraphStatusRunning),
            Self::Success => ([60, 200, 110], false, TextKey::GraphStatusSuccess),
            Self::Error => ([240, 70, 70], true, TextKey::GraphStatusError),
        }
    }

    pub(in crate::app) fn text_key(self) -> TextKey {
        self.appearance().2
    }

    pub(in crate::app) fn color(self) -> Color32 {
        let [r, g, b] = self.appearance().0;
        Color32::from_rgb(r, g, b)
    }
}

/// Persistent visual commands, independent of the scheduled animation.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RealtimeCommand {
    Marker {
        id: String,
        edge: usize,
        #[serde(deserialize_with = "deserialize_percent")]
        percent: f64,
        #[serde(default = "default_smoothing")]
        smoothing_ms: u64,
        #[serde(default = "default_color")]
        color: [u8; 3],
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        stage: Option<String>,
    },
    Element {
        id: String,
        #[serde(default)]
        nodes: Vec<String>,
        #[serde(default)]
        edges: Vec<usize>,
        #[serde(default)]
        status: Option<ElementStatus>,
        #[serde(default)]
        color: Option<[u8; 3]>,
        #[serde(default)]
        pulse: Option<bool>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        stage: Option<String>,
    },
    RemoveMarker {
        id: String,
    },
    RemoveElement {
        id: String,
    },
    Clear,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeUpdate {
    pub graph_id: String,
    pub commands: Vec<RealtimeCommand>,
}

#[derive(Clone)]
struct Marker {
    edge: usize,
    current: f64,
    target: f64,
    updated: f64,
    smoothing_ms: u64,
    color: [u8; 3],
    reverse: bool,
    label: Option<String>,
    stage: Option<String>,
}

impl Marker {
    fn advance(&mut self, now: f64) {
        let elapsed = (now - self.updated).max(0.0);
        self.updated = now.max(self.updated);
        // A changing target never resets the visible position or restarts a tween.
        self.current +=
            (self.target - self.current) * -(-elapsed * 1000.0 / self.smoothing_ms as f64).exp_m1();
        if self.target - self.current < 0.01 {
            self.current = self.target;
        }
    }
}

#[derive(Clone)]
struct Element {
    nodes: Vec<usize>,
    edges: Vec<usize>,
    status: Option<ElementStatus>,
    color: [u8; 3],
    pulse: bool,
    label: Option<String>,
    stage: Option<String>,
}

#[derive(Clone)]
pub(in crate::app) struct RealtimeState {
    graph_id: String,
    markers: BTreeMap<String, Marker>,
    elements: BTreeMap<String, Element>,
}

pub(super) fn node_index(graph: &RelationshipGraph, id: &str) -> Result<usize, String> {
    let mut matches = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.id == id);
    let index = matches
        .next()
        .map(|(index, _)| index)
        .ok_or_else(|| format!("Unknown node ID: {id}"))?;
    if matches.next().is_some() {
        return Err(format!("Ambiguous node ID: {id}"));
    }
    Ok(index)
}

pub(super) fn edge_index(graph: &RelationshipGraph, index: usize) -> Result<usize, String> {
    if index >= graph.edges.len() {
        Err(format!("Unknown edge index: {index}"))
    } else {
        Ok(index)
    }
}

pub(super) fn validate_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() || id.len() > 128 {
        Err("Visual ID must be nonempty and at most 128 UTF-8 bytes".into())
    } else {
        Ok(())
    }
}

pub(super) fn validate_metadata(
    label: &Option<String>,
    stage: &Option<String>,
) -> Result<(), String> {
    for (name, value) in [("label", label), ("stage", stage)] {
        if value
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err(format!(
                "{name} must be nonempty and at most 256 UTF-8 bytes"
            ));
        }
    }
    Ok(())
}

impl RealtimeState {
    pub(in crate::app) fn with_jobs(mut self, jobs: &Self) -> Self {
        self.markers = self
            .markers
            .into_iter()
            .map(|(id, marker)| (format!("realtime/{id}"), marker))
            .collect();
        self.elements = self
            .elements
            .into_iter()
            .map(|(id, element)| (format!("realtime/{id}"), element))
            .collect();
        for (id, marker) in &jobs.markers {
            self.markers.insert(format!("job/{id}"), marker.clone());
        }
        for (id, element) in &jobs.elements {
            self.elements.insert(format!("job/{id}"), element.clone());
        }
        self
    }

    pub(in crate::app) fn apply(
        previous: Option<Self>,
        update: RealtimeUpdate,
        graph: &RelationshipGraph,
        now: f64,
    ) -> Result<Self, String> {
        if update.commands.is_empty() || update.commands.len() > MAX_STEPS {
            return Err(format!("commands must contain 1..={MAX_STEPS} entries"));
        }
        let mut state = previous
            .filter(|state| state.graph_id == update.graph_id)
            .unwrap_or_else(|| Self {
                graph_id: update.graph_id,
                markers: BTreeMap::new(),
                elements: BTreeMap::new(),
            });
        state.advance(now);
        for command in update.commands {
            match command {
                RealtimeCommand::Marker {
                    id,
                    edge,
                    percent,
                    smoothing_ms,
                    color,
                    reverse,
                    label,
                    stage,
                } => {
                    validate_id(&id)?;
                    validate_metadata(&label, &stage)?;
                    edge_index(graph, edge)?;
                    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                        return Err("percent must be a finite number in 0..=100".into());
                    }
                    if !(16..=10_000).contains(&smoothing_ms) {
                        return Err("smoothing_ms must be 16..=10000".into());
                    }
                    if let Some(marker) = state.markers.get_mut(&id) {
                        if marker.edge != edge || marker.reverse != reverse {
                            return Err(
                                "Remove the marker before changing its edge or direction".into()
                            );
                        }
                        marker.target = marker.target.max(percent);
                        marker.smoothing_ms = smoothing_ms;
                        marker.color = color;
                        if label.is_some() {
                            marker.label = label;
                        }
                        if stage.is_some() {
                            marker.stage = stage;
                        }
                    } else {
                        state.markers.insert(
                            id,
                            Marker {
                                edge,
                                current: 0.0,
                                target: percent,
                                updated: now,
                                smoothing_ms,
                                color,
                                reverse,
                                label,
                                stage,
                            },
                        );
                    }
                }
                RealtimeCommand::Element {
                    id,
                    nodes,
                    edges,
                    status,
                    color,
                    pulse,
                    label,
                    stage,
                } => {
                    validate_id(&id)?;
                    validate_metadata(&label, &stage)?;
                    if nodes.is_empty() && edges.is_empty() || nodes.len() + edges.len() > 1024 {
                        return Err("element needs 1..=1024 targets".into());
                    }
                    let nodes = nodes
                        .iter()
                        .map(|id| node_index(graph, id))
                        .collect::<Result<_, _>>()?;
                    let edges = edges
                        .into_iter()
                        .map(|index| edge_index(graph, index))
                        .collect::<Result<_, _>>()?;
                    let (status_color, status_pulse) = status
                        .map(|status| {
                            let (color, pulse, _) = status.appearance();
                            (color, pulse)
                        })
                        .unwrap_or((default_color(), false));
                    state.elements.insert(
                        id,
                        Element {
                            nodes,
                            edges,
                            status,
                            color: color.unwrap_or(status_color),
                            pulse: pulse.unwrap_or(status_pulse),
                            label,
                            stage,
                        },
                    );
                }
                RealtimeCommand::RemoveMarker { id } => {
                    if state.markers.remove(&id).is_none() {
                        return Err(format!("Unknown marker ID: {id}"));
                    }
                }
                RealtimeCommand::RemoveElement { id } => {
                    if state.elements.remove(&id).is_none() {
                        return Err(format!("Unknown element effect ID: {id}"));
                    }
                }
                RealtimeCommand::Clear => {
                    state.markers.clear();
                    state.elements.clear();
                }
            }
            if state.markers.len() + state.elements.len() > MAX_ITEMS {
                return Err(format!(
                    "At most {MAX_ITEMS} realtime markers and element effects"
                ));
            }
        }
        Ok(state)
    }

    pub(in crate::app) fn matches_graph(&self, fingerprint: u64) -> bool {
        self.graph_id == format!("{fingerprint:016x}")
    }
    pub(super) fn graph_id(&self) -> &str {
        &self.graph_id
    }

    pub(in crate::app) fn advance(&mut self, now: f64) {
        for marker in self.markers.values_mut() {
            marker.advance(now);
        }
    }

    pub(in crate::app) fn needs_repaint(&self) -> bool {
        self.markers
            .values()
            .any(|marker| marker.current < marker.target)
            || self.elements.values().any(|element| element.pulse)
    }

    pub(super) fn marker_percent(&self, id: &str) -> Option<f64> {
        self.markers.get(id).map(|marker| marker.current)
    }

    pub(in crate::app) fn statuses_for_node(&self, index: usize) -> StatusAssignments {
        self.elements
            .iter()
            .filter(|(_, element)| element.nodes.contains(&index))
            .map(|(id, element)| {
                (
                    id.clone(),
                    element.label.clone(),
                    element.stage.clone(),
                    element.status,
                )
            })
            .collect()
    }

    pub(in crate::app) fn statuses_for_edge(
        &self,
        index: usize,
    ) -> (StatusAssignments, MarkerProgress) {
        let statuses = self
            .elements
            .iter()
            .filter(|(_, element)| element.edges.contains(&index))
            .map(|(id, element)| {
                (
                    id.clone(),
                    element.label.clone(),
                    element.stage.clone(),
                    element.status,
                )
            })
            .collect();
        let markers = self
            .markers
            .iter()
            .filter(|(_, marker)| marker.edge == index)
            .map(|(id, marker)| {
                (
                    id.clone(),
                    marker.label.clone(),
                    marker.stage.clone(),
                    marker.current,
                    marker.target,
                )
            })
            .collect();
        (statuses, markers)
    }

    pub(super) fn descriptor(&mut self, now: f64, graph: &RelationshipGraph) -> Value {
        self.advance(now);
        json!({"graph_id":self.graph_id,
            "markers":self.markers.iter().map(|(id, marker)| json!({
                "id":id,"edge":marker.edge,"percent":marker.current,"target_percent":marker.target,
                "color":marker.color,"reverse":marker.reverse,"smoothing_ms":marker.smoothing_ms,
                "label":marker.label,"stage":marker.stage
            })).collect::<Vec<_>>(),
            "elements":self.elements.iter().map(|(id, element)| json!({
                "id":id,"nodes":element.nodes.iter().map(|&index| &graph.nodes[index].id).collect::<Vec<_>>(),
                "edges":element.edges,"status":element.status,
                "color":element.color,"pulse":element.pulse,
                "label":element.label,"stage":element.stage
            })).collect::<Vec<_>>()
        })
    }

    pub(in crate::app) fn paint(
        &self,
        painter: &egui::Painter,
        now: f64,
        canvas: AnimationCanvas<'_>,
        locale: Locale,
    ) {
        let zoom = canvas.zoom;
        let mut status_badge_heights = vec![0.0; canvas.positions.len()];
        let mut status_badge_counts = vec![0usize; canvas.positions.len()];
        let mut totals = vec![0usize; canvas.positions.len()];
        for element in self
            .elements
            .values()
            .filter(|element| element.status.is_some())
        {
            for &node in &element.nodes {
                totals[node] += 1;
            }
        }
        for (id, element) in &self.elements {
            let color = Color32::from_rgb(element.color[0], element.color[1], element.color[2]);
            super::paint_highlight(
                painter,
                now,
                canvas,
                &element.nodes,
                &element.edges,
                color,
                element.pulse,
            );
            if let Some(status) = element.status {
                let text = if let Some(label) = &element.label {
                    format!(
                        "{} [{}] · {}\n{}",
                        short_text(label, 24),
                        short_text(id, 16),
                        locale.text(status.appearance().2),
                        short_text(element.stage.as_deref().unwrap_or(id), 32)
                    )
                } else {
                    locale.text(status.appearance().2).into()
                };
                for &node in &element.nodes {
                    if status_badge_counts[node] >= 3 {
                        continue;
                    }
                    status_badge_counts[node] += 1;
                    let mut top_right = canvas.transform(canvas.positions[node])
                        + canvas.sizes[node] * zoom * Vec2::new(0.5, -0.5);
                    let font = egui::FontId::proportional((11.0 * zoom).max(10.0));
                    let text = if status_badge_counts[node] == 3 && totals[node] > 3 {
                        format!(
                            "{text}\n+{} ({})",
                            totals[node] - 3,
                            locale.text(TextKey::GraphJobs)
                        )
                    } else {
                        text.clone()
                    };
                    let galley = painter.layout(text, font, Color32::WHITE, 200.0);
                    top_right.y -= status_badge_heights[node];
                    status_badge_heights[node] += galley.size().y + 8.0;
                    let rect = egui::Rect::from_min_size(
                        top_right - Vec2::new(galley.size().x, galley.size().y + 6.0 * zoom),
                        galley.size(),
                    )
                    .expand(2.0 * zoom);
                    painter.rect_filled(rect, 3, color.gamma_multiply(0.85));
                    painter.rect_stroke(
                        rect,
                        3,
                        egui::Stroke::new(2.0, status.color()),
                        egui::StrokeKind::Outside,
                    );
                    painter.galley(rect.min + Vec2::splat(2.0 * zoom), galley, Color32::WHITE);
                }
            }
        }
        let mut markers_per_edge = BTreeMap::<usize, usize>::new();
        for marker in self.markers.values() {
            *markers_per_edge.entry(marker.edge).or_default() += 1;
        }
        let mut marker_lanes = BTreeMap::<usize, usize>::new();
        for (id, marker) in &self.markers {
            let progress = marker.current as f32 / 100.0;
            let progress = if marker.reverse {
                1.0 - progress
            } else {
                progress
            };
            let lane = marker_lanes.entry(marker.edge).or_default();
            let lane_count = markers_per_edge[&marker.edge];
            let offset = (*lane as f32 - (lane_count - 1) as f32 * 0.5) * 16.0;
            *lane += 1;
            if let Some(point) =
                point_on_path_with_offset(&canvas.paths[marker.edge], progress, offset)
            {
                super::paint_marker(
                    painter,
                    canvas.transform(point),
                    zoom,
                    Color32::from_rgb(marker.color[0], marker.color[1], marker.color[2]),
                );
                if let Some(label) = &marker.label {
                    let text = format!(
                        "{} [{}] · {:.0}%",
                        short_text(label, 24),
                        short_text(id, 16),
                        marker.current
                    );
                    let font = egui::FontId::proportional((11.0 * zoom).max(10.0));
                    let color =
                        Color32::from_rgb(marker.color[0], marker.color[1], marker.color[2]);
                    let galley = painter.layout(text, font, color, 180.0);
                    let anchor = canvas.transform(point)
                        + Vec2::new(10.0 * zoom, (*lane - 1) as f32 * (galley.size().y + 4.0));
                    let rect = egui::Rect::from_min_size(anchor, galley.size()).expand(3.0);
                    painter.rect_filled(
                        rect,
                        3,
                        painter
                            .ctx()
                            .style_of(painter.ctx().theme())
                            .visuals
                            .panel_fill,
                    );
                    painter.rect_stroke(
                        rect,
                        3,
                        egui::Stroke::new(1.0, color),
                        egui::StrokeKind::Outside,
                    );
                    painter.galley(anchor, galley, color);
                }
            }

            fn point_on_path_with_offset(
                path: &[egui::Pos2],
                progress: f32,
                offset: f32,
            ) -> Option<egui::Pos2> {
                if offset == 0.0 {
                    return point_on_path(path, progress);
                }
                let mut distance = path
                    .windows(2)
                    .map(|pair| pair[0].distance(pair[1]))
                    .sum::<f32>()
                    * progress;
                for pair in path.windows(2) {
                    let delta = pair[1] - pair[0];
                    let length = delta.length();
                    if length > 0.0 && distance <= length {
                        let tangent = delta / length;
                        let normal = egui::Vec2::new(-tangent.y, tangent.x);
                        return Some(pair[0] + delta * (distance / length) + normal * offset);
                    }
                    distance -= length;
                }
                path.last().copied()
            }
        }
    }
}

pub(in crate::app) fn state(ctx: &egui::Context) -> Option<RealtimeState> {
    ctx.data(|data| data.get_temp(egui::Id::new(STATE_ID)))
}
pub(in crate::app) fn store(ctx: &egui::Context, state: RealtimeState) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(STATE_ID), state));
}
pub(in crate::app) fn clear(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<RealtimeState>(egui::Id::new(STATE_ID)));
}
