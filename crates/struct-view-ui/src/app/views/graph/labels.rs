mod callouts;
mod leaders;
mod placement;

#[cfg(test)]
pub(super) use callouts::graph_edge_label_callout;
pub(super) use callouts::{
    EdgeLabelPlacementContext, graph_edge_label_callout_at, graph_edge_label_callout_text,
    place_edge_label,
};
pub(super) use leaders::resolve_graph_label_leaders;
#[cfg(test)]
pub(super) use placement::aligned_label_rect;
pub(super) use placement::{
    GraphEdgeLabelLayout, graph_edge_label_layout, graph_reference_radius, layout_graph_edge_label,
    shorten_to_width,
};
