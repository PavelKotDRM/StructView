//! Scheduled and realtime graph animation. The optional HTTP/HTTPS server never edits document data.

mod api;
pub(in crate::app) mod jobs;
mod player;
pub(in crate::app) mod realtime;
#[cfg(test)]
mod tests;

pub(in crate::app) use api::GraphAnimationState;
#[cfg(test)]
use api::{GuiApiOperation, decode_api_operation, graph_descriptor};
pub use jobs::{JobCommand, JobUpdate};
pub(in crate::app) use player::{
    AnimationCanvas, AnimationPlayer, player, stop, stop_scenario, store_player,
};
pub use player::{AnimationCommand, AnimationScenario, AnimationStep};
use player::{MAX_STEPS, default_color, paint_highlight, paint_marker, point_on_path};
pub use realtime::{ElementStatus, RealtimeCommand, RealtimeUpdate};
pub use struct_view_animation_api::server;
pub use struct_view_animation_api::{AnimationApiServer, ApiOperation, ApiReply};
