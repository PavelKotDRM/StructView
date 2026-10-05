use super::*;

impl StructViewApp {
    /// Отрисовать центральную панель с деревом JSON.
    pub(in crate::app) fn show_central_panel(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            let colors = SyntaxColors::new(ui.visuals());
            if self.visualization == VisualizationMode::Structure {
                self.finalize_pending_inline_edit();
                self.structure_view
                    .show(ui, self.locale, self.file_state.path.as_deref());
                self.structure_view
                    .sync_search(&mut self.search, &self.search_query_buf);
                return;
            }
            self.handle_dropped_files(ui);
            let save_requested = std::mem::take(&mut self.save_requested);
            let copy_structures_requested = std::mem::take(&mut self.copy_structures_requested);
            let delete_requested = std::mem::take(&mut self.delete_requested);
            let paste_requested = std::mem::take(&mut self.paste_requested);
            let undo_requested = std::mem::take(&mut self.undo_requested);
            let redo_requested = std::mem::take(&mut self.redo_requested);

            if self.comparison.is_some() {
                if self.visualization == VisualizationMode::Diff {
                    if let Some(comparison) = &mut self.comparison {
                        show_diff(ui, comparison, self.locale);
                    }
                } else {
                    self.show_comparison(ui);
                }
                return;
            }

            if self.root.is_none() && self.parse_error.is_none() {
                show_placeholder(ui, self.locale);
                return;
            }

            if let Some(err) = &self.parse_error {
                show_parse_error(ui, &err.to_string(), self.locale);
                return;
            }

            if self.mode != AppMode::Edit
                || self.visualization != VisualizationMode::Tree
                || self.field_dialog.is_some()
            {
                self.finalize_pending_inline_edit();
            }

            let outcome = match self.visualization {
                VisualizationMode::Tree => self.show_tree(ui),
                VisualizationMode::Structure => {
                    unreachable!("Structure mode has its own input and canvas")
                }
                VisualizationMode::Graph => {
                    if let Some(root) = &self.root {
                        self.graph_calculation.ensure_started(
                            root,
                            self.graph_routing_workers,
                            ui.ctx(),
                        );
                    }
                    if let Some(error) = self.graph_calculation.error() {
                        ui.colored_label(
                            colors.error,
                            format!(
                                "{} {error}",
                                self.locale.text(TextKey::BackgroundOperationFailed)
                            ),
                        );
                    } else if let Some(calculation) = self.graph_calculation.result() {
                        show_graph(
                            ui,
                            &calculation.graph,
                            &calculation.routing,
                            &self.search,
                            self.locale,
                        );
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::Spinner::new());
                                ui.label(self.locale.text(TextKey::GraphCalculating));
                                self.graph_calculation.show_activity(ui, self.locale);
                            });
                        });
                    }
                    TreeOutcome::default()
                }
                VisualizationMode::Table => {
                    if self.visualization_cache.table.is_none()
                        && let Some(root) = &self.root
                    {
                        self.visualization_cache.table = Some(build_table(root));
                    }
                    let export_requested = self
                        .visualization_cache
                        .table
                        .as_ref()
                        .is_some_and(|table| show_table(ui, table, &self.search, self.locale));
                    if export_requested {
                        self.save_table_csv();
                    }
                    TreeOutcome::default()
                }
                VisualizationMode::Schema => {
                    if self.visualization_cache.schema.is_none()
                        && let Some(root) = &self.root
                    {
                        self.visualization_cache.schema = Some(build_schema_diagram(root));
                    }
                    match self.visualization_cache.schema.as_ref() {
                        Some(Ok(diagram)) => {
                            show_schema(ui, diagram, &self.search, self.locale);
                        }
                        Some(Err(error)) => {
                            ui.colored_label(colors.error, error);
                        }
                        None => {}
                    }
                    TreeOutcome::default()
                }
                VisualizationMode::Comparison | VisualizationMode::Diff => self.show_tree(ui),
            };

            let inline_edit_restored = self.handle_inline_edit_events(outcome.inline_edit_events);
            if let Some(request) = outcome.selection_request {
                self.apply_selection_request(request);
            }
            if let Some(request) = outcome.add_child_request {
                self.open_add_child_dialog(request);
            }
            if let Some(request) = outcome.edit_field_request {
                self.open_edit_field_dialog(request);
            }
            if outcome.tree_changed || inline_edit_restored {
                self.invalidate_visualization_cache();
                self.refresh_search();
            }
            if let Some(error) = outcome.edit_error {
                self.show_error(&error);
            }
            if let Some(text) = outcome.copy_request {
                self.clipboard_payload = None;
                match copy_to_clipboard(&text) {
                    Ok(()) => self.show_toast(self.locale.text(TextKey::Copied)),
                    Err(error) => self.show_error(&self.locale.copy_error(&error)),
                }
            }
            if let Some(paths) = outcome.copy_structure_paths {
                self.copy_structures_at_paths(paths);
            }
            if let Some(path) = outcome.paste_target_path {
                self.paste_into_path(path);
            }

            if undo_requested {
                self.undo();
            }
            if redo_requested {
                self.redo();
            }

            if save_requested {
                self.save_current();
            }
            if copy_structures_requested {
                self.copy_structures_at_paths(self.selected_paths.iter().cloned().collect());
            }
            if paste_requested {
                self.paste_into_selected();
            }
            if delete_requested {
                self.delete_selected();
            }
            if self.field_dialog.is_some() {
                self.finalize_pending_inline_edit();
            }
        });
    }

    pub(super) fn show_tree(&mut self, ui: &mut Ui) -> TreeOutcome {
        let scroll_to_path = self.search_scroll_target.take();
        if let Some(path) = scroll_to_path.as_deref()
            && let Some(root) = &mut self.root
        {
            focus_match_path(root, path);
            self.visible_rows_dirty = true;
        }

        if self.visible_rows_dirty {
            self.visible_rows = self
                .root
                .as_ref()
                .map(VisibleRows::from_root)
                .unwrap_or_default();
            self.visible_rows_dirty = false;
        }

        let mode = self.mode;
        let options = RenderOptions {
            search: &self.search,
            matching_paths: self.search.matches.iter().map(String::as_str).collect(),
            format: self.file_state.format.unwrap_or(DataFormat::Json),
            mode,
            scroll_to_path: scroll_to_path.as_deref(),
            selected_paths: &self.selected_paths,
            locale: self.locale,
        };
        let mut outcome = TreeOutcome::default();
        let total_rows = self.visible_rows.len();
        let row_height = tree_row_height(ui);
        let target_row = scroll_to_path.as_deref().and_then(|path| {
            self.root
                .as_ref()
                .and_then(|root| visible_row_index(root, path))
        });
        let visible_rows = &self.visible_rows;
        let root = &mut self.root;

        egui::ScrollArea::both().auto_shrink([false; 2]).show_rows(
            ui,
            row_height,
            total_rows,
            |ui, row_range| {
                if let Some(target_row) = target_row
                    && !row_range.contains(&target_row)
                {
                    let row_height_with_spacing = row_height + ui.spacing().item_spacing.y;
                    let content_top =
                        ui.max_rect().top() - row_range.start as f32 * row_height_with_spacing;
                    let target_top = content_top + target_row as f32 * row_height_with_spacing;
                    let clip_rect = ui.clip_rect();
                    ui.scroll_to_rect(
                        egui::Rect::from_min_max(
                            egui::pos2(clip_rect.left(), target_top),
                            egui::pos2(clip_rect.right(), target_top + row_height),
                        ),
                        Some(egui::Align::Center),
                    );
                }
                if let Some(root) = root {
                    render_visible_rows(ui, root, visible_rows, &options, &mut outcome, row_range);
                }
            },
        );

        if outcome.expansion_changed {
            self.visible_rows = self
                .root
                .as_ref()
                .map(VisibleRows::from_root)
                .unwrap_or_default();
            self.visible_rows_dirty = false;
            ui.ctx().request_discard("Tree expansion changed");
        }
        outcome
    }
}
