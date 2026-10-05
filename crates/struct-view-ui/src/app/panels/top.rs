use super::*;

mod menus;
mod search;
use crate::app::docking::{DetachedViewportAction, show_detached_viewport, show_docking_controls};
use struct_view_core::search::escape_regex_literal;

const COMPACT_MENU_WIDTH: f32 = 520.0;

impl StructViewApp {
    /// Отрисовать верхнюю панель с адаптивным меню и поиском.
    pub(in crate::app) fn show_top_panel(&mut self, ui: &mut Ui) {
        self.handle_shortcuts(ui.ctx());
        self.show_search_window(ui.ctx());
        let locale = self.locale;
        let search_shortcut = if cfg!(target_os = "macos") {
            "Cmd+F"
        } else {
            "Ctrl+F"
        };
        egui::Panel::top("top_panel").show(ui, |ui| {
            let compact_menu = ui.available_width() < COMPACT_MENU_WIDTH;
            egui::ScrollArea::horizontal()
                .id_salt("top_panel_controls_scroll")
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.show_menu_bar(ui, compact_menu);
                        if !self.is_comparing() {
                            ui.separator();
                            if ui
                                .button("🔍")
                                .on_hover_text(format!(
                                    "{} ({})",
                                    locale.text(TextKey::SearchWindow),
                                    search_shortcut
                                ))
                                .clicked()
                            {
                                self.search_window_open = true;
                            }
                        }
                    });
                });
        });
    }

    /// Обработать горячие клавиши команд редактирования и работы со структурами.
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.visualization == VisualizationMode::Structure {
            if !ctx.egui_wants_keyboard_input()
                && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F))
            {
                self.search_window_open = true;
            }
            return;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let (copy, paste, undo, redo, delete, find) = ctx.input(|input| {
            let command = input.modifiers.command;
            (
                command && input.key_pressed(egui::Key::C),
                command && input.key_pressed(egui::Key::V),
                command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
                command
                    && (input.key_pressed(egui::Key::Y)
                        || (input.modifiers.shift && input.key_pressed(egui::Key::Z))),
                input.key_pressed(egui::Key::Delete),
                command && input.key_pressed(egui::Key::F),
            )
        });
        self.search_window_open |= find;
        self.copy_structures_requested |= copy;
        self.paste_requested |= paste;
        self.undo_requested |= undo && self.can_undo();
        self.redo_requested |= redo && self.can_redo();
        self.delete_requested |= delete && self.can_delete_selected();
    }

    /// Развернуть или свернуть все узлы дерева.
    pub(super) fn set_all_expanded(&mut self, expanded: bool) {
        if self.visualization == VisualizationMode::Structure {
            self.structure_view.set_all_expanded(expanded);
            return;
        }
        if let Some(root) = &mut self.root {
            set_expanded_all(root, expanded);
            self.visible_rows_dirty = true;
        }
    }
}
