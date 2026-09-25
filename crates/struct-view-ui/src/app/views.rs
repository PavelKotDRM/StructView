use std::collections::HashSet;

use egui::{Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Vec2};
use serde_json::Value;

use struct_view_core::diff::format_value;
use struct_view_core::parser::{DataFormat, JsonValueType};
use struct_view_core::search::SearchState;

use super::i18n::{Locale, TextKey};
use super::state::ComparisonState;
use super::theme::{
    COLOR_ACTIVE_MATCH, COLOR_ERROR, COLOR_KEY, COLOR_MATCH, COLOR_SUCCESS, value_color,
};
use super::visualization::{
    RelationshipGraph, SchemaDiagram, SchemaSource, TableData, schema_visible_indices,
    table_to_csv, table_visible_indices,
};

const MIN_TABLE_WIDTH: f32 = 760.0;
const GRAPH_NODE_SIZE: Vec2 = Vec2::new(208.0, 70.0);
const GRAPH_STEP: Vec2 = Vec2::new(250.0, 116.0);

mod diff;
mod graph;
mod schema;
mod table;
#[cfg(test)]
mod tests;

#[cfg(test)]
use diff::{PairChange, document_label_for_path, pair_change};
pub(super) use diff::{
    comparison_column_width, comparison_value_color, show_diff, show_difference_legend,
};
pub(super) use graph::show_graph;
pub(super) use schema::show_schema;
use table::table_header;
pub(super) use table::{export_table_csv, show_table};
