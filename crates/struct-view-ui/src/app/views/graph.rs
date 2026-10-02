use super::*;

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use struct_view_core::parser::JsonNode;

pub(super) const GRAPH_DIM_FACTOR: f32 = 0.18;
const GRAPH_EDGE_LABEL_CHAR_WIDTH: f32 = 8.0;
const GRAPH_EDGE_LABEL_HEIGHT: f32 = 16.0;

#[derive(Default)]
pub(in crate::app) struct GraphCalculationState {
    receiver: Option<Receiver<GraphCalculationResult>>,
    result: Option<GraphCalculationResult>,
    error: Option<String>,
}

impl GraphCalculationState {
    pub(in crate::app) fn ensure_started(&mut self, root: &JsonNode, ctx: &egui::Context) {
        if self.receiver.is_some() || self.result.is_some() || self.error.is_some() {
            return;
        }

        let root = root.clone();
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("struct-view-graph-layout".to_string())
            .spawn(move || {
                let result = build_graph_calculation(root);
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.receiver = Some(receiver);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(in crate::app) fn poll(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => {
                self.receiver = None;
                self.result = Some(result);
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                self.error = Some("The graph calculation worker terminated unexpectedly".into());
                ctx.request_repaint();
            }
        }
    }

    pub(in crate::app) fn result(&self) -> Option<&GraphCalculationResult> {
        self.result.as_ref()
    }

    pub(in crate::app) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

pub(in crate::app) struct GraphCalculationResult {
    pub(in crate::app) graph: RelationshipGraph,
    pub(in crate::app) routing: GraphRoutingLayout,
}

pub(in crate::app) struct GraphRoutingLayout {
    graph_fingerprint: u64,
    node_positions: Vec<Pos2>,
    edge_paths: Vec<Vec<Pos2>>,
    edge_labels: Vec<Option<GraphEdgeLabelLayout>>,
    bipartition_labels: Option<[String; 2]>,
    content_size: Vec2,
}

fn build_graph_calculation(root: JsonNode) -> GraphCalculationResult {
    let graph = build_relationship_graph(&root);
    let routing = build_graph_routing_layout(&graph);
    GraphCalculationResult { graph, routing }
}

/// Отрисовать граф идентификаторов и зависимостей.
pub(in crate::app) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    search: &SearchState,
    locale: Locale,
) {
    let colors = SyntaxColors::new(ui.visuals());
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
    let matching_paths = search
        .matches
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let active_path = search.current_match_path();

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let viewport = ui.available_size_before_wrap();
            let canvas_size = Vec2::new(
                viewport.x.max(routing.content_size.x),
                viewport.y.max(routing.content_size.y),
            );
            let (response, painter) = ui.allocate_painter(canvas_size, Sense::hover());
            let canvas = response.rect;
            let canvas_offset = canvas.min.to_vec2();
            let positions = routing
                .node_positions
                .iter()
                .map(|position| *position + canvas_offset)
                .collect::<Vec<_>>();
            if let Some(partition_labels) = &routing.bipartition_labels {
                let header_font = FontId::proportional(13.0);
                for (partition, label) in partition_labels.iter().enumerate() {
                    let center_x = canvas.left()
                        + 24.0
                        + partition as f32 * GRAPH_STEP.x
                        + GRAPH_NODE_SIZE.x / 2.0;
                    painter.text(
                        Pos2::new(center_x, canvas.top() + 10.0),
                        Align2::CENTER_CENTER,
                        label,
                        header_font.clone(),
                        colors.key,
                    );
                }
            }
            let selection_id =
                ui.make_persistent_id(("graph-selected-node", routing.graph_fingerprint));
            let mut selected_path = ui
                .ctx()
                .data(|data| data.get_temp::<Option<String>>(selection_id).flatten());
            let mut hovered_node = None;
            for (index, node) in graph.nodes.iter().enumerate() {
                let rect = egui::Rect::from_center_size(positions[index], GRAPH_NODE_SIZE);
                let response = ui
                    .interact(
                        rect,
                        ui.make_persistent_id(("graph-node", &node.path)),
                        Sense::click(),
                    )
                    .on_hover_text(format!("{}\n{}\n{}", node.label, node.id, node.path));
                if response.hovered() {
                    hovered_node = Some(index);
                }
                if response.clicked() {
                    selected_path = if selected_path.as_deref() == Some(node.path.as_str()) {
                        None
                    } else {
                        Some(node.path.clone())
                    };
                }
            }
            if selected_path
                .as_deref()
                .is_some_and(|path| !graph.nodes.iter().any(|node| node.path == path))
            {
                selected_path = None;
            }
            ui.ctx()
                .data_mut(|data| data.insert_temp(selection_id, selected_path.clone()));
            let selected_node = selected_path
                .as_deref()
                .and_then(|path| graph.nodes.iter().position(|node| node.path == path));
            let focus_node = hovered_node.or(selected_node);

            let edge_colors = graph
                .edges
                .iter()
                .map(|edge| graph_edge_color(&edge.label, colors))
                .collect::<Vec<_>>();
            let mut neighbor_colors = vec![None; graph.nodes.len()];
            if let Some(focus_node) = focus_node {
                for (edge, color) in graph.edges.iter().zip(&edge_colors) {
                    if edge.source == focus_node {
                        neighbor_colors[edge.target].get_or_insert(*color);
                    } else if edge.target == focus_node {
                        neighbor_colors[edge.source].get_or_insert(*color);
                    }
                }
            }

            for (edge_index, edge) in graph.edges.iter().enumerate() {
                let edge_path = &routing.edge_paths[edge_index];
                let is_connected =
                    focus_node.is_none_or(|focus| edge.source == focus || edge.target == focus);
                let edge_color = if focus_node.is_some() && !is_connected {
                    edge_colors[edge_index].gamma_multiply(GRAPH_DIM_FACTOR)
                } else {
                    edge_colors[edge_index]
                };
                let edge_width = if focus_node.is_none() {
                    1.5
                } else if is_connected {
                    2.5
                } else {
                    1.0
                };
                let stroke = Stroke::new(edge_width, edge_color);
                for segment in edge_path.windows(2) {
                    painter.line_segment(
                        [segment[0] + canvas_offset, segment[1] + canvas_offset],
                        stroke,
                    );
                }
                let final_segment = edge_path
                    .windows(2)
                    .last()
                    .expect("graph edges must connect distinct nodes");
                if graph.directed {
                    let direction = (final_segment[1] - final_segment[0]).normalized();
                    draw_arrow_head(
                        &painter,
                        final_segment[1] + canvas_offset,
                        direction,
                        stroke,
                    );
                }
                if let Some(label) = &routing.edge_labels[edge_index] {
                    let label_rect = label.background.translate(canvas_offset);
                    painter.rect_filled(
                        label_rect,
                        egui::CornerRadius::same(3),
                        ui.visuals().panel_fill,
                    );
                    painter.text(
                        label.position + canvas_offset,
                        label.alignment,
                        label.text.as_str(),
                        FontId::proportional(12.0),
                        edge_color,
                    );
                    ui.interact(
                        label_rect,
                        ui.make_persistent_id((
                            "graph-edge-label",
                            edge.source,
                            edge.target,
                            edge.label.as_str(),
                        )),
                        Sense::hover(),
                    )
                    .on_hover_text(&edge.label);
                }
            }

            for (index, node) in graph.nodes.iter().enumerate() {
                let center = positions[index];
                let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE);
                let active_match = node
                    .search_paths
                    .iter()
                    .any(|path| active_path == Some(path.as_str()));
                let is_match = active_match
                    || node
                        .search_paths
                        .iter()
                        .any(|path| matching_paths.contains(path.as_str()));
                let is_selected = selected_node == Some(index);
                let is_focused = focus_node == Some(index);
                let neighbor_color = neighbor_colors[index];
                let is_dimmed =
                    focus_node.is_some() && !is_focused && neighbor_color.is_none() && !is_selected;
                let base_stroke = if active_match {
                    Stroke::new(2.5, colors.active_match)
                } else if is_match {
                    Stroke::new(2.0, colors.matched)
                } else {
                    ui.visuals().widgets.noninteractive.bg_stroke
                };
                let node_stroke = if is_selected {
                    Stroke::new(3.0, colors.active_match)
                } else if is_focused {
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
                    node_stroke,
                    egui::StrokeKind::Inside,
                );
                if is_selected {
                    let marker = Pos2::new(rect.right() - 11.0, rect.top() + 11.0);
                    painter.circle_stroke(marker, 6.0, Stroke::new(1.5, ui.visuals().panel_fill));
                    painter.circle_filled(marker, 4.0, colors.active_match);
                }
                let label_font = FontId::proportional(15.0);
                let label = shorten_to_width(
                    &painter,
                    &node.label,
                    24,
                    &label_font,
                    GRAPH_NODE_SIZE.x - 16.0,
                    if is_dimmed {
                        colors.key.gamma_multiply(GRAPH_DIM_FACTOR)
                    } else {
                        colors.key
                    },
                );
                painter.text(
                    Pos2::new(center.x, center.y - 9.0),
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
                let id_font = FontId::monospace(11.0);
                let id = shorten_to_width(
                    &painter,
                    &node.id,
                    26,
                    &id_font,
                    GRAPH_NODE_SIZE.x - 16.0,
                    id_color,
                );
                painter.text(
                    Pos2::new(center.x, center.y + 13.0),
                    Align2::CENTER_CENTER,
                    id,
                    id_font,
                    id_color,
                );
            }
        });
}

