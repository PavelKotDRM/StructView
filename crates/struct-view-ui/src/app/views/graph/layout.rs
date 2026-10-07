use super::calculation::{
    GraphProgressTracker, GraphStage, advance_graph_progress, begin_graph_stage,
};
use super::*;

use std::collections::{HashMap, HashSet};

const GRAPH_ROUTING_COMPACT_DEGREE_LIMIT: usize = 8;
const GRAPH_ROUTING_LANE_WIDTH: f32 = routing::GRAPH_EDGE_CLEARANCE + 2.0;
const GRAPH_ROUTING_MAX_LAYOUT_ATTEMPTS: usize = 8;
/// Extra layout passes that widen gaps around routes forced into detours by other edges.
const GRAPH_ROUTING_MAX_STRAIGHTENING_PASSES: usize = 2;

#[cfg(test)]
pub(in crate::app::views) fn build_graph_routing_layout(
    graph: &RelationshipGraph,
) -> GraphRoutingLayout {
    build_graph_routing_layout_with_progress(
        graph,
        GraphRoutingWorkerSetting::Automatic,
        RoutingSearchBackend::Builtin,
        None,
    )
    .expect("test graph must have a valid layout")
}

#[cfg(test)]
pub(super) fn graph_node_positions(graph: &RelationshipGraph) -> Vec<Pos2> {
    let slots = graph_node_slots(graph);
    let gaps = GraphGaps::for_density(graph, &slots);
    slots.positions(&gaps)
}

/// Grid cell of every node: connected nodes get a layer column and a global row.
struct GraphSlots {
    cells: Vec<Option<(usize, usize)>>,
    columns: usize,
    rows: usize,
    isolated: Vec<usize>,
    has_edges: bool,
}

/// Extra space added to individual column and row gaps; index `i` is the gap before
/// column/row `i`, the last entry is the gap after the final column/row.
#[derive(Clone, Debug, PartialEq)]
struct GraphGaps {
    columns: Vec<f32>,
    rows: Vec<f32>,
}

impl GraphGaps {
    fn compact(slots: &GraphSlots) -> Self {
        Self {
            columns: vec![0.0; slots.columns + 1],
            rows: vec![0.0; slots.rows + 1],
        }
    }

    /// Reserves extra lanes only around nodes whose degree exceeds the compact limit.
    fn for_density(graph: &RelationshipGraph, slots: &GraphSlots) -> Self {
        let mut gaps = Self::compact(slots);
        let mut degrees = vec![0_usize; graph.nodes.len()];
        for edge in &graph.edges {
            degrees[edge.source] += 1;
            degrees[edge.target] += 1;
        }
        for (node, degree) in degrees.into_iter().enumerate() {
            let extra = degree.saturating_sub(GRAPH_ROUTING_COMPACT_DEGREE_LIMIT) as f32
                * GRAPH_ROUTING_LANE_WIDTH;
            if let (Some((column, row)), true) = (slots.cells[node], extra > 0.0) {
                for gap in &mut gaps.columns[column..=column + 1] {
                    *gap = gap.max(extra);
                }
                for gap in &mut gaps.rows[row..=row + 1] {
                    *gap = gap.max(extra);
                }
            }
        }
        gaps
    }

    /// Widens only the gaps next to the endpoints of crowded edges. Edges that stay crowded
    /// after a previous widening also widen every gap between their endpoints.
    fn widen_around(
        &mut self,
        slots: &GraphSlots,
        crowded: &[(usize, usize)],
        previously_crowded: &HashSet<(usize, usize)>,
    ) {
        let increment = (GRAPH_STEP - GRAPH_NODE_SIZE) * 0.5;
        let mut columns = HashSet::new();
        let mut rows = HashSet::new();
        for &(source, target) in crowded {
            let (Some(from), Some(to)) = (slots.cells[source], slots.cells[target]) else {
                continue;
            };
            for (column, row) in [from, to] {
                columns.extend([column, column + 1]);
                rows.extend([row, row + 1]);
            }
            if previously_crowded.contains(&(source, target)) {
                columns.extend(from.0.min(to.0)..=from.0.max(to.0) + 1);
                rows.extend(from.1.min(to.1)..=from.1.max(to.1) + 1);
            }
        }
        for column in columns {
            self.columns[column] += increment.x;
        }
        for row in rows {
            self.rows[row] += increment.y;
        }
    }
}

