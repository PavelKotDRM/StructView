mod callouts;
mod leaders;
mod placement;

pub(super) use callouts::{
    graph_edge_label_callout, graph_edge_label_callout_text, place_edge_label,
};
pub(super) use leaders::resolve_graph_label_leaders;
#[cfg(test)]
pub(super) use placement::aligned_label_rect;
pub(super) use placement::{
    GraphEdgeLabelLayout, graph_edge_label_layout, layout_graph_edge_label, shorten_to_width,
};