pub(super) fn build_graph_routing_layout(graph: &RelationshipGraph) -> GraphRoutingLayout {
    let node_count = graph.nodes.len();
    let (node_positions, content_size) = if graph.bipartition.is_some() {
        let mut partition_rows = [0; 2];
        let mut positions = vec![Pos2::ZERO; node_count];
        for (index, node) in graph.nodes.iter().enumerate() {
            let partition = node.partition.unwrap_or(0).min(1);
            let row = partition_rows[partition];
            partition_rows[partition] += 1;
            positions[index] = Pos2::new(
                24.0 + partition as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                24.0 + row as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
            );
        }
        let rows = partition_rows[0].max(partition_rows[1]);
        (
            positions,
            Vec2::new(2.0 * GRAPH_STEP.x + 48.0, rows as f32 * GRAPH_STEP.y + 48.0),
        )
    } else {
        let columns = ((node_count as f32).sqrt().ceil() as usize).max(1);
        let rows = node_count.div_ceil(columns);
        let positions = (0..node_count)
            .map(|index| {
                let column = index % columns;
                let row = index / columns;
                Pos2::new(
                    24.0 + column as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                    24.0 + row as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                )
            })
            .collect::<Vec<_>>();
        (
            positions,
            Vec2::new(
                columns as f32 * GRAPH_STEP.x + 48.0,
                rows as f32 * GRAPH_STEP.y + 48.0,
            ),
        )
    };
    let routing_grid = GraphRoutingGrid::new(&node_positions);
    let edge_endpoints = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect::<Vec<_>>();
    let edge_ports = graph_edge_ports(&node_positions, &edge_endpoints);
    let mut routed_edges = Vec::with_capacity(graph.edges.len());
    for (index, edge) in graph.edges.iter().enumerate() {
        let points = routing_grid.route_edge_with_ports(
            edge.source,
            edge.target,
            edge_ports[index],
            &routed_edges,
        );
        routed_edges.push(points);
    }
    let node_rects = node_positions
        .iter()
        .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
        .collect::<Vec<_>>();
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, content_size);
    let mut occupied_label_rects = Vec::with_capacity(graph.edges.len());
    let edge_labels = graph
        .edges
        .iter()
        .zip(&routed_edges)
        .map(|(edge, points)| {
            let (position, alignment, max_width) = edge_label_placement(points, &node_rects);
            let text = shorten_graph_edge_label(&edge.label, max_width);
            place_edge_label(
                &text,
                position,
                alignment,
                canvas,
                &node_rects,
                &occupied_label_rects,
            )
            .map(|(position, background)| {
                occupied_label_rects.push(background);
                GraphEdgeLabelLayout {
                    text,
                    position,
                    alignment,
                    background,
                }
            })
        })
        .collect();

    GraphRoutingLayout {
        graph_fingerprint: relationship_graph_fingerprint(graph),
        node_positions,
        edge_paths: routed_edges,
        edge_labels,
        bipartition_labels: graph.bipartition.clone(),
        content_size,
    }
}

