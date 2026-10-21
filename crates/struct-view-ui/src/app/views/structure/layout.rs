use std::collections::{HashMap, HashSet};

use egui::{Pos2, Rect, Vec2};
use struct_view_core::structure::Document;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Direction {
    Vertical,
    Horizontal,
    #[default]
    Compact,
}

#[derive(Default)]
pub(super) struct Layout {
    pub(super) nodes: Vec<(usize, Rect)>,
    pub(super) edges: Vec<(usize, usize, Vec<Pos2>)>,
    pub(super) size: Vec2,
}

pub(super) fn build(
    doc: &Document,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
    direction: Direction,
) -> Layout {
    let size = if direction == Direction::Compact {
        Vec2::new(200.0, 64.0)
    } else {
        Vec2::new(224.0, 72.0)
    };
    let vertical = direction == Direction::Vertical;
    let gap = if direction == Direction::Compact {
        20.0
    } else {
        36.0
    };
    let breadth = if vertical { size.x + gap } else { size.y + gap };
    let depth_step = if vertical {
        size.y + 64.0
    } else {
        size.x + 80.0
    };
    let mut layout = Layout::default();
    if direction == Direction::Compact {
        place_compact(
            0,
            Pos2::new(24.0, 24.0),
            doc,
            collapsed,
            limits,
            &mut layout,
        );
    } else {
        let mut cursor = 0.0;
        place(
            0,
            0,
            doc,
            collapsed,
            limits,
            &mut layout,
            &mut cursor,
            size,
            breadth,
            depth_step,
            vertical,
        );
    }
    let by_id: HashMap<_, _> = layout.nodes.iter().copied().collect();
    for &(id, rect) in &layout.nodes {
        layout.size = layout.size.max(rect.max.to_vec2() + Vec2::splat(24.0));
        if let Some(parent) = doc.nodes[id].parent
            && let Some(parent_rect) = by_id.get(&parent)
        {
            let (start, end) = if vertical {
                (parent_rect.center_bottom(), rect.center_top())
            } else {
                (parent_rect.right_center(), rect.left_center())
            };
            let middle = (start + end.to_vec2()) * 0.5;
            let points = if direction == Direction::Compact
                && visible_children(id, doc, collapsed, limits).is_empty()
            {
                let lane_x = start.x + 40.0;
                let lane_y = rect.top() - 10.0;
                vec![
                    start,
                    Pos2::new(lane_x, start.y),
                    Pos2::new(lane_x, lane_y),
                    Pos2::new(rect.center().x, lane_y),
                    rect.center_top(),
                ]
            } else if vertical {
                vec![
                    start,
                    Pos2::new(start.x, middle.y),
                    Pos2::new(end.x, middle.y),
                    end,
                ]
            } else {
                vec![
                    start,
                    Pos2::new(middle.x, start.y),
                    Pos2::new(middle.x, end.y),
                    end,
                ]
            };
            layout.edges.push((parent, id, points));
        }
    }
    layout
}

fn visible_children<'a>(
    id: usize,
    doc: &'a Document,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
) -> &'a [usize] {
    let children = &doc.nodes[id].children;
    let count = if collapsed.contains(&id) {
        0
    } else {
        children.len().min(*limits.get(&id).unwrap_or(&100))
    };
    &children[..count]
}