impl GraphSlots {
    fn positions(&self, gaps: &GraphGaps) -> Vec<Pos2> {
        let offsets = |extra: &[f32], step: f32, count: usize| {
            let mut offset = 24.0 + extra[0] / 2.0;
            (0..count)
                .map(|index| {
                    if index > 0 {
                        offset += step + extra[index];
                    }
                    offset
                })
                .collect::<Vec<_>>()
        };
        let xs = offsets(&gaps.columns, GRAPH_STEP.x, self.columns);
        let ys = offsets(&gaps.rows, GRAPH_STEP.y, self.rows);
        let mut positions = vec![Pos2::ZERO; self.cells.len()];
        for (position, cell) in positions.iter_mut().zip(&self.cells) {
            if let Some((column, row)) = *cell {
                *position = Pos2::new(xs[column], ys[row]) + GRAPH_NODE_SIZE / 2.0;
            }
        }
        if !self.isolated.is_empty() {
            let columns = (self.isolated.len() as f32).sqrt().ceil() as usize;
            let isolated_start_x = if self.has_edges {
                xs.last().copied().unwrap_or(24.0)
                    + GRAPH_NODE_SIZE.x / 2.0
                    + GRAPH_STEP.x * 2.0
                    + gaps.columns.last().copied().unwrap_or(0.0)
            } else {
                24.0
            };
            for (index, &node) in self.isolated.iter().enumerate() {
                let column = index % columns;
                let row = index / columns;
                positions[node] = Pos2::new(
                    isolated_start_x + column as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0,
                    24.0 + row as f32 * GRAPH_STEP.y + GRAPH_NODE_SIZE.y / 2.0,
                );
            }
        }
        positions
    }
}

fn graph_node_slots(graph: &RelationshipGraph) -> GraphSlots {
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
    let mut isolated = Vec::new();
    if partitioned.is_none() {
        groups.retain(|group| {
            if group.len() == 1 && neighbors[group[0]].is_empty() {
                isolated.push(group[0]);
                false
            } else {
                true
            }
        });
    }
    let mut cells = vec![None; count];
    let mut column_count = 0;
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
                cells[node] = Some((column, row_offset + row));
            }
        }
        column_count = column_count.max(columns);
        row_offset += height + 1;
    }
    GraphSlots {
        cells,
        columns: column_count,
        rows: row_offset,
        isolated,
        has_edges: !graph.edges.is_empty(),
    }
}

struct GraphRoutePlacement {
    positions: Vec<Pos2>,
    routes: Vec<Vec<Pos2>>,
}

