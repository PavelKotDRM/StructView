use super::super::*;
use super::interaction::{GraphInteractionState, graph_keyboard};
use super::tooltip::{
    graph_edges_at_pointer, show_graph_attributes, show_graph_edge_information, show_graph_tooltip,
};
use crate::app::views::diagram::DiagramAction;
use std::collections::HashSet;

/// Отрисовать граф идентификаторов и зависимостей.
pub(in crate::app) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    search: &SearchState,
    locale: Locale,
) {
    let colors = SyntaxColors::new(ui.visuals());
    let interaction_id = egui::Id::new(("graph-interaction", routing.graph_fingerprint));
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
        return;
    }
    if graph.edges.is_empty() {
        ui.label(RichText::new(locale.text(TextKey::GraphNoRelationships)).weak());
    }
    let (_, viewport) = ui.allocate_space(ui.available_size().max(Vec2::splat(1.0)));
    let canvas_response = ui.interact(
        viewport,
        ui.id().with("graph-canvas"),
        Sense::click_and_drag(),
    );
    canvas_response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            locale.text(TextKey::GraphView),
        )
    });
    if canvas_response.clicked() || canvas_response.drag_started() {
        canvas_response.request_focus();
    }
    let mut fit_requested = false;
    let actions = ui
        .ctx()
        .data_mut(|data| {
            data.remove_temp::<Vec<DiagramAction>>(egui::Id::new((
                "graph-menu-actions",
                routing.graph_fingerprint,
            )))
        })
        .unwrap_or_default();
    for action in actions {
        match action {
            DiagramAction::ZoomOut => crate::app::views::diagram::zoom_at(
                &mut interaction.zoom,
                &mut interaction.pan,
                1.0 / crate::app::views::diagram::ZOOM_STEP,
                (viewport.size() * 0.5).to_pos2(),
            ),
            DiagramAction::ZoomIn => crate::app::views::diagram::zoom_at(
                &mut interaction.zoom,
                &mut interaction.pan,
                crate::app::views::diagram::ZOOM_STEP,
                (viewport.size() * 0.5).to_pos2(),
            ),
            DiagramAction::ActualSize => {
                let factor = 1.0 / interaction.zoom;
                crate::app::views::diagram::zoom_at(
                    &mut interaction.zoom,
                    &mut interaction.pan,
                    factor,
                    (viewport.size() * 0.5).to_pos2(),
                );
            }
            DiagramAction::Fit => fit_requested = true,
            DiagramAction::SelectAll => interaction.selected = (0..graph.nodes.len()).collect(),
            DiagramAction::ClearSelection => interaction.selected.clear(),
        }
    }
    if canvas_response.contains_pointer()
        && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
    {
        crate::app::views::diagram::zoom_at(
            &mut interaction.zoom,
            &mut interaction.pan,
            crate::app::views::diagram::wheel_zoom(ui),
            pointer - viewport.min.to_vec2(),
        );
    }
    if canvas_response.has_focus() {
        let factor = crate::app::views::diagram::keyboard_zoom(ui);
        crate::app::views::diagram::zoom_at(
            &mut interaction.zoom,
            &mut interaction.pan,
            factor,
            (viewport.size() * 0.5).to_pos2(),
        );
        fit_requested |= ui.input(|i| i.key_pressed(egui::Key::Home));
        graph_keyboard(ui, graph, routing, viewport.size(), &mut interaction);
    }
    if canvas_response.dragged()
        && interaction.marquee_start.is_none()
        && !ui.input(|i| i.modifiers.shift)
    {
        interaction.pan += ui.input(|i| i.pointer.delta());
    }
    if fit_requested {
        crate::app::views::diagram::fit_to(
            &mut interaction.zoom,
            &mut interaction.pan,
            viewport.size(),
            routing.content_size,
        );
    }
    let active_search = search.current_match_path();
    if interaction.active_search.as_deref() != active_search {
        interaction.active_search = active_search.map(str::to_string);
        if let Some(path) = active_search
            && let Some(index) = graph
                .nodes
                .iter()
                .position(|node| node.search_paths.iter().any(|candidate| candidate == path))
        {
            interaction.cursor = index;
            interaction.pan =
                viewport.size() * 0.5 - routing.node_positions[index].to_vec2() * interaction.zoom;
        }
    }
    let zoom = interaction.zoom;
    let matching_paths = search
        .matches
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let active_path = search.current_match_path();

    ui.scope_builder(egui::UiBuilder::new().max_rect(viewport), |ui| {
        ui.set_clip_rect(viewport.intersect(ui.clip_rect()));
        let response = &canvas_response;
        let painter = ui.painter_at(viewport);
        let canvas = viewport;
        let canvas_offset = canvas.min.to_vec2() + interaction.pan;
        let transform = |point: Pos2| Pos2::ZERO + point.to_vec2() * zoom + canvas_offset;
        let positions = routing
            .node_positions
            .iter()
            .map(|position| transform(*position))
            .collect::<Vec<_>>();
        if let Some(partition_labels) = &routing.partition_labels {
            let header_font = FontId::proportional(13.0 * zoom);
            for (partition, label) in partition_labels.iter().enumerate() {
                let position = transform(Pos2::new(
                    24.0 + partition as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                    10.0,
                ));
                painter.text(
                    position,
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
            if !rect.intersects(viewport) {
                continue;
            }
            let response = ui.interact(
                rect.intersect(viewport),
                ui.make_persistent_id(("graph-node", &node.path)),
                Sense::click(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    true,
                    interaction.selected.contains(&index),
                    format!("{}: {} ({})", node.label, node.id, node.path),
                )
            });
            if response.has_focus() {
                if ui.input(|i| {
                    i.events
                        .iter()
                        .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }))
                }) {
                    ui.ctx().request_repaint();
                }
                interaction.cursor = index;
                graph_keyboard(ui, graph, routing, viewport.size(), &mut interaction);
                let factor = crate::app::views::diagram::keyboard_zoom(ui);
                crate::app::views::diagram::zoom_at(
                    &mut interaction.zoom,
                    &mut interaction.pan,
                    factor,
                    (viewport.size() * 0.5).to_pos2(),
                );
                if ui.input(|i| {
                    [
                        egui::Key::Home,
                        egui::Key::ArrowUp,
                        egui::Key::ArrowDown,
                        egui::Key::ArrowLeft,
                        egui::Key::ArrowRight,
                        egui::Key::Enter,
                        egui::Key::Space,
                    ]
                    .iter()
                    .any(|&key| i.key_pressed(key))
                }) {
                    canvas_response.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Home)) {
                        crate::app::views::diagram::fit_to(
                            &mut interaction.zoom,
                            &mut interaction.pan,
                            viewport.size(),
                            routing.content_size,
                        );
                    }
                }
            }
            show_graph_tooltip(&response, false, |ui| {
                ui.set_max_width(480.0);
                ui.strong(&node.label);
                ui.monospace(&node.id);
                ui.weak(&node.path);
                show_graph_attributes(ui, ("graph-node-attributes", &node.path), &node.attributes);
            });
            if response.hovered() {
                hovered_node = Some(index);
            }
            if response.clicked() && !crate::app::views::diagram::keyboard_activate(ui) {
                canvas_response.request_focus();
                interaction.select_node(
                    index,
                    ui.input(|input| input.modifiers.command || input.modifiers.ctrl),
                );
            }
        }
        if response.drag_started() && ui.input(|i| i.modifiers.shift) {
            let start = ui.input(|input| input.pointer.press_origin());
            if let Some(start) = start
                && !positions.iter().any(|center| {
                    egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE * zoom).contains(start)
                })
            {
                interaction.marquee_start =
                    Some(Pos2::ZERO + (start - canvas.min - interaction.pan) / zoom);
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
            let end = Pos2::ZERO + (pointer - canvas.min - interaction.pan) / zoom;
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
        if response.clicked() && !crate::app::views::diagram::keyboard_activate(ui) {
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
            let points = edge_path.iter().copied().map(transform).collect::<Vec<_>>();
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(
                    stroke.width + 2.0 * GRAPH_EDGE_OUTLINE_WIDTH,
                    ui.visuals().panel_fill,
                ),
            ));
            painter.add(egui::Shape::line(points, stroke));
            for (tip, direction) in edge_arrowheads(edge_path, edge.direction) {
                draw_arrow_head(
                    &painter,
                    transform(tip),
                    direction,
                    stroke,
                    ui.visuals().panel_fill,
                    zoom,
                );
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
                show_graph_tooltip(&response, false, |ui| {
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
        let pin_route_tooltip = ui.input(|input| input.modifiers.ctrl || input.modifiers.command);
        if !pin_route_tooltip {
            interaction.edge_hover_pinned = false;
        }
        let mut edge_hit = false;
        if !interaction.edge_hover_pinned
            && interaction.marquee_start.is_none()
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
            let local_pointer = Pos2::ZERO + (pointer - canvas.min - interaction.pan) / zoom;
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
            let tooltip_open = response.is_tooltip_open();
            if pin_route_tooltip && (edge_hit || tooltip_open) {
                interaction.edge_hover_pinned = true;
            }
            if edge_hit || tooltip_open || interaction.edge_hover_pinned {
                show_graph_tooltip(&response, interaction.edge_hover_pinned, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("graph-overlapping-edges", routing.graph_fingerprint))
                        .max_height(480.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (index, &edge_index) in edges.iter().enumerate() {
                                if index > 0 {
                                    ui.separator();
                                }
                                show_graph_edge_information(ui, graph, edge_index, locale);
                            }
                        });
                });
            } else {
                interaction.edge_hover = None;
                interaction.edge_hover_pinned = false;
            }
        }
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(interaction_id, interaction));
}
