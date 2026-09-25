use super::*;

/// Отрисовать граф идентификаторов и зависимостей.
pub(in crate::app) fn show_graph(
    ui: &mut egui::Ui,
    graph: &RelationshipGraph,
    search: &SearchState,
    locale: Locale,
) {
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
            let node_count = graph.nodes.len();
            let columns = (node_count as f32).sqrt().ceil() as usize;
            let columns = columns.max(1);
            let rows = node_count.div_ceil(columns);
            let viewport = ui.available_size_before_wrap();
            let canvas_size = Vec2::new(
                viewport.x.max(columns as f32 * GRAPH_STEP.x + 48.0),
                viewport.y.max(rows as f32 * GRAPH_STEP.y + 48.0),
            );
            let (response, painter) = ui.allocate_painter(canvas_size, Sense::hover());
            let canvas = response.rect;
            let positions = (0..node_count)
                .map(|index| {
                    let column = index % columns;
                    let row = index / columns;
                    Pos2::new(
                        canvas.left()
                            + 24.0
                            + column as f32 * GRAPH_STEP.x
                            + GRAPH_NODE_SIZE.x / 2.0,
                        canvas.top() + 24.0 + row as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                    )
                })
                .collect::<Vec<_>>();

            for edge in &graph.edges {
                let start = positions[edge.source];
                let end = positions[edge.target];
                let direction = (end - start).normalized();
                let start_offset = box_border_offset(direction);
                let end_offset = box_border_offset(direction);
                let line_start = start + direction * start_offset;
                let line_end = end - direction * end_offset;
                let stroke = Stroke::new(1.5, COLOR_KEY);
                painter.line_segment([line_start, line_end], stroke);
                draw_arrow_head(&painter, line_end, direction, stroke);
                let label_position = Pos2::new(
                    (line_start.x + line_end.x) / 2.0,
                    (line_start.y + line_end.y) / 2.0 - 8.0,
                );
                painter.text(
                    label_position,
                    Align2::CENTER_CENTER,
                    shorten(&edge.label, 18),
                    FontId::proportional(12.0),
                    COLOR_KEY,
                );
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
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::same(6),
                    ui.visuals().faint_bg_color,
                );
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(6),
                    if active_match {
                        Stroke::new(2.5, COLOR_ACTIVE_MATCH)
                    } else if is_match {
                        Stroke::new(2.0, COLOR_MATCH)
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke
                    },
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    Pos2::new(center.x, center.y - 9.0),
                    Align2::CENTER_CENTER,
                    shorten(&node.label, 24),
                    FontId::proportional(15.0),
                    COLOR_KEY,
                );
                painter.text(
                    Pos2::new(center.x, center.y + 13.0),
                    Align2::CENTER_CENTER,
                    shorten(&node.id, 26),
                    FontId::monospace(11.0),
                    ui.visuals().weak_text_color(),
                );
                ui.interact(
                    rect,
                    ui.make_persistent_id(("graph-node", &node.path)),
                    Sense::hover(),
                )
                .on_hover_text(format!("{}\n{}\n{}", node.label, node.id, node.path));
            }
        });
}

fn shorten(text: &str, max_chars: usize) -> String {
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn box_border_offset(direction: Vec2) -> f32 {
    if direction.x.abs() >= direction.y.abs() {
        GRAPH_NODE_SIZE.x / 2.0
    } else {
        GRAPH_NODE_SIZE.y / 2.0
    }
}

fn draw_arrow_head(painter: &egui::Painter, tip: Pos2, direction: Vec2, stroke: Stroke) {
    let arrow_length = 9.0;
    for angle_offset in [2.55, -2.55] {
        let wing = tip + Vec2::angled(direction.angle() + angle_offset) * arrow_length;
        painter.line_segment([tip, wing], stroke);
    }
}
