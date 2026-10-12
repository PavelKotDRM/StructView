//! Scheduled and realtime graph animation. The optional HTTP/HTTPS server never edits document data.

use super::i18n::TextKey;
use super::state::StructViewApp;
use super::visualization::{RelationshipGraph, VisualizationMode};
use egui::{Color32, Pos2, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(in crate::app) mod realtime;
mod server;
#[cfg(test)]
mod tests;

pub use realtime::{ElementStatus, RealtimeCommand, RealtimeUpdate};
use server::{AnimationApiServer, ApiOperation, ApiReply};

const PLAYER_ID: &str = "graph-animation-player";
const MAX_STEPS: usize = 256;
const MAX_DURATION_MS: u64 = 60_000;
const MAX_TIMELINE_MS: u64 = 600_000;

#[derive(Clone, Copy)]
pub(in crate::app) struct AnimationCanvas<'a> {
    pub positions: &'a [Pos2],
    pub sizes: &'a [Vec2],
    pub paths: &'a [Vec<Pos2>],
    pub zoom: f32,
    pub offset: Vec2,
}

impl AnimationCanvas<'_> {
    fn transform(&self, point: Pos2) -> Pos2 {
        Pos2::ZERO + point.to_vec2() * self.zoom + self.offset
    }
}

fn default_duration() -> u64 {
    1000
}
fn default_color() -> [u8; 3] {
    [255, 180, 40]
}

// Preserve serde_json's arbitrary-precision numbers inside the tagged command enum.
fn deserialize_zoom<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    let number = Option::<serde_json::Number>::deserialize(deserializer)?;
    number
        .map(|number| {
            number
                .as_f64()
                .filter(|value| value.is_finite() && (0.15..=3.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| serde::de::Error::custom("zoom must be a finite number in 0.15..=3"))
        })
        .transpose()
}

/// Visual-only commands. Edge numbers are zero-based indices from `GET /v1/graph`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnimationCommand {
    Highlight {
        #[serde(default)]
        nodes: Vec<String>,
        #[serde(default)]
        edges: Vec<usize>,
        #[serde(default = "default_color")]
        color: [u8; 3],
        #[serde(default = "default_duration")]
        duration_ms: u64,
        #[serde(default)]
        pulse: bool,
    },
    Flow {
        edge: usize,
        #[serde(default = "default_color")]
        color: [u8; 3],
        #[serde(default = "default_duration")]
        duration_ms: u64,
        #[serde(default)]
        reverse: bool,
    },
    FlowPercent {
        edge: usize,
        #[serde(default, deserialize_with = "realtime::deserialize_percent")]
        from_percent: f64,
        #[serde(deserialize_with = "realtime::deserialize_percent")]
        to_percent: f64,
        #[serde(default = "default_color")]
        color: [u8; 3],
        #[serde(default = "default_duration")]
        duration_ms: u64,
        #[serde(default)]
        reverse: bool,
    },
    Camera {
        #[serde(default)]
        node: Option<String>,
        #[serde(default, deserialize_with = "deserialize_zoom")]
        zoom: Option<f32>,
        #[serde(default)]
        fit: bool,
        #[serde(default = "default_duration")]
        duration_ms: u64,
    },
    Clear,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationStep {
    #[serde(default)]
    pub at_ms: u64,
    pub command: AnimationCommand,
}

/// A new scenario replaces the current one. Times are relative to acceptance.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationScenario {
    pub graph_id: String,
    pub steps: Vec<AnimationStep>,
}

#[derive(Clone)]
enum Effect {
    Highlight {
        nodes: Vec<usize>,
        edges: Vec<usize>,
        color: Color32,
        pulse: bool,
    },
    Flow {
        edge: usize,
        color: Color32,
        reverse: bool,
        from: f32,
        to: f32,
    },
    Camera {
        node: Option<usize>,
        zoom: Option<f32>,
        fit: bool,
    },
    Clear,
}

#[derive(Clone)]
struct Step {
    at: f64,
    duration: f64,
    effect: Effect,
}

#[derive(Clone)]
pub(in crate::app) struct AnimationPlayer {
    graph_id: String,
    started: f64,
    steps: Vec<Step>,
    end: f64,
    camera: Option<(usize, f32, Vec2)>,
    camera_finished: Option<usize>,
}

