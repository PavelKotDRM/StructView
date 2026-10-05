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
    pub(super) edges: Vec<(usize, usize, [Pos2; 4])>,
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
            let points = if vertical {
                [
                    start,
                    Pos2::new(start.x, middle.y),
                    Pos2::new(end.x, middle.y),
                    end,
                ]
            } else {
                [
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
    let children = &doc.nodes[id].children;
    let count = if collapsed.contains(&id) {
        0
    } else {
        children.len().min(*limits.get(&id).unwrap_or(&100))
    };
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
}
