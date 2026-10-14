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
    realtime: Option<&crate::app::graph_animation::realtime::RealtimeState>,
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
    show_graph_realtime_status(ui, realtime, None, Some(edge_index), locale);
}

pub(super) fn show_graph_realtime_status(
    ui: &mut egui::Ui,
    realtime: Option<&crate::app::graph_animation::realtime::RealtimeState>,
    node: Option<usize>,
    edge: Option<usize>,
    locale: Locale,
) {
    let (statuses, mut markers) = if let Some(node) = node {
        (
            realtime.map_or_else(Vec::new, |state| state.statuses_for_node(node)),
            Vec::new(),
        )
    } else if let Some(edge) = edge {
        realtime.map_or_else(
            || (Vec::new(), Vec::new()),
            |state| state.statuses_for_edge(edge),
        )
    } else {
        return;
    };

    ui.separator();
    ui.strong(locale.text(TextKey::GraphStatusTitle));
    if statuses.is_empty() && markers.is_empty() {
        ui.label(locale.text(TextKey::GraphStatusNoAssignment));
    } else {
        for (id, label, stage, status) in statuses {
            ui.horizontal_wrapped(|ui| {
                if let Some(label) = label {
                    ui.strong(label);
                }
                ui.monospace(&id);
                ui.label(status.map_or_else(
                    || locale.text(TextKey::GraphStatusNoAssignment),
                    |status| locale.text(status.text_key()),
                ));
            });
            if let Some(stage) = stage {
                ui.label(format!(
                    "{}: {stage}",
                    locale.text(TextKey::GraphStatusStage)
                ));
            }
            if let Some(index) = markers.iter().position(|marker| marker.0 == id) {
                let (_, _, _, current, target) = markers.remove(index);
                show_progress(ui, current, target, locale);
            }
        }
    }
    for (id, label, stage, current, target) in markers {
        ui.horizontal_wrapped(|ui| {
            if let Some(label) = label {
                ui.strong(label);
            }
            ui.monospace(id);
            show_progress(ui, current, target, locale);
        });
        if let Some(stage) = stage {
            ui.label(format!(
                "{}: {stage}",
                locale.text(TextKey::GraphStatusStage)
            ));
        }
    }
}

fn show_progress(ui: &mut egui::Ui, current: f64, target: f64, locale: Locale) {
    ui.label(format!(
        "{}: {:.0}%",
        locale.text(TextKey::GraphStatusProgress),
        current
    ));
    if (target - current).abs() >= 0.05 {
        ui.weak(format!(
            "{}: {:.0}%",
            locale.text(TextKey::GraphStatusTargetProgress),
            target
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::graph_animation::{
        ElementStatus, RealtimeCommand, RealtimeUpdate, realtime::RealtimeState,
    };

    #[test]
    fn graph_tooltips_report_status_assignment_and_marker_progress() {
        let root =
            struct_view_core::parser::parse_json(r#"[{"id":"a","depends_on":"b"},{"id":"b"}]"#)
                .unwrap();
        let graph = crate::app::visualization::build_relationship_graph(&root);
        let node_a = graph.nodes.iter().position(|node| node.id == "a").unwrap();
        let node_b = graph.nodes.iter().position(|node| node.id == "b").unwrap();
        let state = RealtimeState::apply(
            None,
            RealtimeUpdate {
                graph_id: "graph".into(),
                commands: vec![
                    RealtimeCommand::Element {
                        id: "job".into(),
                        nodes: vec!["a".into()],
                        edges: vec![],
                        status: Some(ElementStatus::Running),
                        color: None,
                        pulse: None,
                        label: Some("Order processing".into()),
                        stage: Some("Validating payment".into()),
                    },
                    RealtimeCommand::Marker {
                        id: "worker".into(),
                        edge: 0,
                        percent: 80.0,
                        smoothing_ms: 300,
                        color: [40, 160, 255],
                        reverse: false,
                        label: Some("Payment risk check".into()),
                        stage: Some("Reviewing transaction".into()),
                    },
                ],
            },
            &graph,
            0.0,
        )
        .unwrap();
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_graph_realtime_status(ui, Some(&state), Some(node_a), None, Locale::English);
            show_graph_realtime_status(ui, Some(&state), None, Some(0), Locale::English);
            show_graph_realtime_status(ui, Some(&state), Some(node_b), None, Locale::English);
        });
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Animation status"));
        assert!(text.contains("Running"));
        assert!(text.contains("Order processing"));
        assert!(text.contains("Validating payment"));
        assert!(text.contains("Payment risk check"));
        assert!(text.contains("Reviewing transaction"));
        assert!(text.contains("Progress: 0%"));
        assert!(text.contains("Target: 80%"));
        assert!(text.contains("No status assigned"));
        output.drop_without_applying_deltas();
    }
}
