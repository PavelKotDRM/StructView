//! HTTP transport for the graph animation API.

pub mod server;

pub use server::{
    AnimationApiServer, ApiOperation, ApiReply, ApiRequest, ApiRequestRecord, ApiServerStats,
    ServerOptions, TlsFiles,
};
