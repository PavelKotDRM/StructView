use egui::{Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Vec2};
use serde_json::Value;

use struct_view_core::diff::format_value;
use struct_view_core::parser::{DataFormat, JsonValueType};
use struct_view_core::search::SearchState;

use super::i18n::{Locale, TextKey};
use super::state::ComparisonState;
use super::theme::SyntaxColors;
#[cfg(test)]
use super::visualization::build_relationship_graph;
use super::visualization::{
    RelationshipGraph, SchemaDiagram, SchemaSource, TableData, schema_visible_indices,
    table_to_csv, table_visible_indices, try_build_relationship_graph,
};
use super::widgets::{Column, column_label, show_virtualized_columns, single_line_text};

const MIN_TABLE_WIDTH: f32 = 760.0;
const GRAPH_NODE_SIZE: Vec2 = Vec2::new(208.0, 70.0);
const GRAPH_STEP: Vec2 = Vec2::new(340.0, 150.0);

pub(super) mod diagram;
mod diff;
mod graph;
mod schema;
pub(super) mod structure;
mod table;
#[cfg(test)]
mod tests;

#[cfg(test)]
use diff::{PairChange, document_label_for_path, pair_change};
pub(super) use diff::{
    comparison_column_width, comparison_value_color, show_diff, show_difference_legend,
};
#[cfg(test)]
pub(super) use diff::{display_diff_value, human_diff_path};
pub(super) use graph::headless_graph_image as graph_image_for_headless;
pub(super) use graph::headless_graph_image_with_progress as graph_image_with_progress_for_headless;
pub(super) use graph::{GraphCalculationState, export_graph_image, show_graph};
pub(super) use graph::{GraphExportFormat, GraphExportStyle, graph_view_menu};
pub(in crate::app) use graph::{
    GraphRoutingWorkerSetting, available_graph_routing_workers, graph_routing_worker_count,
};
pub(super) use schema::show_schema;
pub(super) use table::{export_table_csv, show_table};