struct GraphEdgeLabelLayout {
    text: String,
    position: Pos2,
    alignment: Align2,
    background: egui::Rect,
}

fn relationship_graph_fingerprint(graph: &RelationshipGraph) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for node in &graph.nodes {
        node.id.hash(&mut hasher);
        node.label.hash(&mut hasher);
        node.path.hash(&mut hasher);
        node.partition.hash(&mut hasher);
    }
    for edge in &graph.edges {
        edge.source.hash(&mut hasher);
        edge.target.hash(&mut hasher);
        edge.label.hash(&mut hasher);
    }
    graph.directed.hash(&mut hasher);
    graph.bipartition.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn graph_edge_color(label: &str, colors: SyntaxColors) -> egui::Color32 {
    let hash = label.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let palette = [
        colors.number,
        colors.string,
        colors.boolean,
        colors.metadata,
        colors.comment,
        colors.null,
        colors.matched,
        colors.error,
    ];
    palette[(hash % palette.len() as u64) as usize]
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum GraphNodeSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl GraphNodeSide {
    fn for_direction(direction: Vec2) -> Self {
        let half_size = GRAPH_NODE_SIZE / 2.0;
        let horizontal_distance = if direction.x.abs() > 0.0 {
            half_size.x / direction.x.abs()
        } else {
            f32::INFINITY
        };
        let vertical_distance = if direction.y.abs() > 0.0 {
            half_size.y / direction.y.abs()
        } else {
            f32::INFINITY
        };
        if horizontal_distance <= vertical_distance {
            if direction.x > 0.0 {
                Self::Right
            } else {
                Self::Left
            }
        } else if direction.y > 0.0 {
            Self::Bottom
        } else {
            Self::Top
        }
    }

    fn max_lane_offset(self) -> f32 {
        match self {
            Self::Left | Self::Right => GRAPH_NODE_SIZE.y / 2.0 - 10.0,
            Self::Top | Self::Bottom => GRAPH_NODE_SIZE.x / 2.0 - 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GraphEdgePorts {
    pub(super) source_side: GraphNodeSide,
    pub(super) target_side: GraphNodeSide,
    pub(super) source_offset: f32,
    pub(super) target_offset: f32,
}

pub(super) fn graph_edge_ports(
    node_positions: &[Pos2],
    edge_endpoints: &[(usize, usize)],
) -> Vec<GraphEdgePorts> {
    let mut source_groups: HashMap<(usize, GraphNodeSide), Vec<usize>> = HashMap::new();
    let mut target_groups: HashMap<(usize, GraphNodeSide), Vec<usize>> = HashMap::new();
    let mut ports = Vec::with_capacity(edge_endpoints.len());

    for (edge_index, &(source, target)) in edge_endpoints.iter().enumerate() {
        let direction = (node_positions[target] - node_positions[source]).normalized();
        let source_side = GraphNodeSide::for_direction(direction);
        let target_side = GraphNodeSide::for_direction(-direction);
        ports.push(GraphEdgePorts {
            source_side,
            target_side,
            source_offset: 0.0,
            target_offset: 0.0,
        });
        source_groups
            .entry((source, source_side))
            .or_default()
            .push(edge_index);
        target_groups
            .entry((target, target_side))
            .or_default()
            .push(edge_index);
    }

    for ((_, side), edge_indices) in source_groups {
        let lane_spacing = (2.0 * side.max_lane_offset()
            / edge_indices.len().saturating_sub(1).max(1) as f32)
            .min(16.0);
        for (lane, edge_index) in edge_indices.iter().enumerate() {
            ports[*edge_index].source_offset =
                (lane as f32 - (edge_indices.len() - 1) as f32 / 2.0) * lane_spacing;
        }
    }
    for ((_, side), edge_indices) in target_groups {
        let lane_spacing = (2.0 * side.max_lane_offset()
            / edge_indices.len().saturating_sub(1).max(1) as f32)
            .min(16.0);
        for (lane, edge_index) in edge_indices.iter().enumerate() {
            ports[*edge_index].target_offset =
                (lane as f32 - (edge_indices.len() - 1) as f32 / 2.0) * lane_spacing;
        }
    }

    ports
}

pub(super) const GRAPH_ROUTE_CLEARANCE: f32 = 18.0;
pub(super) const GRAPH_EDGE_CLEARANCE: f32 = 8.0;
const GRAPH_ROUTE_TURN_PENALTY: f32 = 48.0;
const GRAPH_EDGE_ROUTE_PENALTY: f32 = 500.0;
const GRAPH_EDGE_CROSSING_PENALTY: f32 = 10_000.0;
const GRAPH_ROUTE_DIRECTIONS: usize = 3;
const GRAPH_ROUTE_HORIZONTAL: usize = 0;
const GRAPH_ROUTE_VERTICAL: usize = 1;
const GRAPH_ROUTE_NO_DIRECTION: usize = 2;

#[derive(Clone, Copy)]
struct GraphRoutePort {
    card: Pos2,
    route: Pos2,
}

#[derive(Debug)]
struct GraphRouteQueueEntry {
    estimated_total: f32,
    distance: f32,
    state: usize,
}

impl PartialEq for GraphRouteQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.estimated_total.total_cmp(&other.estimated_total) == Ordering::Equal
            && self.distance.total_cmp(&other.distance) == Ordering::Equal
            && self.state == other.state
    }
}

impl Eq for GraphRouteQueueEntry {}

impl PartialOrd for GraphRouteQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GraphRouteQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .total_cmp(&self.estimated_total)
            .then_with(|| other.distance.total_cmp(&self.distance))
            .then_with(|| other.state.cmp(&self.state))
    }
}

