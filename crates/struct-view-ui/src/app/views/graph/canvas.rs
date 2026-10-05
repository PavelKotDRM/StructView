use super::*;

use std::collections::HashSet;

#[derive(Clone)]
pub(super) struct GraphInteractionState {
    pub(super) zoom: f32,
    pub(super) selected: HashSet<usize>,
    marquee_start: Option<Pos2>,
    marquee_base: HashSet<usize>,
    edge_hover: Option<(Pos2, Vec<usize>)>,
}

impl Default for GraphInteractionState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            selected: HashSet::new(),
            marquee_start: None,
            marquee_base: HashSet::new(),
            edge_hover: None,
        }
    }
}

impl GraphInteractionState {
    fn select_node(&mut self, index: usize, additive: bool) {
        if additive {
            if !self.selected.insert(index) {
                self.selected.remove(&index);
            }
        } else if self.selected.len() == 1 && self.selected.contains(&index) {
            self.selected.clear();
        } else {
            self.selected.clear();
            self.selected.insert(index);
        }
    }
}

/// Отрисовать граф идентификаторов и зависимостей.
pub(in crate::app) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    search: &SearchState,
    locale: Locale,
) -> Option<(GraphExportFormat, GraphExportStyle)> {
    let colors = SyntaxColors::new(ui.visuals());
    let mut export_requested = None;
    let interaction_id = ui.make_persistent_id(("graph-interaction", routing.graph_fingerprint));
    let mut interaction = ui
        .ctx()
        .data(|data| data.get_temp::<GraphInteractionState>(interaction_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphNodes),
            graph.nodes.len()
        ));
        ui.separator();
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphEdges),
            graph.edges.len()
        ));
    });

    if graph.nodes.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(locale.text(TextKey::GraphNoEntities));
        });
        return None;
    }
    if graph.edges.is_empty() {
        ui.label(RichText::new(locale.text(TextKey::GraphNoRelationships)).weak());
    }
    let mut fit_requested = false;
    ui.horizontal_wrapped(|ui| {
        ui.menu_button(locale.text(TextKey::GraphExport), |ui| {
            for format in [GraphExportFormat::Svg, GraphExportFormat::Png] {
                ui.menu_button(format.label(), |ui| {
                    for style in [
                        GraphExportStyle::LightTransparent,
                        GraphExportStyle::DarkOpaque,
                    ] {
                        if ui.button(locale.text(style.text_key())).clicked() {
                            export_requested = Some((format, style));
                            ui.close();
                        }
                    }
                });
            }
        });
        if ui
            .button("-")
            .on_hover_text(locale.text(TextKey::GraphZoomOut))
            .clicked()
        {
            interaction.zoom = (interaction.zoom / 1.2).clamp(0.1, 3.0);
        }
        ui.label(format!("{:.0}%", interaction.zoom * 100.0));
        if ui
            .button("+")
            .on_hover_text(locale.text(TextKey::GraphZoomIn))
            .clicked()
        {
            interaction.zoom = (interaction.zoom * 1.2).clamp(0.1, 3.0);
        }
        if ui.button("100%").clicked() {
            interaction.zoom = 1.0;
        }
        if ui.button(locale.text(TextKey::GraphFit)).clicked() {
            fit_requested = true;
        }
        ui.separator();
        if ui.button(locale.text(TextKey::GraphSelectAll)).clicked() {
            interaction.selected = (0..graph.nodes.len()).collect();
        }
        if ui
            .button(locale.text(TextKey::GraphClearSelection))
            .clicked()
        {
            interaction.selected.clear();
        }
        ui.label(format!(
            "{}: {}",
            locale.text(TextKey::GraphSelected),
            interaction.selected.len()
        ));
    });
    if fit_requested {
        let viewport = ui.available_size();
        interaction.zoom = ((viewport.x - 16.0).max(1.0) / routing.content_size.x)
            .min((viewport.y - 16.0).max(1.0) / routing.content_size.y)
            .min(3.0);
    }
    let zoom = interaction.zoom;
    let matching_paths = search
        .matches
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let active_path = search.current_match_path();

    let mut scroll_area = egui::ScrollArea::both().scroll_source(egui::scroll_area::ScrollSource {
        drag: egui::scroll_area::DragScroll::Never,
        ..Default::default()
    });
    if fit_requested {
        scroll_area = scroll_area.scroll_offset(Vec2::ZERO);
    }
    scroll_area.auto_shrink([false; 2]).show(ui, |ui| {
        let viewport = ui.available_size_before_wrap();
        let canvas_size = Vec2::new(
            viewport.x.max(routing.content_size.x * zoom),
            viewport.y.max(routing.content_size.y * zoom),
        );
        let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
        let canvas = response.rect;
        let canvas_offset = canvas.min.to_vec2();
        let transform = |point: Pos2| Pos2::ZERO + point.to_vec2() * zoom + canvas_offset;
        let positions = routing
            .node_positions
            .iter()
            .map(|position| transform(*position))
            .collect::<Vec<_>>();
        if let Some(partition_labels) = &routing.partition_labels {
            let header_font = FontId::proportional(13.0 * zoom);
            for (partition, label) in partition_labels.iter().enumerate() {
                let center_x = canvas.left()
                    + (24.0 + partition as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0) * zoom;
                painter.text(
                    Pos2::new(center_x, canvas.top() + 10.0 * zoom),
                    Align2::CENTER_CENTER,
                    label,
                    header_font.clone(),
                    colors.key,
                );
            }
        }
        let mut hovered_node = None;
        for (index, node) in graph.nodes.iter().enumerate() {
            let rect = egui::Rect::from_center_size(positions[index], GRAPH_NODE_SIZE * zoom);
            let response = ui.interact(
                rect,
                ui.make_persistent_id(("graph-node", &node.path)),
                Sense::click(),
            );
            show_graph_tooltip(&response, |ui| {
                ui.set_max_width(480.0);
                ui.strong(&node.label);
                ui.monospace(&node.id);
                ui.weak(&node.path);
                show_graph_attributes(ui, ("graph-node-attributes", &node.path), &node.attributes);
            });
            if response.hovered() {
                hovered_node = Some(index);
            }
            if response.clicked() {
                interaction.select_node(
                    index,
                    ui.input(|input| input.modifiers.command || input.modifiers.ctrl),
                );
            }
        }
        if response.drag_started() {
            let start = ui.input(|input| input.pointer.press_origin());
            if let Some(start) = start
                && !positions.iter().any(|center| {
                    egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE * zoom).contains(start)
                })
            {
                interaction.marquee_start = Some(Pos2::ZERO + (start - canvas.min) / zoom);
                interaction.marquee_base =
                    if ui.input(|input| input.modifiers.command || input.modifiers.ctrl) {
                        interaction.selected.clone()
                    } else {
                        HashSet::new()
                    };
            }
        }
        if let Some(start) = interaction.marquee_start
            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
        {
            let end = Pos2::ZERO + (pointer - canvas.min) / zoom;
            let selection_rect = egui::Rect::from_two_pos(start, end);
            interaction.selected = interaction.marquee_base.clone();
            for (index, position) in routing.node_positions.iter().enumerate() {
                if selection_rect
                    .intersects(egui::Rect::from_center_size(*position, GRAPH_NODE_SIZE))
                {
                    interaction.selected.insert(index);
                }
            }
            let rect = egui::Rect::from_two_pos(transform(start), transform(end));
            painter.rect_filled(rect, 0, ui.visuals().selection.bg_fill.gamma_multiply(0.25));
            painter.rect_stroke(
                rect,
                0,
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }
        if response.drag_stopped() {
            interaction.marquee_start = None;
        }
        if response.clicked() {
            interaction.selected.clear();
        }
        let focused = |index: usize| {
            hovered_node.map_or_else(
                || interaction.selected.contains(&index),
                |hovered| hovered == index,
            )
        };
        let has_focus = hovered_node.is_some() || !interaction.selected.is_empty();

        let edge_colors = graph
            .edges
            .iter()
            .map(|edge| graph_edge_color(&edge.label, colors))
            .collect::<Vec<_>>();
        let mut neighbor_colors = vec![None; graph.nodes.len()];
        if has_focus {
            for (edge, color) in graph.edges.iter().zip(&edge_colors) {
                if focused(edge.source) {
                    neighbor_colors[edge.target].get_or_insert(*color);
                }
                if focused(edge.target) {
                    neighbor_colors[edge.source].get_or_insert(*color);
                }
            }
        }

        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let edge_path = &routing.edge_paths[edge_index];
            let is_connected = !has_focus || focused(edge.source) || focused(edge.target);
            let edge_color = if has_focus && !is_connected {
                edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                edge_colors[edge_index]
            };
            let edge_width = if !has_focus {
                1.5
            } else if is_connected {
                2.5
            } else {
                1.0
            };
            let stroke = Stroke::new(edge_width * zoom, edge_color);
            for segment in edge_path.windows(2) {
                painter.line_segment([transform(segment[0]), transform(segment[1])], stroke);
            }
            for (tip, direction) in edge_arrowheads(edge_path, edge.direction) {
                draw_arrow_head(&painter, transform(tip), direction, stroke, zoom);
            }
        }
        for (edge_index, label) in routing.edge_labels.iter().enumerate() {
            if let Some(label) = label
                && let Some([start, end]) = label.leader
            {
                let edge = &graph.edges[edge_index];
                let edge_color = if has_focus && !focused(edge.source) && !focused(edge.target) {
                    edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    edge_colors[edge_index]
                };
                painter.extend(egui::Shape::dashed_line(
                    &[transform(start), transform(end)],
                    Stroke::new(zoom, edge_color.gamma_multiply(0.6)),
                    3.0 * zoom,
                    3.0 * zoom,
                ));
            }
        }
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let edge_color = if has_focus && !focused(edge.source) && !focused(edge.target) {
                edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                edge_colors[edge_index]
            };
            if let Some(label) = &routing.edge_labels[edge_index] {
                let label_rect = egui::Rect::from_min_max(
                    transform(label.background.min),
                    transform(label.background.max),
                );
                painter.rect_filled(
                    label_rect,
                    egui::CornerRadius::same(3),
                    ui.visuals().panel_fill,
                );
                painter.text(
                    transform(label.position),
                    label.alignment,
                    label.text.as_str(),
                    FontId::proportional(12.0 * zoom),
                    edge_color,
                );
                if let Some((number, anchor)) = label.reference {
                    for center in [
                        transform(anchor),
                        transform(Pos2::new(
                            label.background.right() - 11.0,
                            label.background.center().y,
                        )),
                    ] {
                        painter.circle_filled(center, 8.0 * zoom, ui.visuals().panel_fill);
                        painter.circle_stroke(center, 8.0 * zoom, Stroke::new(zoom, edge_color));
                        painter.text(
                            center,
                            Align2::CENTER_CENTER,
                            number.to_string(),
                            FontId::proportional(10.0 * zoom),
                            edge_color,
                        );
                    }
                }
                let response = ui.interact(
                    label_rect,
                    ui.make_persistent_id(("graph-edge-label", edge_index)),
                    Sense::hover(),
                );
                show_graph_tooltip(&response, |ui| {
                    show_graph_edge_information(ui, graph, edge_index, locale)
                });
            }
        }

        for (index, node) in graph.nodes.iter().enumerate() {
            let center = positions[index];
            let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE * zoom);
            let active_match = node
                .search_paths
                .iter()
                .any(|path| active_path == Some(path.as_str()));
            let is_match = active_match
                || node
                    .search_paths
                    .iter()
                    .any(|path| matching_paths.contains(path.as_str()));
            let is_selected = interaction.selected.contains(&index);
            let is_focused = focused(index);
            let neighbor_color = neighbor_colors[index];
            let is_dimmed = has_focus && !is_focused && neighbor_color.is_none() && !is_selected;
            let base_stroke = if active_match {
                Stroke::new(2.5, colors.active_match)
            } else if is_match {
                Stroke::new(2.0, colors.matched)
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke
            };
            let node_stroke = if is_selected || is_focused {
                Stroke::new(3.0, colors.active_match)
            } else if let Some(color) = neighbor_color {
                Stroke::new(
                    2.5,
                    if active_match {
                        colors.active_match
                    } else if is_match {
                        colors.matched
                    } else {
                        color
                    },
                )
            } else if is_dimmed {
                Stroke::new(
                    base_stroke.width,
                    base_stroke.color.gamma_multiply(GRAPH_DIM_FACTOR),
                )
            } else {
                base_stroke
            };
            let node_fill = if is_focused || is_selected {
                ui.visuals().selection.bg_fill
            } else if is_dimmed {
                ui.visuals().faint_bg_color.gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                ui.visuals().faint_bg_color
            };
            painter.rect_filled(rect, egui::CornerRadius::same(6), node_fill);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(6),
                Stroke::new(node_stroke.width * zoom, node_stroke.color),
                egui::StrokeKind::Inside,
            );
            if is_selected {
                let marker = Pos2::new(rect.right() - 11.0 * zoom, rect.top() + 11.0 * zoom);
                painter.circle_stroke(
                    marker,
                    6.0 * zoom,
                    Stroke::new(1.5 * zoom, ui.visuals().panel_fill),
                );
                painter.circle_filled(marker, 4.0 * zoom, colors.active_match);
            }
            let label_font = FontId::proportional(15.0 * zoom);
            let label = shorten_to_width(
                &painter,
                &node.label,
                24,
                &label_font,
                (GRAPH_NODE_SIZE.x - 16.0) * zoom,
                if is_dimmed {
                    colors.key.gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    colors.key
                },
            );
            painter.text(
                Pos2::new(center.x, center.y - 9.0 * zoom),
                Align2::CENTER_CENTER,
                label,
                label_font,
                if is_dimmed {
                    colors.key.gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    colors.key
                },
            );
            let id_color = if is_dimmed {
                ui.visuals()
                    .weak_text_color()
                    .gamma_multiply(GRAPH_DIM_FACTOR)
            } else {
                ui.visuals().weak_text_color()
            };
            let id_font = FontId::monospace(11.0 * zoom);
            let id = shorten_to_width(
                &painter,
                &node.id,
                26,
                &id_font,
                (GRAPH_NODE_SIZE.x - 16.0) * zoom,
                id_color,
            );
            painter.text(
                Pos2::new(center.x, center.y + 13.0 * zoom),
                Align2::CENTER_CENTER,
                id,
                id_font,
                id_color,
            );
        }
        let mut edge_hit = false;
        if interaction.marquee_start.is_none()
            && !ui.input(|input| input.pointer.any_down())
            && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
            && ui.clip_rect().contains(pointer)
            && canvas.contains(pointer)
            && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id())
            && !positions.iter().any(|center| {
                egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE * zoom).contains(pointer)
            })
            && !routing.edge_labels.iter().flatten().any(|label| {
                egui::Rect::from_min_max(
                    transform(label.background.min),
                    transform(label.background.max),
                )
                .contains(pointer)
            })
        {
            let local_pointer = Pos2::ZERO + (pointer - canvas.min) / zoom;
            let edges = graph_edges_at_pointer(&routing.edge_paths, local_pointer, 6.0 / zoom);
            if !edges.is_empty() {
                edge_hit = true;
                interaction.edge_hover = Some((pointer, edges));
            }
        }
        if let Some((pointer, edges)) = &interaction.edge_hover {
            let response = ui.interact(
                egui::Rect::from_center_size(*pointer, Vec2::splat(2.0)),
                ui.make_persistent_id(("graph-edge-hover", routing.graph_fingerprint)),
                Sense::hover(),
            );
            if edge_hit || response.is_tooltip_open() {
                show_graph_tooltip(&response, |ui| {
                    for (index, &edge_index) in edges.iter().enumerate() {
                        if index > 0 {
                            ui.separator();
                        }
                        show_graph_edge_information(ui, graph, edge_index, locale);
                    }
                });
            } else {
                interaction.edge_hover = None;
            }
        }
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(interaction_id, interaction));
    export_requested
}

pub(super) fn graph_edges_at_pointer(
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

fn show_graph_edge_information(
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

fn show_graph_attributes(
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

fn show_graph_tooltip(response: &egui::Response, contents: impl FnOnce(&mut egui::Ui)) {
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
    let tooltip = if hovered {
        egui::Tooltip::for_widget(response)
    } else {
        egui::Tooltip::for_enabled(response)
    };
    tooltip.show(contents);
}
