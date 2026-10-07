use super::super::*;
use crate::app::views::diagram::view_controls;
use std::collections::HashSet;

#[derive(Clone)]
pub(in crate::app::views::graph) struct GraphInteractionState {
    pub(in crate::app::views::graph) zoom: f32,
    pub(in crate::app::views::graph) selected: HashSet<usize>,
    pub(in crate::app::views::graph) pan: Vec2,
    pub(in crate::app::views::graph) cursor: usize,
    pub(in crate::app::views::graph) active_search: Option<String>,
    pub(in crate::app::views::graph) marquee_start: Option<Pos2>,
    pub(in crate::app::views::graph) marquee_base: HashSet<usize>,
    pub(in crate::app::views::graph) edge_hover: Option<(Pos2, Vec<usize>)>,
    pub(in crate::app::views::graph) edge_hover_pinned: bool,
}

impl Default for GraphInteractionState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            selected: HashSet::new(),
            pan: Vec2::ZERO,
            cursor: 0,
            active_search: None,
            marquee_start: None,
            marquee_base: HashSet::new(),
            edge_hover: None,
            edge_hover_pinned: false,
        }
    }
}

impl GraphInteractionState {
    pub(super) fn select_node(&mut self, index: usize, additive: bool) {
        self.cursor = index;
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

pub(in crate::app) fn graph_view_menu(
    ui: &mut egui::Ui,
    routing: &GraphRoutingLayout,
    locale: Locale,
) {
    let id = egui::Id::new(("graph-interaction", routing.graph_fingerprint));
    let interaction = ui
        .ctx()
        .data(|data| data.get_temp::<GraphInteractionState>(id))
        .unwrap_or_default();
    let actions = view_controls(ui, locale, interaction.zoom);
    ui.label(format!(
        "{}: {}",
        locale.text(TextKey::GraphSelected),
        interaction.selected.len()
    ));
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::new(("graph-menu-actions", routing.graph_fingerprint)),
            actions,
        )
    });
}

pub(super) fn graph_keyboard(
    ui: &egui::Ui,
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    viewport: Vec2,
    interaction: &mut GraphInteractionState,
) {
    let pressed = |key| ui.input(|i| i.key_pressed(key));
    let mut next = interaction.cursor.min(graph.nodes.len().saturating_sub(1));
    let mut moved = false;
    if pressed(egui::Key::ArrowDown) {
        next = (next + 1).min(graph.nodes.len().saturating_sub(1));
        moved = true;
    }
    if pressed(egui::Key::ArrowUp) {
        next = next.saturating_sub(1);
        moved = true;
    }
    if pressed(egui::Key::ArrowLeft) || pressed(egui::Key::ArrowRight) {
        let left = pressed(egui::Key::ArrowLeft);
        let neighbor = graph
            .edges
            .iter()
            .filter_map(|edge| {
                if left && edge.target == next && edge.source != next {
                    Some(edge.source)
                } else if !left && edge.source == next && edge.target != next {
                    Some(edge.target)
                } else {
                    None
                }
            })
            .min()
            .or_else(|| {
                graph
                    .edges
                    .iter()
                    .filter_map(|edge| {
                        if edge.source == next && edge.target != next {
                            Some(edge.target)
                        } else if edge.target == next && edge.source != next {
                            Some(edge.source)
                        } else {
                            None
                        }
                    })
                    .min()
            });
        if let Some(neighbor) = neighbor {
            next = neighbor;
            moved = true;
        }
    }
    if moved {
        interaction.cursor = next;
        interaction.selected = HashSet::from([next]);
        interaction.pan =
            viewport * 0.5 - routing.node_positions[next].to_vec2() * interaction.zoom;
    }
    if crate::app::views::diagram::keyboard_activate(ui) {
        interaction.select_node(next, ui.input(|i| i.modifiers.command || i.modifiers.ctrl));
    }
}