pub(super) struct GraphRoutingGrid {
    node_positions: Vec<Pos2>,
    node_rects: Vec<egui::Rect>,
    obstacles: Vec<egui::Rect>,
    x_coordinates: Vec<f32>,
    y_coordinates: Vec<f32>,
}

impl GraphRoutingGrid {
    pub(super) fn new(node_positions: &[Pos2]) -> Self {
        let node_rects = node_positions
            .iter()
            .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
            .collect::<Vec<_>>();
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(GRAPH_ROUTE_CLEARANCE))
            .collect::<Vec<_>>();
        let mut x_coordinates = Vec::with_capacity(node_positions.len() * 3);
        let mut y_coordinates = Vec::with_capacity(node_positions.len() * 3);

        for (center, obstacle) in node_positions.iter().zip(&obstacles) {
            x_coordinates.extend([obstacle.left(), center.x, obstacle.right()]);
            y_coordinates.extend([obstacle.top(), center.y, obstacle.bottom()]);
        }
        sort_unique_coordinates(&mut x_coordinates);
        sort_unique_coordinates(&mut y_coordinates);

        Self {
            node_positions: node_positions.to_vec(),
            node_rects,
            obstacles,
            x_coordinates,
            y_coordinates,
        }
    }

    #[cfg(test)]
    pub(super) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        let direction = (self.node_positions[target] - self.node_positions[source]).normalized();
        self.route_edge_with_ports(
            source,
            target,
            GraphEdgePorts {
                source_side: GraphNodeSide::for_direction(direction),
                target_side: GraphNodeSide::for_direction(-direction),
                source_offset: 0.0,
                target_offset: 0.0,
            },
            &[],
        )
    }

    pub(super) fn route_edge_with_ports(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let source_port = self.route_port(source, edge_ports.source_side, edge_ports.source_offset);
        let target_port = self.route_port(target, edge_ports.target_side, edge_ports.target_offset);
        let line_start = source_port.card;
        let line_end = target_port.card;
        let crosses_another_node = self.obstacles.iter().enumerate().any(|(index, obstacle)| {
            index != source
                && index != target
                && segment_intersects_rect(line_start, line_end, *obstacle)
        });
        let overlaps_another_edge =
            routed_edges
                .iter()
                .flat_map(|route| route.windows(2))
                .any(|segment| {
                    segments_within_clearance(
                        line_start,
                        line_end,
                        segment[0],
                        segment[1],
                        GRAPH_EDGE_CLEARANCE,
                    )
                });
        if !crosses_another_node && !overlaps_another_edge {
            return vec![line_start, line_end];
        }

        let mut x_coordinates = self.x_coordinates.clone();
        x_coordinates.extend([source_port.route.x, target_port.route.x]);
        sort_unique_coordinates(&mut x_coordinates);
        let mut y_coordinates = self.y_coordinates.clone();
        y_coordinates.extend([source_port.route.y, target_port.route.y]);
        sort_unique_coordinates(&mut y_coordinates);
        let route = self.find_orthogonal_path(
            source_port.route,
            target_port.route,
            &x_coordinates,
            &y_coordinates,
            routed_edges,
        );

        let mut points = Vec::with_capacity(route.len() + 2);
        points.push(source_port.card);
        points.extend(route);
        points.push(target_port.card);
        points
    }

    fn route_port(&self, node: usize, side: GraphNodeSide, offset: f32) -> GraphRoutePort {
        let center = self.node_positions[node];
        let card = self.node_rects[node];
        let route = self.obstacles[node];
        match side {
            GraphNodeSide::Left => GraphRoutePort {
                card: Pos2::new(card.left(), center.y + offset),
                route: Pos2::new(route.left(), center.y + offset),
            },
            GraphNodeSide::Right => GraphRoutePort {
                card: Pos2::new(card.right(), center.y + offset),
                route: Pos2::new(route.right(), center.y + offset),
            },
            GraphNodeSide::Top => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.top()),
                route: Pos2::new(center.x + offset, route.top()),
            },
            GraphNodeSide::Bottom => GraphRoutePort {
                card: Pos2::new(center.x + offset, card.bottom()),
                route: Pos2::new(center.x + offset, route.bottom()),
            },
        }
    }

    fn find_orthogonal_path(
        &self,
        source: Pos2,
        target: Pos2,
        x_coordinates: &[f32],
        y_coordinates: &[f32],
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let width = x_coordinates.len();
        let vertex_count = width * y_coordinates.len();
        let state_count = vertex_count * GRAPH_ROUTE_DIRECTIONS;
        let mut distances = vec![f32::INFINITY; state_count];
        let mut previous_states = vec![usize::MAX; state_count];
        let goal_vertex = Self::vertex_index(target, x_coordinates, y_coordinates);
        let start_vertex = Self::vertex_index(source, x_coordinates, y_coordinates);
        let mut queue = BinaryHeap::new();
        let start_state = start_vertex * GRAPH_ROUTE_DIRECTIONS + GRAPH_ROUTE_NO_DIRECTION;
        distances[start_state] = 0.0;
        queue.push(GraphRouteQueueEntry {
            estimated_total: route_heuristic(source, &[target]),
            distance: 0.0,
            state: start_state,
        });

        let mut goal_state = None;
        while let Some(entry) = queue.pop() {
            if entry.distance > distances[entry.state] {
                continue;
            }

            let current_vertex = entry.state / GRAPH_ROUTE_DIRECTIONS;
            if current_vertex == goal_vertex {
                goal_state = Some(entry.state);
                break;
            }

            let column = current_vertex % width;
            let row = current_vertex / width;
            let current_point = Self::point_at(current_vertex, x_coordinates, y_coordinates);
            let mut neighbors = [None; 4];
            if column > 0 {
                neighbors[0] = Some((
                    current_vertex - 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column] - x_coordinates[column - 1],
                ));
            }
            if column + 1 < width {
                neighbors[1] = Some((
                    current_vertex + 1,
                    GRAPH_ROUTE_HORIZONTAL,
                    x_coordinates[column + 1] - x_coordinates[column],
                ));
            }
            if row > 0 {
                neighbors[2] = Some((
                    current_vertex - width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row] - y_coordinates[row - 1],
                ));
            }
            if row + 1 < y_coordinates.len() {
                neighbors[3] = Some((
                    current_vertex + width,
                    GRAPH_ROUTE_VERTICAL,
                    y_coordinates[row + 1] - y_coordinates[row],
                ));
            }

            for (next_vertex, direction, segment_length) in neighbors.into_iter().flatten() {
                let next_point = Self::point_at(next_vertex, x_coordinates, y_coordinates);
                if self.obstacles.iter().any(|obstacle| {
                    segment_crosses_rect_interior(current_point, next_point, *obstacle)
                }) {
                    continue;
                }
                let edge_overlap_penalty =
                    route_segment_penalty(current_point, next_point, routed_edges);

                let turn_penalty = if entry.state % GRAPH_ROUTE_DIRECTIONS
                    != GRAPH_ROUTE_NO_DIRECTION
                    && entry.state % GRAPH_ROUTE_DIRECTIONS != direction
                {
                    GRAPH_ROUTE_TURN_PENALTY
                } else {
                    0.0
                };
                let next_distance =
                    entry.distance + segment_length + turn_penalty + edge_overlap_penalty;
                let next_state = next_vertex * GRAPH_ROUTE_DIRECTIONS + direction;
                if next_distance >= distances[next_state] {
                    continue;
                }

                distances[next_state] = next_distance;
                previous_states[next_state] = entry.state;
                queue.push(GraphRouteQueueEntry {
                    estimated_total: next_distance + route_heuristic(next_point, &[target]),
                    distance: next_distance,
                    state: next_state,
                });
            }
        }

        let mut state = goal_state.expect("graph nodes must have an obstacle-free route");
        let mut route = Vec::new();
        loop {
            route.push(Self::point_at(
                state / GRAPH_ROUTE_DIRECTIONS,
                x_coordinates,
                y_coordinates,
            ));
            let previous = previous_states[state];
            if previous == usize::MAX {
                break;
            }
            state = previous;
        }
        route.reverse();
        route
    }

    fn vertex_index(point: Pos2, x_coordinates: &[f32], y_coordinates: &[f32]) -> usize {
        let column = x_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.x))
            .expect("graph route x coordinates must include every port");
        let row = y_coordinates
            .binary_search_by(|coordinate| coordinate.total_cmp(&point.y))
            .expect("graph route y coordinates must include every port");
        row * x_coordinates.len() + column
    }

    fn point_at(vertex: usize, x_coordinates: &[f32], y_coordinates: &[f32]) -> Pos2 {
        let width = x_coordinates.len();
        Pos2::new(x_coordinates[vertex % width], y_coordinates[vertex / width])
    }
}

