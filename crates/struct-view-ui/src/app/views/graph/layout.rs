use super::*;

use std::collections::HashMap;

#[cfg(test)]
pub(in crate::app::views) fn build_graph_routing_layout(
    graph: &RelationshipGraph,
) -> GraphRoutingLayout {
    build_graph_routing_layout_with_progress(graph, GraphRoutingWorkerSetting::Automatic, None)
}

pub(super) fn graph_node_positions(graph: &RelationshipGraph) -> Vec<Pos2> {
    let count = graph.nodes.len();
    let mut outgoing = vec![Vec::new(); count];
    let mut incoming = vec![Vec::new(); count];
    let mut neighbors = vec![Vec::new(); count];
    for edge in &graph.edges {
        if edge.direction != EdgeDirection::Reverse {
            outgoing[edge.source].push(edge.target);
            incoming[edge.target].push(edge.source);
        }
        if edge.direction != EdgeDirection::Directed {
            outgoing[edge.target].push(edge.source);
            incoming[edge.source].push(edge.target);
        }
        neighbors[edge.source].push(edge.target);
        neighbors[edge.target].push(edge.source);
    }
    for list in outgoing
        .iter_mut()
        .chain(incoming.iter_mut())
        .chain(neighbors.iter_mut())
    {
        list.sort_unstable();
        list.dedup();
    }

    // Collapse directed cycles before assigning left-to-right layers (iterative Kosaraju).
    let mut visited = vec![false; count];
    let mut order = Vec::with_capacity(count);
    for start in 0..count {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if *next < outgoing[*node].len() {
                let target = outgoing[*node][*next];
                *next += 1;
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    let mut component = vec![usize::MAX; count];
    let mut component_count = 0;
    for &start in order.iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        component[start] = component_count;
        while let Some(node) = stack.pop() {
            for &target in &incoming[node] {
                if component[target] == usize::MAX {
                    component[target] = component_count;
                    stack.push(target);
                }
            }
        }
        component_count += 1;
    }
    let mut component_layers = vec![0; component_count];
    // Kosaraju numbers components in topological order.
    let mut component_edges = vec![Vec::new(); component_count];
    for (node, targets) in outgoing.iter().enumerate() {
        for &target in targets {
            let source = component[node];
            let target = component[target];
            if source != target {
                component_edges[source].push(target);
            }
        }
    }
    for source in 0..component_count {
        for &target in &component_edges[source] {
            component_layers[target] = component_layers[target].max(component_layers[source] + 1);
        }
    }
    let mut layers = component
        .iter()
        .map(|&index| component_layers[index])
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    visited.fill(false);
    let all_undirected = graph
        .edges
        .iter()
        .all(|edge| edge.direction == EdgeDirection::Undirected);
    for start in 0..count {
        if visited[start] {
            continue;
        }
        let mut group = Vec::new();
        let mut queue = std::collections::VecDeque::from([start]);
        visited[start] = true;
        if all_undirected {
            layers[start] = 0;
        }
        while let Some(node) = queue.pop_front() {
            group.push(node);
            for &target in &neighbors[node] {
                if !visited[target] {
                    visited[target] = true;
                    if all_undirected {
                        layers[target] = layers[node] + 1;
                    }
                    queue.push_back(target);
                }
            }
        }
        group.sort_unstable();
        groups.push(group);
    }
    let partitioned = graph
        .partition_names
        .as_ref()
        .filter(|names| !names.is_empty());
    if let Some(names) = partitioned {
        for (index, node) in graph.nodes.iter().enumerate() {
            layers[index] = node.partition.unwrap_or(0).min(names.len() - 1);
        }
    }
    let mut positions = vec![Pos2::ZERO; count];
    let mut row_offset = 0;
    let mut row_indices = vec![0; count];
    for group in groups {
        let columns = group.iter().map(|&node| layers[node]).max().unwrap_or(0) + 1;
        let mut rows = vec![Vec::new(); columns];
        for node in group {
            rows[layers[node]].push(node);
        }
        for column in &rows {
            for (row, &node) in column.iter().enumerate() {
                row_indices[node] = row;
            }
        }
        // Alternating barycenter sweeps reduce crossings without changing layer membership.
        for sweep in 0..4 {
            let column_order: Vec<_> = if sweep % 2 == 0 {
                (0..columns).collect()
            } else {
                (0..columns).rev().collect()
            };
            for column in column_order {
                let mut scores = HashMap::new();
                for &node in &rows[column] {
                    let mut sum = 0.0;
                    let mut adjacent_count = 0;
                    for &neighbor in &neighbors[node] {
                        let adjacent = if sweep % 2 == 0 {
                            layers[neighbor] < column
                        } else {
                            layers[neighbor] > column
                        };
                        if adjacent {
                            sum += row_indices[neighbor] as f32;
                            adjacent_count += 1;
                        }
                    }
                    scores.insert(
                        node,
                        if adjacent_count == 0 {
                            row_indices[node] as f32
                        } else {
                            sum / adjacent_count as f32
                        },
                    );
                }
                rows[column].sort_by(|&left, &right| {
                    scores[&left]
                        .total_cmp(&scores[&right])
                        .then(left.cmp(&right))
                });
                for (row, &node) in rows[column].iter().enumerate() {
                    row_indices[node] = row;
                }
            }
        }
        let height = rows.iter().map(Vec::len).max().unwrap_or(0);
        for (column, nodes) in rows.iter().enumerate() {
            for (row, &node) in nodes.iter().enumerate() {
                positions[node] = Pos2::new(
                    24.0 + column as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                    24.0 + (row_offset + row) as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                );
            }
        }
        row_offset += height + 1;
    }
    positions
}

pub(super) fn build_graph_routing_layout_with_progress(
    graph: &RelationshipGraph,
    worker_setting: GraphRoutingWorkerSetting,
    progress: Option<&GraphProgressTracker>,
) -> GraphRoutingLayout {
    begin_graph_stage(progress, GraphStage::Layout, 0, 1);
    let node_positions = graph_node_positions(graph);
    let mut content_size = node_positions
        .iter()
        .fold(Vec2::splat(48.0), |size, point| {
            Vec2::new(
                size.x
                    .max(point.x + GRAPH_STEP.x - GRAPH_NODE_SIZE.x / 2.0 + 24.0),
                size.y
                    .max(point.y + GRAPH_STEP.y - GRAPH_NODE_SIZE.y / 2.0 + 24.0),
            )
        });
    let routing_grid = GraphRoutingGrid::new(&node_positions);
    let edge_endpoints = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect::<Vec<_>>();
    let edge_ports = graph_edge_ports(&node_positions, &edge_endpoints);
    let routed_edges = route_graph_edges_with_progress(
        &routing_grid,
        &edge_endpoints,
        &edge_ports,
        graph_routing_worker_count(graph.edges.len(), worker_setting),
        progress,
    );
    for point in routed_edges.iter().flatten() {
        content_size.x = content_size.x.max(point.x + 24.0);
        content_size.y = content_size.y.max(point.y + 24.0);
    }
    let node_rects = node_positions
        .iter()
        .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
        .collect::<Vec<_>>();
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, content_size);
    let mut occupied_label_rects = Vec::with_capacity(graph.edges.len());
    begin_graph_stage(progress, GraphStage::Labels, graph.edges.len(), 1);
    let mut edge_labels = graph
        .edges
        .iter()
        .zip(&routed_edges)
        .map(|(edge, points)| {
            let label = layout_graph_edge_label(
                &edge.label,
                points,
                canvas,
                &node_rects,
                &occupied_label_rects,
                &routed_edges,
            )
            .or_else(|| {
                graph_edge_label_callout(&edge.label, points, canvas, &occupied_label_rects)
            })
            .inspect(|label| {
                occupied_label_rects.push(label.background);
                content_size.x = content_size.x.max(label.background.right() + 24.0);
                content_size.y = content_size.y.max(label.background.bottom() + 24.0);
            });
            advance_graph_progress(progress);
            label
        })
        .collect::<Vec<_>>();
    resolve_graph_label_leaders(&mut edge_labels, &node_rects, &routed_edges);
    for (_, anchor) in edge_labels
        .iter()
        .flatten()
        .filter_map(|label| label.reference)
    {
        content_size.x = content_size.x.max(anchor.x + 24.0);
        content_size.y = content_size.y.max(anchor.y + 24.0);
    }

    GraphRoutingLayout {
        graph_fingerprint: relationship_graph_fingerprint(graph),
        node_positions,
        edge_paths: routed_edges,
        edge_labels,
        partition_labels: graph.partition_names.clone(),
        content_size,
    }
}