fn route_graph_with_spacing(
    graph: &RelationshipGraph,
    routed_nodes: &[usize],
    edge_endpoints: &[(usize, usize)],
    worker_count: usize,
    search_backend: RoutingSearchBackend,
    progress: Option<&GraphProgressTracker>,
) -> Result<GraphRoutePlacement, String> {
    let slots = graph_node_slots(graph);
    let mut gaps = GraphGaps::for_density(graph, &slots);
    let mut previously_crowded = HashSet::new();
    let mut straightening_passes = 0;
    let mut routed = None;
    for attempt in 1..=GRAPH_ROUTING_MAX_LAYOUT_ATTEMPTS {
        begin_graph_stage(progress, GraphStage::Layout, 0, 1);
        let positions = slots.positions(&gaps);
        let mut routing_grid =
            GraphRoutingGrid::new_for_graph(&positions, routed_nodes, search_backend);
        let edge_ports = graph_edge_ports(&positions, edge_endpoints);
        routing_grid.reserve_port_leads(edge_endpoints, &edge_ports);
        match route_graph_edges_with_progress(
            &routing_grid,
            edge_endpoints,
            &edge_ports,
            worker_count,
            progress,
        ) {
            Ok(routes) => {
                let detoured = if straightening_passes < GRAPH_ROUTING_MAX_STRAIGHTENING_PASSES
                    && attempt < GRAPH_ROUTING_MAX_LAYOUT_ATTEMPTS
                {
                    GraphRoutingGrid::new_for_graph(&positions, routed_nodes, search_backend)
                        .detoured_edges(edge_endpoints, &edge_ports, &routes)
                } else {
                    Vec::new()
                };
                let placement = GraphRoutePlacement { positions, routes };
                if detoured.is_empty() {
                    return Ok(placement);
                }
                straightening_passes += 1;
                gaps.widen_around(&slots, &detoured, &previously_crowded);
                previously_crowded.extend(detoured);
                routed = Some(placement);
            }
            // Keep the routed layout if a wider one unexpectedly fails.
            Err(_) if routed.is_some() => break,
            Err(error)
                if error.needs_more_space() && attempt < GRAPH_ROUTING_MAX_LAYOUT_ATTEMPTS =>
            {
                let crowded = error.crowded_edges();
                eprintln!(
                    "Graph routing needs more space (attempt {attempt}): {error}; \
                     widening gaps around {} crowded edge(s)",
                    crowded.len()
                );
                gaps.widen_around(&slots, crowded, &previously_crowded);
                previously_crowded.extend(crowded.iter().copied());
            }
            Err(error) => return Err(format!("{error} (after {attempt} layout attempt(s))")),
        }
    }
    routed.ok_or_else(|| "graph routing ran out of layout attempts".to_owned())
}