impl AnimationPlayer {
    fn new(
        scenario: AnimationScenario,
        graph: &RelationshipGraph,
        now: f64,
    ) -> Result<Self, String> {
        if scenario.steps.is_empty() || scenario.steps.len() > MAX_STEPS {
            return Err(format!("steps must contain 1..={MAX_STEPS} commands"));
        }
        let node_index = |id: &str| realtime::node_index(graph, id);
        let edge_index = |index: usize| realtime::edge_index(graph, index);
        let mut steps = Vec::with_capacity(scenario.steps.len());
        for step in scenario.steps {
            let (duration, effect) = match step.command {
                AnimationCommand::Highlight {
                    nodes,
                    edges,
                    color,
                    duration_ms,
                    pulse,
                } => {
                    if nodes.is_empty() && edges.is_empty() {
                        return Err("highlight needs nodes or edges".into());
                    }
                    if nodes.len() + edges.len() > 1024 {
                        return Err("highlight supports at most 1024 targets".into());
                    }
                    let nodes = nodes
                        .iter()
                        .map(|id| node_index(id))
                        .collect::<Result<_, _>>()?;
                    let edges = edges
                        .into_iter()
                        .map(edge_index)
                        .collect::<Result<_, _>>()?;
                    (
                        duration_ms,
                        Effect::Highlight {
                            nodes,
                            edges,
                            color: Color32::from_rgb(color[0], color[1], color[2]),
                            pulse,
                        },
                    )
                }
                AnimationCommand::Flow {
                    edge,
                    color,
                    duration_ms,
                    reverse,
                } => (
                    duration_ms,
                    Effect::Flow {
                        edge: edge_index(edge)?,
                        color: Color32::from_rgb(color[0], color[1], color[2]),
                        reverse,
                        from: 0.0,
                        to: 1.0,
                    },
                ),
                AnimationCommand::FlowPercent {
                    edge,
                    from_percent,
                    to_percent,
                    color,
                    duration_ms,
                    reverse,
                } => {
                    if !from_percent.is_finite()
                        || !to_percent.is_finite()
                        || !(0.0..=100.0).contains(&from_percent)
                        || !(from_percent..=100.0).contains(&to_percent)
                    {
                        return Err(
                            "flow_percent requires 0 <= from_percent <= to_percent <= 100".into(),
                        );
                    }
                    (
                        duration_ms,
                        Effect::Flow {
                            edge: edge_index(edge)?,
                            color: Color32::from_rgb(color[0], color[1], color[2]),
                            reverse,
                            from: from_percent as f32 / 100.0,
                            to: to_percent as f32 / 100.0,
                        },
                    )
                }
                AnimationCommand::Camera {
                    node,
                    zoom,
                    fit,
                    duration_ms,
                } => {
                    if fit && (node.is_some() || zoom.is_some()) {
                        return Err("camera fit cannot be combined with node or zoom".into());
                    }
                    if !fit && node.is_none() && zoom.is_none() {
                        return Err("camera needs node, zoom or fit".into());
                    }
                    if zoom
                        .is_some_and(|value| !value.is_finite() || !(0.15..=3.0).contains(&value))
                    {
                        return Err("camera zoom must be between 0.15 and 3".into());
                    }
                    (
                        duration_ms,
                        Effect::Camera {
                            node: node.as_deref().map(node_index).transpose()?,
                            zoom,
                            fit,
                        },
                    )
                }
                AnimationCommand::Clear => (0, Effect::Clear),
            };
            if (!matches!(effect, Effect::Clear) && (duration == 0 || duration > MAX_DURATION_MS))
                || step
                    .at_ms
                    .checked_add(duration)
                    .is_none_or(|end| end > MAX_TIMELINE_MS)
            {
                return Err(
                    "duration_ms must be 1..=60000; timeline must end within 600000 ms".into(),
                );
            }
            steps.push(Step {
                at: step.at_ms as f64 / 1000.0,
                duration: duration as f64 / 1000.0,
                effect,
            });
        }
        steps.sort_by(|a, b| a.at.total_cmp(&b.at));
        let end = steps
            .iter()
            .map(|step| step.at + step.duration)
            .fold(0.0, f64::max);
        Ok(Self {
            graph_id: scenario.graph_id,
            started: now,
            steps,
            end,
            camera: None,
            camera_finished: None,
        })
    }

