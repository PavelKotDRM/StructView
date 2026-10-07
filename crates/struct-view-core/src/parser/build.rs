//! Format detection, parsing, and serialization facade.

mod comments;
mod format;
mod json5;
mod parse;
mod paths;
mod serialize;
mod special_graphs;
#[cfg(test)]
mod tests;
mod tree;

pub(crate) use comments::extract_comments;
pub use comments::{comment_input, format_comment_for_format};
pub use format::DataFormat;
pub use parse::{parse_data, parse_json};
pub use paths::{build_path, plural_ru};
pub use serialize::{node_to_value, serialize_data, serialize_node};

#[cfg(test)]
use super::node::JsonValueType;
#[cfg(test)]
use comments::{collect_comments, format_comment};
