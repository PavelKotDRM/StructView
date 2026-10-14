mod descriptor;
mod dispatch;
mod settings;
mod window;

#[cfg(test)]
pub(super) use descriptor::graph_descriptor;
#[cfg(test)]
pub(super) use dispatch::{GuiApiOperation, decode_api_operation};
pub(in crate::app) use settings::GraphAnimationState;