    pub(in crate::app) fn matches_graph(&self, fingerprint: u64) -> bool {
        self.graph_id == format!("{fingerprint:016x}")
    }

    fn elapsed(&self, now: f64) -> f64 {
        (now - self.started).max(0.0)
    }

    pub(in crate::app) fn is_running(&self, now: f64) -> bool {
        let elapsed = self.elapsed(now);
        self.remaining_steps(now)
            .any(|(_, step)| elapsed < step.at + step.duration)
    }

    fn remaining_steps(&self, now: f64) -> impl DoubleEndedIterator<Item = (usize, &Step)> {
        let elapsed = self.elapsed(now);
        let cleared = self
            .steps
            .iter()
            .enumerate()
            .filter(|(_, step)| step.at <= elapsed && matches!(step.effect, Effect::Clear))
            .map(|(index, _)| index)
            .next_back();
        self.steps
            .iter()
            .enumerate()
            .filter(move |(index, _)| cleared.is_none_or(|clear| *index > clear))
    }

    fn visible_steps(&self, now: f64) -> impl DoubleEndedIterator<Item = (usize, &Step, f32)> {
        let elapsed = self.elapsed(now);
        self.remaining_steps(now)
            .filter(move |(_, step)| step.at <= elapsed)
            .map(move |(index, step)| {
                (
                    index,
                    step,
                    ((elapsed - step.at) / step.duration.max(0.001)).clamp(0.0, 1.0) as f32,
                )
            })
    }

    pub(in crate::app) fn camera(
        &mut self,
        now: f64,
        viewport: Vec2,
        content: Vec2,
        positions: &[Pos2],
        zoom: &mut f32,
        pan: &mut Vec2,
    ) {
        let next = self
            .visible_steps(now)
            .filter_map(|(index, step, progress)| {
                if let Effect::Camera { node, zoom, fit } = step.effect {
                    Some((index, node, zoom, fit, progress))
                } else {
                    None
                }
            })
            .next_back();
        let Some((index, node, target_zoom, fit, progress)) = next else {
            self.camera = None;
            self.camera_finished = None;
            return;
        };
        if self.camera_finished == Some(index) {
            return;
        }
        if self.camera.is_none_or(|(active, _, _)| active != index) {
            self.camera = Some((index, *zoom, *pan));
        }
        let (_, from_zoom, from_pan) = self.camera.expect("camera origin");
        let mut to_zoom = target_zoom.unwrap_or(from_zoom);
        let to_pan = if fit {
            to_zoom = ((viewport.x - 32.0) / content.x.max(1.0))
                .min((viewport.y - 32.0) / content.y.max(1.0))
                .clamp(0.15, 3.0);
            (viewport - content * to_zoom) * 0.5
        } else if let Some(node) = node {
            viewport * 0.5 - positions[node].to_vec2() * to_zoom
        } else {
            viewport * 0.5 - (viewport * 0.5 - from_pan) * (to_zoom / from_zoom)
        };
        let t = progress * progress * (3.0 - 2.0 * progress);
        *zoom = from_zoom + (to_zoom - from_zoom) * t;
        *pan = from_pan + (to_pan - from_pan) * t;
        if progress >= 1.0 {
            self.camera_finished = Some(index);
            self.camera = None;
        }
    }

    pub(in crate::app) fn paint(
        &self,
        painter: &egui::Painter,
        now: f64,
        canvas: AnimationCanvas<'_>,
    ) {
        for (_, step, progress) in self.visible_steps(now) {
            if self.elapsed(now) >= step.at + step.duration {
                continue;
            }
            match &step.effect {
                Effect::Highlight {
                    nodes,
                    edges,
                    color,
                    pulse,
                } => {
                    paint_highlight(
                        painter,
                        self.elapsed(now) - step.at,
                        canvas,
                        nodes,
                        edges,
                        *color,
                        *pulse,
                    );
                }
                Effect::Flow {
                    edge,
                    color,
                    reverse,
                    from,
                    to,
                } => {
                    let progress = from + (to - from) * progress;
                    let progress = if *reverse { 1.0 - progress } else { progress };
                    if let Some(point) = point_on_path(&canvas.paths[*edge], progress) {
                        paint_marker(painter, canvas.transform(point), canvas.zoom, *color);
                    }
                }
                _ => {}
            }
        }
    }
}