fn sort_unique_coordinates(coordinates: &mut Vec<f32>) {
    coordinates.sort_by(f32::total_cmp);
    coordinates.dedup_by(|left, right| *left == *right);
}

fn route_heuristic(point: Pos2, goals: &[Pos2]) -> f32 {
    goals
        .iter()
        .map(|goal| (point.x - goal.x).abs() + (point.y - goal.y).abs())
        .fold(f32::INFINITY, f32::min)
}

fn segment_intersects_rect(start: Pos2, end: Pos2, rect: egui::Rect) -> bool {
    let direction = end - start;
    let mut first_intersection: f32 = 0.0;
    let mut last_intersection: f32 = 1.0;
    for (origin, delta, minimum, maximum) in [
        (start.x, direction.x, rect.left(), rect.right()),
        (start.y, direction.y, rect.top(), rect.bottom()),
    ] {
        if delta == 0.0 {
            if origin <= minimum || origin >= maximum {
                return false;
            }
            continue;
        }

        let first = (minimum - origin) / delta;
        let last = (maximum - origin) / delta;
        first_intersection = first_intersection.max(first.min(last));
        last_intersection = last_intersection.min(first.max(last));
        if first_intersection >= last_intersection {
            return false;
        }
    }
    first_intersection < last_intersection
}

