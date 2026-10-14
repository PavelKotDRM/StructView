use super::super::{
    AnimationApiServer, AnimationPlayer, AnimationScenario, ApiOperation, ApiReply, RealtimeUpdate,
    player, realtime, stop, store_player,
};
use super::super::{JobUpdate, jobs};
use super::descriptor::graph_descriptor;
use crate::app::StructViewApp;
use crate::app::visualization::VisualizationMode;
use serde_json::json;

pub enum GuiApiOperation {
    Capabilities,
    Graph,
    Status,
    Play(AnimationScenario),
    Realtime(RealtimeUpdate),
    RealtimeStatus,
    Jobs(JobUpdate),
    JobsStatus,
    Stop,
}

pub fn decode_api_operation(operation: ApiOperation) -> Result<GuiApiOperation, String> {
    match operation {
        ApiOperation::Capabilities => Ok(GuiApiOperation::Capabilities),
        ApiOperation::Graph => Ok(GuiApiOperation::Graph),
        ApiOperation::Status => Ok(GuiApiOperation::Status),
        ApiOperation::Play(value) => serde_json::from_value(value)
            .map(GuiApiOperation::Play)
            .map_err(|error| format!("Invalid scenario JSON: {error}")),
        ApiOperation::Realtime(value) => serde_json::from_value(value)
            .map(GuiApiOperation::Realtime)
            .map_err(|error| format!("Invalid realtime JSON: {error}")),
        ApiOperation::RealtimeStatus => Ok(GuiApiOperation::RealtimeStatus),
        ApiOperation::Jobs(value) => serde_json::from_value(value)
            .map(GuiApiOperation::Jobs)
            .map_err(|error| format!("Invalid jobs JSON: {error}")),
        ApiOperation::JobsStatus => Ok(GuiApiOperation::JobsStatus),
        ApiOperation::Stop => Ok(GuiApiOperation::Stop),
    }
}

impl StructViewApp {
    pub(in crate::app) fn poll_graph_animation_api(&mut self, ctx: &egui::Context) {
        let graph_id = self.graph_calculation.animation_graph_id();
        if self.visualization != VisualizationMode::Graph
            || self.comparison.is_some()
            || player(ctx).is_some_and(|player| Some(&player.graph_id) != graph_id.as_ref())
            || realtime::state(ctx)
                .is_some_and(|state| Some(state.graph_id()) != graph_id.as_deref())
            || jobs::state(ctx).is_some_and(|state| Some(&state.graph_id) != graph_id.as_ref())
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
            let operation = match decode_api_operation(request.operation) {
                Ok(operation) => operation,
                Err(error) => {
                    let _ = request.reply.send(ApiReply {
                        status: 400,
                        value: json!({"error":error}),
                    });
                    continue;
                }
            };
            let result = if matches!(&operation, GuiApiOperation::Capabilities) {
                Ok(json!({
                    "version":1,
                    "features":["scheduled","realtime","jobs","atomic_job_updates","stage_progress"],
                    "limits":{"body_bytes":65536,"steps":256,"realtime_items":256,
                        "jobs":jobs::MAX_JOBS,"job_commands":jobs::MAX_JOBS,
                        "id_bytes":128,"label_bytes":256,"stage_bytes":256},
                    "job_commands":["upsert","remove","clear"],
                    "statuses":["waiting","running","success","error"]
                }))
            } else if matches!(&operation, GuiApiOperation::Stop) {
                stop(ctx);
                Ok(json!({"stopped":true}))
            } else if self.visualization != VisualizationMode::Graph || self.comparison.is_some() {
                Err((409, "Relationship graph view must be active".to_string()))
            } else if let Some(calculation) = self.graph_calculation.result() {
                let graph_id = graph_id.as_ref().expect("ready graph ID");
                match operation {
                    GuiApiOperation::Capabilities => unreachable!("handled without active graph"),
                    GuiApiOperation::Graph => Ok(graph_descriptor(graph_id, &calculation.graph)),
                    GuiApiOperation::Status => {
                        let now = ctx.input(|input| input.time);
                        Ok(json!({"graph_id":graph_id,
                            "running":player(ctx).is_some_and(|player| player.is_running(now))}))
                    }
                    GuiApiOperation::Stop => {
                        stop(ctx);
                        Ok(json!({"stopped":true}))
                    }
                    GuiApiOperation::JobsStatus => {
                        let mut state = jobs::state(ctx);
                        let value = state.as_mut().map_or_else(
                            || json!({"graph_id":graph_id,"jobs":[]}),
                            |state| state.descriptor(ctx.input(|input| input.time)),
                        );
                        if let Some(state) = state {
                            jobs::store(ctx, state);
                        }
                        Ok(value)
                    }
                    GuiApiOperation::Jobs(update) => {
                        if &update.graph_id != graph_id {
                            Err((409, "graph_id is stale; read /v1/graph again".into()))
                        } else {
                            jobs::JobsState::apply(
                                jobs::state(ctx),
                                update,
                                &calculation.graph,
                                ctx.input(|input| input.time),
                            )
                            .map(|state| {
                                let count = state.jobs.len();
                                jobs::store(ctx, state);
                                ctx.request_repaint();
                                json!({"accepted":true,"job_count":count})
                            })
                            .map_err(|error| (400, error))
                        }
                    }
                    GuiApiOperation::RealtimeStatus => {
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
                    GuiApiOperation::Realtime(update) => {
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
                    GuiApiOperation::Play(scenario) => {
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