fn paint_marker(painter: &egui::Painter, point: Pos2, zoom: f32, color: Color32) {
    painter.circle_filled(point, 6.0 * zoom, color);
    painter.circle_stroke(point, 7.0 * zoom, egui::Stroke::new(zoom, Color32::WHITE));
}

fn paint_highlight(
    painter: &egui::Painter,
    time: f64,
    canvas: AnimationCanvas<'_>,
    nodes: &[usize],
    edges: &[usize],
    color: Color32,
    pulse: bool,
) {
    let alpha = if pulse {
        0.5 + 0.5 * (time as f32 * 8.0).sin().abs()
    } else {
        1.0
    };
    let stroke = egui::Stroke::new(4.0 * canvas.zoom, color.gamma_multiply(alpha));
    for &index in edges {
        painter.add(egui::Shape::line(
            canvas.paths[index]
                .iter()
                .map(|point| canvas.transform(*point))
                .collect(),
            stroke,
        ));
    }
    for &index in nodes {
        let rect = egui::Rect::from_center_size(
            canvas.transform(canvas.positions[index]),
            canvas.sizes[index] * canvas.zoom,
        )
        .expand(3.0 * canvas.zoom);
        painter.rect_stroke(rect, 7, stroke, egui::StrokeKind::Outside);
    }
}

fn point_on_path(path: &[Pos2], progress: f32) -> Option<Pos2> {
    let mut distance = path
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum::<f32>()
        * progress;
    for pair in path.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length > 0.0 && distance <= length {
            return Some(pair[0] + (pair[1] - pair[0]) * (distance / length));
        }
        distance -= length;
    }
    path.last().copied()
}

pub(in crate::app) fn player(ctx: &egui::Context) -> Option<AnimationPlayer> {
    ctx.data(|data| data.get_temp(egui::Id::new(PLAYER_ID)))
}

pub(in crate::app) fn store_player(ctx: &egui::Context, player: AnimationPlayer) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(PLAYER_ID), player));
}

pub(in crate::app) fn stop(ctx: &egui::Context) {
    stop_scenario(ctx);
    realtime::clear(ctx);
}

pub(in crate::app) fn stop_scenario(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<AnimationPlayer>(egui::Id::new(PLAYER_ID)));
}

pub(in crate::app) struct GraphAnimationState {
    server: Option<AnimationApiServer>,
    port: u16,
    bind: String,
    host: String,
    network: bool,
    https: bool,
    certificate: String,
    private_key: String,
    token_input: String,
}

impl Default for GraphAnimationState {
    fn default() -> Self {
        Self {
            server: None,
            port: 0,
            bind: "127.0.0.1".into(),
            host: String::new(),
            network: false,
            https: false,
            certificate: String::new(),
            private_key: String::new(),
            token_input: String::new(),
        }
    }
}

impl GraphAnimationState {
    fn server_options(&self) -> Result<server::ServerOptions, String> {
        let bind = self
            .bind
            .trim()
            .parse()
            .map_err(|error| format!("Invalid bind IP: {error}"))?;
        Ok(server::ServerOptions {
            bind,
            port: self.port,
            host: (!self.host.trim().is_empty()).then(|| self.host.trim().into()),
            allow_network: self.network,
            tls: self.https.then(|| server::TlsFiles {
                certificate: self.certificate.trim().into(),
                private_key: self.private_key.trim().into(),
            }),
        })
    }
}