fn route_segment_penalty(start: Pos2, end: Pos2, routed_edges: &[Vec<Pos2>]) -> f32 {
    routed_edges
        .iter()
        .flat_map(|route| route.windows(2))
        .map(|segment| {
            if segments_intersect(start, end, segment[0], segment[1])
                || segments_within_clearance(start, end, segment[0], segment[1], 1.0)
            {
                GRAPH_EDGE_CROSSING_PENALTY
            } else if segments_within_clearance(
                start,
                end,
                segment[0],
                segment[1],
                GRAPH_EDGE_CLEARANCE,
            ) {
                GRAPH_EDGE_ROUTE_PENALTY
            } else {
                0.0
            }
        })
        .sum()
}

pub(super) fn segments_within_clearance(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
    clearance: f32,
) -> bool {
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return true;
    }
    [
        point_to_segment_distance(first_start, second_start, second_end),
        point_to_segment_distance(first_end, second_start, second_end),
        point_to_segment_distance(second_start, first_start, first_end),
        point_to_segment_distance(second_end, first_start, first_end),
    ]
    .into_iter()
    .any(|distance| distance <= clearance)
}

fn segments_intersect(
    first_start: Pos2,
    first_end: Pos2,
    second_start: Pos2,
    second_end: Pos2,
) -> bool {
    let first_direction = first_end - first_start;
    let second_direction = second_end - second_start;
    let denominator = cross_product(first_direction, second_direction);
    if denominator == 0.0 {
        return false;
    }

    let between_starts = second_start - first_start;
    let first_position = cross_product(between_starts, second_direction) / denominator;
    let second_position = cross_product(between_starts, first_direction) / denominator;
    (0.0..=1.0).contains(&first_position) && (0.0..=1.0).contains(&second_position)
}

