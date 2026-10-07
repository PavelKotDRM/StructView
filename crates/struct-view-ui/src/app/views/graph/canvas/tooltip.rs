use super::super::*;

pub(in crate::app::views::graph) fn graph_edges_at_pointer(
    routes: &[Vec<Pos2>],
    pointer: Pos2,
    tolerance: f32,
) -> Vec<usize> {
    let distances = routes
        .iter()
        .map(|route| {
            route
                .windows(2)
                .map(|segment| point_to_segment_distance(pointer, segment[0], segment[1]))
                .fold(f32::INFINITY, f32::min)
        })
        .collect::<Vec<_>>();
    let nearest = distances.iter().copied().fold(f32::INFINITY, f32::min);
    distances
        .iter()
        .enumerate()
        .filter_map(|(index, &distance)| {
            (distance <= tolerance && distance <= nearest + tolerance / 6.0).then_some(index)
        })
        .collect()
}

pub(super) fn show_graph_edge_information(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    edge_index: usize,
    locale: Locale,
) {
    ui.set_max_width(480.0);
    let edge = &graph.edges[edge_index];
    ui.label(locale.text(match edge.direction {
        EdgeDirection::Directed => TextKey::GraphDirectedLink,
        EdgeDirection::Undirected => TextKey::GraphUndirectedLink,
        EdgeDirection::Bidirectional => TextKey::GraphBidirectionalLink,
        EdgeDirection::Reverse => TextKey::GraphReverseLink,
    }));
    if !edge.label.is_empty() {
        ui.label(&edge.label);
    }
    for (key, index) in [
        (TextKey::GraphLinkSource, edge.source),
        (TextKey::GraphLinkTarget, edge.target),
    ] {
        let node = &graph.nodes[index];
        ui.label(format!(
            "{}: {} ({})",
            locale.text(key),
            node.label,
            node.id
        ));
        ui.weak(&node.path);
    }
    show_graph_attributes(ui, ("graph-edge-attributes", edge_index), &edge.attributes);
}

pub(super) fn show_graph_attributes(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    attributes: &[(String, String)],
) {
    if attributes.is_empty() {
        return;
    }

    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(320.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (key, value) in attributes {
                ui.add(
                    egui::Label::new(format!("{key}: {value}"))
                        .wrap()
                        .selectable(true),
                );
            }
        });
}

pub(super) fn show_graph_tooltip(
    response: &egui::Response,
    keep_open: bool,
    contents: impl FnOnce(&mut egui::Ui),
) {
    let id = egui::Tooltip::next_tooltip_id(&response.ctx, response.id);
    let hovered = response.is_tooltip_open()
        && response
            .ctx
            .input(|input| input.pointer.hover_pos())
            .is_some_and(|pointer| {
                response.ctx.layer_id_at(pointer)
                    == Some(egui::LayerId::new(egui::Order::Tooltip, id))
            });
    // Normal egui tooltips close on scroll, even when the tooltip contains a scroll area.
    let tooltip = if keep_open || hovered {
        egui::Tooltip::for_widget(response)
    } else {
        egui::Tooltip::for_enabled(response)
    };
    tooltip.show(contents);
}