impl StructViewApp {
    pub(in crate::app) fn show_graph_animation_settings(&mut self, ui: &mut egui::Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::GraphAnimationApi), |ui| {
            ui.weak(locale.text(TextKey::GraphApiHint));
            let mut token_result = None;
            if let Some(server) = &mut self.graph_animation.server {
                ui.monospace(server.url());
                ui.label(format!(
                    "{}: {}",
                    locale.text(TextKey::GraphApiBind),
                    server.address()
                ));
                ui.label("Bearer token");
                let mut token = server.token();
                ui.add(egui::TextEdit::singleline(&mut token).desired_width(420.0));
                ui.horizontal(|ui| {
                    if ui.button(locale.text(TextKey::GraphApiCopyToken)).clicked() {
                        token_result = Some(crate::clipboard::copy_to_clipboard(server.token()));
                    }
                    if ui
                        .button(locale.text(TextKey::GraphApiRegenerateToken))
                        .clicked()
                    {
                        token_result = Some(server.regenerate_token());
                    }
                });
                ui.label(locale.text(TextKey::GraphApiOwnToken));
                ui.add(
                    egui::TextEdit::singleline(&mut self.graph_animation.token_input)
                        .password(true)
                        .desired_width(420.0),
                );
                ui.horizontal(|ui| {
                    if ui
                        .button(locale.text(TextKey::GraphApiPasteToken))
                        .clicked()
                    {
                        match crate::clipboard::read_from_clipboard() {
                            Ok(token) => self.graph_animation.token_input = token,
                            Err(error) => token_result = Some(Err(error)),
                        }
                    }
                    if ui
                        .button(locale.text(TextKey::GraphApiApplyToken))
                        .clicked()
                    {
                        let result = server.set_token(self.graph_animation.token_input.clone());
                        if result.is_ok() {
                            self.graph_animation.token_input.clear();
                        }
                        token_result = Some(result);
                    }
                });
                ui.weak(locale.text(TextKey::GraphApiTokenHint));
                if ui.button(locale.text(TextKey::GraphApiStop)).clicked() {
                    self.graph_animation.server = None;
                    self.graph_animation.token_input.clear();
                    stop(ui.ctx());
                }
                if let Some(result) = token_result {
                    match result {
                        Ok(()) => self.show_toast(locale.text(TextKey::GraphApiTokenDone)),
                        Err(error) => self.show_error(&error),
                    }
                }
            } else {
                ui.label(locale.text(TextKey::GraphApiBind));
                ui.text_edit_singleline(&mut self.graph_animation.bind);
                ui.label(locale.text(TextKey::GraphApiHost));
                ui.text_edit_singleline(&mut self.graph_animation.host);
                ui.horizontal(|ui| {
                    ui.label(locale.text(TextKey::GraphApiPort));
                    ui.add(egui::DragValue::new(&mut self.graph_animation.port).range(0..=65535));
                });
                ui.checkbox(
                    &mut self.graph_animation.network,
                    locale.text(TextKey::GraphApiNetwork),
                );
                if self.graph_animation.network {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        locale.text(TextKey::GraphApiNetworkWarning),
                    );
                }
                ui.checkbox(&mut self.graph_animation.https, "HTTPS (TLS)");
                if self.graph_animation.https {
                    for (path, key) in [
                        (
                            &mut self.graph_animation.certificate,
                            TextKey::GraphApiCertificate,
                        ),
                        (
                            &mut self.graph_animation.private_key,
                            TextKey::GraphApiPrivateKey,
                        ),
                    ] {
                        ui.label(locale.text(key));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(path);
                            if ui.button("...").clicked()
                                && let Some(selected) = rfd::FileDialog::new()
                                    .add_filter("PEM", &["pem", "crt", "key"])
                                    .pick_file()
                            {
                                *path = selected.to_string_lossy().into_owned();
                            }
                        });
                    }
                }
                if ui.button(locale.text(TextKey::GraphApiStart)).clicked() {
                    match self.graph_animation.server_options().and_then(|options| {
                        AnimationApiServer::start_with(options, ui.ctx().clone())
                    }) {
                        Ok(server) => self.graph_animation.server = Some(server),
                        Err(error) => self.show_error(&error),
                    }
                }
            }
            if ui
                .button(locale.text(TextKey::GraphAnimationStop))
                .clicked()
            {
                stop(ui.ctx());
            }
        });
    }

    pub(in crate::app) fn poll_graph_animation_api(&mut self, ctx: &egui::Context) {
        let graph_id = self.graph_calculation.animation_graph_id();
        if self.visualization != VisualizationMode::Graph
            || self.comparison.is_some()
            || player(ctx).is_some_and(|player| Some(&player.graph_id) != graph_id.as_ref())
            || realtime::state(ctx)
                .is_some_and(|state| Some(state.graph_id()) != graph_id.as_deref())
        {
            stop(ctx);
        }
        // Bound the amount of protocol work performed in one GUI frame.
        for _ in 0..8 {
            let request = match self
                .graph_animation
                .server
                .as_ref()
                .map(AnimationApiServer::try_request)
            {
                Some(Ok(request)) => request,
                Some(Err(error)) => {
                    self.show_error(&error);
                    self.graph_animation.server = None;
                    stop(ctx);
                    break;
                }
                None => None,
            };
            let Some(request) = request else { break };
            if std::time::Instant::now() >= request.deadline {
                let _ = request.reply.send(ApiReply {
                    status: 504,
                    value: json!({"error":"Request expired before GUI execution"}),
                });
                continue;
            }
            let result = if matches!(&request.operation, ApiOperation::Stop) {
                stop(ctx);
                Ok(json!({"stopped":true}))
            } else if self.visualization != VisualizationMode::Graph || self.comparison.is_some() {
                Err((409, "Relationship graph view must be active".to_string()))
            } else if let Some(calculation) = self.graph_calculation.result() {
                let graph_id = graph_id.as_ref().expect("ready graph ID");
                match request.operation {
                    ApiOperation::Graph => Ok(graph_descriptor(graph_id, &calculation.graph)),
                    ApiOperation::Status => {
                        let now = ctx.input(|input| input.time);
                        Ok(json!({"graph_id":graph_id,
                            "running":player(ctx).is_some_and(|player| player.is_running(now))}))
                    }
                    ApiOperation::Stop => {
                        stop(ctx);
                        Ok(json!({"stopped":true}))
                    }
                    ApiOperation::RealtimeStatus => {
                        let mut state = realtime::state(ctx);
                        let value = if let Some(state) = &mut state {
                            state.descriptor(ctx.input(|input| input.time), &calculation.graph)
                        } else {
                            json!({"graph_id":graph_id,"markers":[],"elements":[]})
                        };
                        if let Some(state) = state {
                            realtime::store(ctx, state);
                        }
                        Ok(value)
                    }
                    ApiOperation::Realtime(update) => {
                        if &update.graph_id != graph_id {
                            Err((409, "graph_id is stale; read /v1/graph again".into()))
                        } else {
                            realtime::RealtimeState::apply(
                                realtime::state(ctx),
                                update,
                                &calculation.graph,
                                ctx.input(|input| input.time),
                            )
                            .map(|state| {
                                realtime::store(ctx, state);
                                ctx.request_repaint();
                                json!({"accepted":true})
                            })
                            .map_err(|error| (400, error))
                        }
                    }
                    ApiOperation::Play(scenario) => {
                        if &scenario.graph_id != graph_id {
                            Err((409, "graph_id is stale; read /v1/graph again".into()))
                        } else {
                            AnimationPlayer::new(
                                scenario,
                                &calculation.graph,
                                ctx.input(|input| input.time),
                            )
                            .map(|player| {
                                let duration_ms = (player.end * 1000.0).round() as u64;
                                store_player(ctx, player);
                                ctx.request_repaint();
                                json!({"accepted":true,"duration_ms":duration_ms})
                            })
                            .map_err(|error| (400, error))
                        }
                    }
                }
            } else {
                Err((409, "Graph layout is not ready".into()))
            };
            let reply = match result {
                Ok(value) => ApiReply { status: 200, value },
                Err((status, error)) => ApiReply {
                    status,
                    value: json!({"error":error}),
                },
            };
            let _ = request.reply.send(reply);
        }
    }
}

fn graph_descriptor(graph_id: &str, graph: &RelationshipGraph) -> Value {
    json!({
        "graph_id":graph_id,
        "nodes":graph.nodes.iter().map(|node| json!({"id":node.id,"label":node.label})).collect::<Vec<_>>(),
        "edges":graph.edges.iter().enumerate().map(|(index, edge)| json!({
            "index":index, "source":graph.nodes[edge.source].id,
            "target":graph.nodes[edge.target].id, "label":edge.label,
            "direction":edge.direction.as_str()
        })).collect::<Vec<_>>()
    })
}