fn place_compact(
    id: usize,
    origin: Pos2,
    doc: &Document,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
    layout: &mut Layout,
) -> Vec2 {
    let size = Vec2::new(200.0, 64.0);
    let gap = 20.0;
    let depth_step = size.x + 80.0;
    let slot = layout.nodes.len();
    layout.nodes.push((id, Rect::from_min_size(origin, size)));
    let children = visible_children(id, doc, collapsed, limits);
    if children.is_empty() {
        return size;
    }

    let mut height = 0.0;
    let mut width = size.x;
    let mut index = 0;
    while index < children.len() {
        let child_origin = origin + Vec2::new(depth_step, height);
        let child = children[index];
        let block_size = if visible_children(child, doc, collapsed, limits).is_empty() {
            let count = children[index..]
                .iter()
                .take_while(|&&child| visible_children(child, doc, collapsed, limits).is_empty())
                .count();
            let columns = ((count as f32 * (size.y + gap) / (size.x + gap))
                .sqrt()
                .ceil() as usize)
                .max(1);
            let rows = count.div_ceil(columns);
            for (offset, &child) in children[index..index + count].iter().enumerate() {
                let position = child_origin
                    + Vec2::new(
                        (offset % columns) as f32 * (size.x + gap),
                        (offset / columns) as f32 * (size.y + gap),
                    );
                layout
                    .nodes
                    .push((child, Rect::from_min_size(position, size)));
            }
            index += count;
            Vec2::new(
                columns as f32 * (size.x + gap) - gap,
                rows as f32 * (size.y + gap) - gap,
            )
        } else {
            index += 1;
            place_compact(child, child_origin, doc, collapsed, limits, layout)
        };
        height += block_size.y + gap;
        width = width.max(depth_step + block_size.x);
    }
    height -= gap;
    layout.nodes[slot].1 =
        Rect::from_min_size(origin + Vec2::new(0.0, (height - size.y) * 0.5), size);
    Vec2::new(width, height)
}

#[allow(clippy::too_many_arguments)]
fn place(
    id: usize,
    depth: usize,
    doc: &Document,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
    layout: &mut Layout,
    cursor: &mut f32,
    size: Vec2,
    breadth: f32,
    depth_step: f32,
    vertical: bool,
) -> f32 {
    let slot = layout.nodes.len();
    layout.nodes.push((id, Rect::NOTHING));
    let children = visible_children(id, doc, collapsed, limits);
    let count = children.len();
    let mut first = 0.0;
    let mut last = 0.0;
    for (index, &child) in children.iter().take(count).enumerate() {
        let center = place(
            child,
            depth + 1,
            doc,
            collapsed,
            limits,
            layout,
            cursor,
            size,
            breadth,
            depth_step,
            vertical,
        );
        if index == 0 {
            first = center;
        }
        last = center;
    }
    let center = if count == 0 {
        let center = *cursor;
        *cursor += breadth;
        center
    } else {
        (first + last) * 0.5
    };
    let position = if vertical {
        Pos2::new(center, depth as f32 * depth_step)
    } else {
        Pos2::new(depth as f32 * depth_step, center)
    };
    layout.nodes[slot].1 = Rect::from_min_size(position + Vec2::splat(24.0), size);
    center
}

#[cfg(test)]
mod tests {
    use super::*;
    use struct_view_core::parser::DataFormat;

    #[test]
    fn directions_preserve_tree_edges_and_non_overlapping_nodes() {
        let doc = struct_view_core::structure::parse(
            r#"{"a":[1,2,3],"b":{"x":true}}"#,
            Some(DataFormat::Json),
        )
        .unwrap();
        for direction in [
            Direction::Vertical,
            Direction::Horizontal,
            Direction::Compact,
        ] {
            let layout = build(&doc, &HashSet::new(), &HashMap::new(), direction);
            assert_eq!(layout.edges.len(), doc.nodes.len() - 1);
            for (i, &(id, rect)) in layout.nodes.iter().enumerate() {
                for &(_, other) in &layout.nodes[i + 1..] {
                    assert!(!rect.intersects(other));
                }
                if let Some(parent) = doc.nodes[id].parent {
                    let parent_rect = layout.nodes.iter().find(|(id, _)| *id == parent).unwrap().1;
                    if direction == Direction::Vertical {
                        assert!(rect.min.y > parent_rect.max.y);
                    } else {
                        assert!(rect.min.x > parent_rect.max.x);
                    }
                }
            }
        }
    }