fn cross_product(first: Vec2, second: Vec2) -> f32 {
    first.x * second.y - first.y * second.x
}

fn point_to_segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return (point - start).length();
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    (point - (start + segment * projection)).length()
}

pub(super) fn segment_crosses_rect_interior(start: Pos2, end: Pos2, rect: egui::Rect) -> bool {
    if start.y == end.y {
        start.y > rect.top()
            && start.y < rect.bottom()
            && start.x.min(end.x) < rect.right()
            && start.x.max(end.x) > rect.left()
    } else {
        start.x > rect.left()
            && start.x < rect.right()
            && start.y.min(end.y) < rect.bottom()
            && start.y.max(end.y) > rect.top()
    }
}

fn edge_label_placement(points: &[Pos2], node_rects: &[egui::Rect]) -> (Pos2, Align2, f32) {
    if points.len() == 2 {
        let line_start = points[0];
        let line_end = points[1];
        return (
            Pos2::new(
                (line_start.x + line_end.x) / 2.0,
                (line_start.y + line_end.y) / 2.0 - 8.0,
            ),
            Align2::CENTER_CENTER,
            (line_end - line_start).length() - 8.0,
        );
    }

    let longest_horizontal = points
        .windows(2)
        .filter(|segment| segment[0].y == segment[1].y)
        .max_by(|left, right| {
            (left[0].x - left[1].x)
                .abs()
                .total_cmp(&(right[0].x - right[1].x).abs())
        });
    if let Some(segment) = longest_horizontal
        && (segment[0].x - segment[1].x).abs() >= 32.0
    {
        return (
            Pos2::new((segment[0].x + segment[1].x) / 2.0, segment[0].y - 8.0),
            Align2::CENTER_CENTER,
            (segment[0].x - segment[1].x).abs() - 8.0,
        );
    }

    let segment = points
        .windows(2)
        .filter(|segment| segment[0].x == segment[1].x)
        .max_by(|left, right| {
            (left[0].y - left[1].y)
                .abs()
                .total_cmp(&(right[0].y - right[1].y).abs())
        })
        .expect("a detoured graph route must contain a vertical segment");
    let midpoint = (segment[0].y + segment[1].y) / 2.0;
    let left_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.right() <= segment[0].x
        })
        .map(|rect| segment[0].x - rect.right())
        .fold(f32::INFINITY, f32::min);
    let right_space = node_rects
        .iter()
        .filter(|rect| {
            rect.top() < midpoint + 8.0
                && rect.bottom() > midpoint - 8.0
                && rect.left() >= segment[0].x
        })
        .map(|rect| rect.left() - segment[0].x)
        .fold(f32::INFINITY, f32::min);
    let (position, alignment, available_space) = if right_space >= left_space {
        (
            Pos2::new(segment[0].x + 6.0, midpoint),
            Align2::LEFT_CENTER,
            right_space,
        )
    } else {
        (
            Pos2::new(segment[0].x - 6.0, midpoint),
            Align2::RIGHT_CENTER,
            left_space,
        )
    };
    let width = (available_space - 12.0).clamp(0.0, GRAPH_NODE_SIZE.x - 16.0);
    (position, alignment, width)
}