pub(super) fn build_graph_routing_layout_with_progress(
    graph: &RelationshipGraph,
    worker_setting: GraphRoutingWorkerSetting,
    search_backend: RoutingSearchBackend,
    progress: Option<&GraphProgressTracker>,
) -> Result<GraphRoutingLayout, String> {
    let routed_nodes = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, _node)| {
            graph
                .edges
                .iter()
                .any(|edge| edge.source == index || edge.target == index)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let edge_endpoints = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect::<Vec<_>>();
    let worker_count = graph_routing_worker_count(graph.edges.len(), worker_setting);
    let GraphRoutePlacement {
        positions: mut node_positions,
        routes: routed_edges,
    } = route_graph_with_spacing(
        graph,
        &routed_nodes,
        &edge_endpoints,
        worker_count,
        search_backend,
        progress,
    )?;
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
    for point in routed_edges.iter().flatten() {
        content_size.x = content_size.x.max(point.x + 24.0);
        content_size.y = content_size.y.max(point.y + 24.0);
    }
    let connected_node_rects = routed_nodes
        .iter()
        .map(|&index| egui::Rect::from_center_size(node_positions[index], GRAPH_NODE_SIZE))
        .collect::<Vec<_>>();
    let mut label_content_size = routed_nodes.iter().fold(Vec2::splat(48.0), |size, &index| {
        let position = node_positions[index];
        Vec2::new(
            size.x.max(position.x + GRAPH_NODE_SIZE.x / 2.0 + 24.0),
            size.y.max(position.y + GRAPH_NODE_SIZE.y / 2.0 + 24.0),
        )
    });
    for point in routed_edges.iter().flatten() {
        label_content_size.x = label_content_size.x.max(point.x + 24.0);
        label_content_size.y = label_content_size.y.max(point.y + 24.0);
    }
    let label_canvas = egui::Rect::from_min_size(Pos2::ZERO, label_content_size);
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
                label_canvas,
                &connected_node_rects,
                &occupied_label_rects,
                &routed_edges,
            )
            .inspect(|label| {
                occupied_label_rects.push(label.background);
                content_size.x = content_size.x.max(label.background.right() + 24.0);
                content_size.y = content_size.y.max(label.background.bottom() + 24.0);
            });
            advance_graph_progress(progress);
            label
        })
        .collect::<Vec<_>>();
    let callout_indices = edge_labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            (label.is_none() && !graph.edges[index].label.trim().is_empty()).then_some(index)
        })
        .collect::<Vec<_>>();
    let row_height = GRAPH_EDGE_LABEL_HEIGHT + 9.0;
    let connected_bottom = connected_node_rects
        .iter()
        .map(egui::Rect::bottom)
        .chain(routed_edges.iter().flatten().map(|point| point.y))
        .fold(24.0_f32, f32::max)
        + 24.0;
    let max_rows = ((connected_bottom - 48.0) / row_height).floor().max(1.0) as usize;
    let column_count = callout_indices.len().div_ceil(max_rows);
    for (callout_index, &edge_index) in callout_indices.iter().enumerate() {
        let row = callout_index / column_count;
        let column = callout_index % column_count;
        let position = Pos2::new(
            label_canvas.right() + 24.0 + column as f32 * 156.0,
            label_canvas.top() + 24.0 + row as f32 * row_height + GRAPH_EDGE_LABEL_HEIGHT / 2.0,
        );
        let label = graph_edge_label_callout_at(
            &graph.edges[edge_index].label,
            &routed_edges[edge_index],
            position,
        )
        .inspect(|label| {
            content_size.x = content_size.x.max(label.background.right() + 24.0);
            content_size.y = content_size.y.max(label.background.bottom() + 24.0);
        });
        edge_labels[edge_index] = label;
    }
    resolve_graph_label_leaders(&mut edge_labels, &connected_node_rects, &routed_edges);
    for (_, anchor) in edge_labels
        .iter()
        .flatten()
        .filter_map(|label| label.reference)
    {
        content_size.x = content_size.x.max(anchor.x + 24.0);
        content_size.y = content_size.y.max(anchor.y + 24.0);
    }
    let label_right = edge_labels
        .iter()
        .flatten()
        .map(|label| label.background.right())
        .max_by(f32::total_cmp);
    if let Some(label_right) = label_right {
        let mut is_routed = vec![false; graph.nodes.len()];
        for &index in &routed_nodes {
            is_routed[index] = true;
        }
        let isolated_left = node_positions
            .iter()
            .enumerate()
            .filter_map(|(index, position)| (!is_routed[index]).then_some(position.x))
            .min_by(f32::total_cmp);
        if let Some(isolated_left) = isolated_left {
            let shift = (label_right + GRAPH_STEP.x + 0.1 - isolated_left).max(0.0);
            if shift > 0.0 {
                for (index, position) in node_positions.iter_mut().enumerate() {
                    if !is_routed[index] {
                        position.x += shift;
                        content_size.x = content_size
                            .x
                            .max(position.x + GRAPH_STEP.x - GRAPH_NODE_SIZE.x / 2.0 + 24.0);
                    }
                }
            }
        }
    }

    Ok(GraphRoutingLayout {
        graph_fingerprint: relationship_graph_fingerprint(graph),
        node_positions,
        edge_paths: routed_edges,
        edge_labels,
        partition_labels: graph.partition_names.clone(),
        content_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Full large-fixture layout; run with --release --ignored --nocapture"]
    fn large_routing_fixture_builds_a_complete_layout() {
        let input = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/large-routing-graph.json"
        ))
        .unwrap();
        let root = struct_view_core::parser::parse_json(&input).unwrap();
        let graph = build_relationship_graph(&root);
        assert!(graph.edges.len() > 1000, "{}", graph.edges.len());
        let started = std::time::Instant::now();
        let layout = build_graph_routing_layout_with_progress(
            &graph,
            GraphRoutingWorkerSetting::Automatic,
            RoutingSearchBackend::Builtin,
            None,
        )
        .unwrap();
        eprintln!("large fixture layout: {:?}", started.elapsed());
        assert_eq!(layout.edge_paths.len(), graph.edges.len());
        let endpoints = graph
            .edges
            .iter()
            .map(|edge| (edge.source, edge.target))
            .collect::<Vec<_>>();
        let routed_nodes = (0..graph.nodes.len())
            .filter(|&node| endpoints.iter().any(|&(s, t)| s == node || t == node))
            .collect::<Vec<_>>();
        let ports = graph_edge_ports(&layout.node_positions, &endpoints);
        let detoured = GraphRoutingGrid::new_for_graph(
            &layout.node_positions,
            &routed_nodes,
            RoutingSearchBackend::Builtin,
        )
        .detoured_edges(&endpoints, &ports, &layout.edge_paths);
        assert!(detoured.is_empty(), "{detoured:?}");
    }

    #[test]
    fn detoured_routes_get_more_space() {
        let nodes = (0..6)
            .map(|id| serde_json::json!({"id": id.to_string()}))
            .collect::<Vec<_>>();
        let edges = (0..6)
            .flat_map(|source| {
                (0..6).map(move |target| {
                    serde_json::json!({"source": source.to_string(), "target": target.to_string()})
                })
            })
            .collect::<Vec<_>>();
        let root = struct_view_core::parser::parse_json(
            &serde_json::json!({"nodes": nodes, "edges": edges}).to_string(),
        )
        .unwrap();
        let graph = build_relationship_graph(&root);
        let routed_nodes = (0..graph.nodes.len()).collect::<Vec<_>>();
        let endpoints = graph
            .edges
            .iter()
            .map(|edge| (edge.source, edge.target))
            .collect::<Vec<_>>();
        let detoured = |positions: &[Pos2], routes: &[Vec<Pos2>]| {
            GraphRoutingGrid::new_for_graph(positions, &routed_nodes, RoutingSearchBackend::Builtin)
                .detoured_edges(&endpoints, &graph_edge_ports(positions, &endpoints), routes)
                .len()
        };
        let slots = graph_node_slots(&graph);
        let compact = slots.positions(&GraphGaps::for_density(&graph, &slots));
        let mut grid =
            GraphRoutingGrid::new_for_graph(&compact, &routed_nodes, RoutingSearchBackend::Builtin);
        let ports = graph_edge_ports(&compact, &endpoints);
        grid.reserve_port_leads(&endpoints, &ports);
        let compact_routes =
            route_graph_edges_with_progress(&grid, &endpoints, &ports, 1, None).unwrap();
        let placement = route_graph_with_spacing(
            &graph,
            &routed_nodes,
            &endpoints,
            1,
            RoutingSearchBackend::Builtin,
            None,
        )
        .unwrap();
        let before = detoured(&compact, &compact_routes);
        let after = detoured(&placement.positions, &placement.routes);
        eprintln!("detoured before {before} after {after}");
        assert!(before > 0 && after < before, "{before} -> {after}");
    }

    fn dense_pair_with_sparse_chain(parallel_links: usize) -> RelationshipGraph {
        let mut edges = (0..parallel_links)
            .map(|_| serde_json::json!({"source": "a", "target": "b"}))
            .collect::<Vec<_>>();
        edges.push(serde_json::json!({"source": "c", "target": "d"}));
        let input = serde_json::json!({
            "graph": {"type": "directed_multigraph"},
            "nodes": [{"id": "a"}, {"id": "b"}, {"id": "c"}, {"id": "d"}],
            "edges": edges,
        })
        .to_string();
        build_relationship_graph(&struct_view_core::parser::parse_json(&input).unwrap())
    }

    #[test]
    fn density_spacing_only_widens_gaps_around_busy_nodes() {
        for (parallel_links, extra_lanes) in [(1, 0), (8, 0), (9, 1), (20, 12)] {
            let graph = dense_pair_with_sparse_chain(parallel_links);
            let slots = graph_node_slots(&graph);
            let gaps = GraphGaps::for_density(&graph, &slots);
            let extra = extra_lanes as f32 * 10.0;
            let (_, dense_row) = slots.cells[0].unwrap();
            let (_, sparse_row) = slots.cells[2].unwrap();
            assert_eq!(gaps.rows[dense_row], extra);
            assert_eq!(gaps.rows[dense_row + 1], extra);
            assert_eq!(gaps.rows[sparse_row + 1], 0.0, "{gaps:?}");
            let positions = slots.positions(&gaps);
            assert_eq!(positions[3].y - positions[2].y, 0.0);
            assert_eq!(positions[1].x - positions[0].x, GRAPH_STEP.x + extra);
        }
    }

    #[test]
    fn crowded_edges_widen_only_nearby_gaps() {
        let graph = dense_pair_with_sparse_chain(1);
        let slots = graph_node_slots(&graph);
        let compact = GraphGaps::compact(&slots);
        let mut gaps = compact.clone();
        gaps.widen_around(&slots, &[(0, 1)], &HashSet::new());
        let (_, dense_row) = slots.cells[0].unwrap();
        let (_, sparse_row) = slots.cells[2].unwrap();
        assert!(gaps.rows[dense_row] > 0.0 && gaps.rows[dense_row + 1] > 0.0);
        assert_eq!(gaps.rows[sparse_row + 1], 0.0);
        let before = slots.positions(&compact);
        let after = slots.positions(&gaps);
        assert!(after[1].x - after[0].x > before[1].x - before[0].x);
        assert_eq!(after[3].y - after[2].y, before[3].y - before[2].y);
    }

    #[test]
    fn failed_crowded_layout_is_rebuilt_with_larger_gaps() {
        // A complete graph this size is too crowded for the compact layout.
        const N: usize = 11;
        let nodes = (0..N)
            .map(|id| serde_json::json!({"id": id.to_string()}))
            .collect::<Vec<_>>();
        let edges = (0..N)
            .flat_map(|source| {
                (0..N).map(move |target| {
                    serde_json::json!({
                        "source": source.to_string(), "target": target.to_string()
                    })
                })
            })
            .collect::<Vec<_>>();
        let root = struct_view_core::parser::parse_json(
            &serde_json::json!({"nodes": nodes, "edges": edges}).to_string(),
        )
        .unwrap();
        let graph = build_relationship_graph(&root);
        let routed_nodes = (0..graph.nodes.len()).collect::<Vec<_>>();
        let endpoints = graph
            .edges
            .iter()
            .map(|edge| (edge.source, edge.target))
            .collect::<Vec<_>>();
        let slots = graph_node_slots(&graph);
        let original_positions = slots.positions(&GraphGaps::compact(&slots));
        let grid = GraphRoutingGrid::new_for_graph(
            &original_positions,
            &routed_nodes,
            RoutingSearchBackend::Builtin,
        );
        let ports = graph_edge_ports(&original_positions, &endpoints);
        let error =
            route_graph_edges_with_progress(&grid, &endpoints, &ports, 4, None).unwrap_err();
        assert!(error.needs_more_space(), "{error}");

        let placement = route_graph_with_spacing(
            &graph,
            &routed_nodes,
            &endpoints,
            4,
            RoutingSearchBackend::Builtin,
            None,
        )
        .unwrap();
        assert_eq!(placement.routes.len(), endpoints.len());
        let old_span = original_positions.last().unwrap().y - original_positions[0].y;
        let new_span = placement.positions.last().unwrap().y - placement.positions[0].y;
        assert!(new_span > old_span, "{new_span} <= {old_span}");
        let mut index =
            struct_view_routing::orthogonal::RouteIndex::new(&[], routing::GRAPH_EDGE_CLEARANCE)
                .unwrap();
        for route in &placement.routes {
            let points = route
                .iter()
                .map(|point| struct_view_routing::orthogonal::Point::new(point.x, point.y))
                .collect::<Vec<_>>();
            assert!(index.first_overlapping_segment(&points, 0.0).is_none());
            index.insert_route(&points).unwrap();
            for &position in &placement.positions {
                let rect = egui::Rect::from_center_size(position, GRAPH_NODE_SIZE);
                assert!(route.windows(2).all(|pair| {
                    !routing::segment_crosses_rect_interior(pair[0], pair[1], rect)
                }));
            }
        }
    }
}