    #[test]
    fn wide_arrays_are_paged_without_losing_count() {
        let text = format!(
            "[{}]",
            (0..2000)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let doc = struct_view_core::structure::parse(&text, Some(DataFormat::Json)).unwrap();
        assert_eq!(doc.nodes.len(), 2001);
        assert_eq!(
            build(
                &doc,
                &HashSet::from([0]),
                &HashMap::new(),
                Direction::Compact
            )
            .nodes
            .len(),
            1
        );
        assert_eq!(
            build(&doc, &HashSet::new(), &HashMap::new(), Direction::Compact)
                .nodes
                .len(),
            101
        );
        assert_eq!(
            build(
                &doc,
                &HashSet::new(),
                &HashMap::from([(0, 2000)]),
                Direction::Compact
            )
            .nodes
            .len(),
            2001
        );
    }

    fn assert_clear_layout(layout: &Layout) {
        for (index, &(_, rect)) in layout.nodes.iter().enumerate() {
            assert!(rect.is_finite());
            assert!(rect.min.x >= 0.0 && rect.min.y >= 0.0);
            assert!(rect.max.x <= layout.size.x && rect.max.y <= layout.size.y);
            for &(_, other) in &layout.nodes[index + 1..] {
                assert!(!rect.intersects(other), "{rect:?} overlaps {other:?}");
            }
        }
        for (_, _, points) in &layout.edges {
            for segment in points.windows(2) {
                let [start, end] = [segment[0], segment[1]];
                assert!(start.x == end.x || start.y == end.y);
                for &(_, rect) in &layout.nodes {
                    let interior = rect.shrink(0.1);
                    assert!(
                        !interior.intersects(Rect::from_two_pos(start, end)),
                        "Edge {start:?} -> {end:?} crosses {rect:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn compact_collapsed_siblings_form_a_balanced_grid_in_source_order() {
        let text = format!(
            "[{}]",
            (0..100)
                .map(|i| format!(r#"{{"value":{i}}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let doc = struct_view_core::structure::parse(&text, Some(DataFormat::Json)).unwrap();
        let collapsed = doc.nodes[0]
            .children
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let compact = build(&doc, &collapsed, &HashMap::new(), Direction::Compact);
        let horizontal = build(&doc, &collapsed, &HashMap::new(), Direction::Horizontal);
        assert_eq!(compact.nodes.len(), 101);
        assert_eq!(compact.edges.len(), 100);
        assert!(compact.size.y < horizontal.size.y * 0.25);
        assert!(compact.size.x / compact.size.y < 2.0);
        assert!(compact.size.y / compact.size.x < 2.0);
        assert_eq!(
            compact
                .nodes
                .iter()
                .skip(1)
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            doc.nodes[0].children
        );
        assert!(compact.nodes[1].1.min.x < compact.nodes[2].1.min.x);
        assert_eq!(compact.nodes[1].1.min.y, compact.nodes[2].1.min.y);
        assert_clear_layout(&compact);
    }

    #[test]
    fn compact_expansion_reserves_space_for_subtrees_and_collapsing_restores_grid() {
        let text = format!(
            "[{}]",
            (0..20)
                .map(|i| format!(r#"{{"items":[{i},true,null],"other":{{"value":{i}}}}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let doc = struct_view_core::structure::parse(&text, Some(DataFormat::Json)).unwrap();
        let mut collapsed = doc.nodes[0]
            .children
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let original = build(&doc, &collapsed, &HashMap::new(), Direction::Compact);
        for index in [0, 9, 19] {
            let id = doc.nodes[0].children[index];
            collapsed.remove(&id);
            let expanded = build(&doc, &collapsed, &HashMap::new(), Direction::Compact);
            assert_eq!(expanded.edges.len(), expanded.nodes.len() - 1);
            for &child in &doc.nodes[id].children {
                assert!(expanded.nodes.iter().any(|(visible, _)| *visible == child));
            }
            assert_clear_layout(&expanded);
        }
        collapsed.extend(doc.nodes[0].children.iter().copied());
        let restored = build(&doc, &collapsed, &HashMap::new(), Direction::Compact);
        assert_eq!(restored.nodes, original.nodes);
        assert_eq!(restored.edges, original.edges);
    }

    #[test]
    fn compact_grid_respects_paging_and_hidden_descendants() {
        let text = format!("[{}]", vec![r#"{"value":1}"#; 150].join(","));
        let doc = struct_view_core::structure::parse(&text, Some(DataFormat::Json)).unwrap();
        let collapsed = doc.nodes[0]
            .children
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        for (limit, expected) in [(0, 0), (1, 1), (10, 10), (100, 100), (200, 150)] {
            let layout = build(
                &doc,
                &collapsed,
                &HashMap::from([(0, limit)]),
                Direction::Compact,
            );
            assert_eq!(layout.nodes.len(), expected + 1);
            assert_eq!(layout.edges.len(), expected);
            assert_clear_layout(&layout);
        }
    }
}