fn shorten_graph_edge_label(label: &str, max_width: f32) -> String {
    let label = single_line_text(label).into_owned();
    let max_chars = (max_width / GRAPH_EDGE_LABEL_CHAR_WIDTH).floor().max(0.0) as usize;
    let max_chars = max_chars.min(18);
    let characters = label.chars().collect::<Vec<_>>();
    if characters.len() <= max_chars {
        return label;
    }
    if max_chars == 0 {
        return String::new();
    }

    let mut shortened = characters
        .into_iter()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

fn place_edge_label(
    label: &str,
    position: Pos2,
    alignment: Align2,
    canvas: egui::Rect,
    node_rects: &[egui::Rect],
    occupied_label_rects: &[egui::Rect],
) -> Option<(Pos2, egui::Rect)> {
    if label.is_empty() {
        return None;
    }

    let size = Vec2::new(
        label.chars().count() as f32 * GRAPH_EDGE_LABEL_CHAR_WIDTH,
        GRAPH_EDGE_LABEL_HEIGHT,
    );
    let mut offsets = vec![Vec2::ZERO];
    for distance in [18.0, 36.0, 54.0, 72.0, 90.0] {
        offsets.extend([
            Vec2::new(0.0, -distance),
            Vec2::new(0.0, distance),
            Vec2::new(-distance, 0.0),
            Vec2::new(distance, 0.0),
        ]);
    }

    for offset in offsets {
        let candidate = position + offset;
        let label_rect = aligned_label_rect(candidate, size, alignment).expand(3.0);
        if label_rect.left() < canvas.left()
            || label_rect.right() > canvas.right()
            || label_rect.top() < canvas.top()
            || label_rect.bottom() > canvas.bottom()
            || node_rects
                .iter()
                .any(|rect| label_rect.intersects(rect.expand(2.0)))
            || occupied_label_rects
                .iter()
                .any(|rect| label_rect.intersects(*rect))
        {
            continue;
        }
        return Some((candidate, label_rect));
    }
    None
}

fn aligned_label_rect(position: Pos2, size: Vec2, alignment: Align2) -> egui::Rect {
    let min = if alignment == Align2::LEFT_CENTER {
        Pos2::new(position.x, position.y - size.y / 2.0)
    } else if alignment == Align2::RIGHT_CENTER {
        Pos2::new(position.x - size.x, position.y - size.y / 2.0)
    } else {
        position - size / 2.0
    };
    egui::Rect::from_min_size(min, size)
}

fn shorten_to_width(
    painter: &egui::Painter,
    text: &str,
    max_chars: usize,
    font_id: &FontId,
    max_width: f32,
    color: egui::Color32,
) -> String {
    let text = single_line_text(text);
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    let needs_ellipsis = characters.next().is_some();
    let candidate = if needs_ellipsis {
        format!("{prefix}…")
    } else {
        prefix.clone()
    };
    if painter
        .layout_no_wrap(candidate.clone(), font_id.clone(), color)
        .size()
        .x
        <= max_width
    {
        return candidate;
    }

    let ellipsis = "…";
    if painter
        .layout_no_wrap(ellipsis.to_string(), font_id.clone(), color)
        .size()
        .x
        > max_width
    {
        return String::new();
    }

    let mut result = String::new();
    for character in prefix.chars() {
        let next = format!("{result}{character}{ellipsis}");
        if painter
            .layout_no_wrap(next, font_id.clone(), color)
            .size()
            .x
            > max_width
        {
            break;
        }
        result.push(character);
    }
    result.push_str(ellipsis);
    result
}

#[cfg(test)]
pub(super) fn box_border_offset(direction: Vec2) -> f32 {
    (GRAPH_NODE_SIZE.x / 2.0 / direction.x.abs()).min(GRAPH_NODE_SIZE.y / 2.0 / direction.y.abs())
}

fn draw_arrow_head(painter: &egui::Painter, tip: Pos2, direction: Vec2, stroke: Stroke) {
    let arrow_length = 9.0;
    for angle_offset in [2.55, -2.55] {
        let wing = tip + Vec2::angled(direction.angle() + angle_offset) * arrow_length;
        painter.line_segment([tip, wing], stroke);
    }
}
