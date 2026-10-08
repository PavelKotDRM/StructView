use super::*;

use struct_view_core::graph::EdgeDirection;

mod calculation;
mod canvas;
mod cards;
mod export;
mod labels;
mod layout;
mod routing;

pub(in crate::app) use calculation::{GraphCalculationState, GraphRoutingLayout};
pub(in crate::app) use calculation::{headless_graph_image, headless_graph_image_with_progress};
pub(in crate::app) use canvas::graph_view_menu;
pub(in crate::app) use canvas::show_graph;
#[cfg(test)]
use canvas::{GraphInteractionState, graph_edges_at_pointer};
use cards::{
    GRAPH_CARD_COUNTS_OFFSET, GRAPH_CARD_ID_OFFSET, GRAPH_CARD_LABEL_OFFSET,
    GRAPH_CARD_PATH_OFFSET, graph_link_counts, graph_node_sizes,
};
pub(in crate::app) use export::render_graph_image;
pub(in crate::app) use export::{GraphExportFormat, GraphExportStyle, export_graph_image};
use labels::{
    GraphEdgeLabelLayout, graph_edge_label_callout_at, graph_edge_label_callout_text,
    graph_edge_label_layout, layout_graph_edge_label, place_edge_label,
    resolve_graph_label_leaders, shorten_to_width,
};
#[cfg(test)]
use labels::{aligned_label_rect, graph_edge_label_callout};
#[cfg(test)]
pub(super) use layout::build_graph_routing_layout;
use layout::build_graph_routing_layout_with_progress;
#[cfg(test)]
use layout::graph_node_positions;
use layout::partition_column_center;
#[cfg(test)]
pub(super) use routing::graph_edge_ports;
use routing::graph_edge_ports_sized;
#[cfg(not(test))]
use routing::segments_within_clearance;
#[cfg(test)]
pub(super) use routing::{
    GRAPH_EDGE_CLEARANCE, GRAPH_ROUTE_CLEARANCE, box_border_offset, segment_crosses_rect_interior,
    segments_within_clearance,
};
use routing::{
    GRAPH_EDGE_OUTLINE_WIDTH, arrow_head_wings, closest_point_on_segment, draw_arrow_head,
    edge_arrowheads, point_to_segment_distance, relationship_graph_fingerprint,
    route_graph_edges_with_progress, segment_intersects_rect, segments_intersect,
};
pub(super) use routing::{GraphRoutingGrid, graph_edge_color};
pub(in crate::app) use routing::{
    GraphRoutingWorkerSetting, RoutingSearchBackend, available_graph_routing_workers,
    graph_routing_worker_count,
};

pub(super) const GRAPH_DIM_FACTOR: f32 = 0.18;

const GRAPH_EDGE_LABEL_CHAR_WIDTH: f32 = 8.0;
const GRAPH_EDGE_LABEL_HEIGHT: f32 = 16.0;
