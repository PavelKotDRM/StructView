use super::*;

impl StructViewApp {
    /// Отрисовать центральную панель с деревом JSON.
    pub(in crate::app) fn show_central_panel(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
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
                VisualizationMode::Graph => {
                    if self.visualization_cache.graph.is_none()
                        && let Some(root) = &self.root
                    {
                        self.visualization_cache.graph = Some(build_relationship_graph(root));
                    }
                    if let Some(graph) = &self.visualization_cache.graph {
                        show_graph(ui, graph, &self.search, self.locale);
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
                            ui.colored_label(COLOR_ERROR, error);
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
                self.show_toast(&error);
            }
            if let Some(text) = outcome.copy_request {
                self.clipboard_payload = None;
                match copy_to_clipboard(&text) {
                    Ok(()) => self.show_toast(self.locale.text(TextKey::Copied)),
                    Err(error) => self.show_toast(&self.locale.copy_error(&error)),
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
            self.show_field_dialog(ui.ctx());
        });
    }

    fn save_table_csv(&mut self) {
        let Some(table) = self.visualization_cache.table.as_ref() else {
            return;
        };
        let content = table_csv(table, &self.search);
        if let Some(mut path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name("table.csv")
            .save_file()
        {
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("csv"))
            {
                path.set_extension("csv");
            }
            match std::fs::write(&path, content) {
                Ok(()) => self.show_toast(self.locale.text(TextKey::TableExported)),
                Err(error) => self.show_toast(&self.locale.save_error(&error.to_string())),
            }
        }
    }

    /// Отрисовать таблицу отличий по всем загруженным документам.
    fn show_comparison(&self, ui: &mut Ui) {
        let Some(comparison) = &self.comparison else {
            return;
        };
        let locale = self.locale;

        if comparison.differences.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(locale.text(TextKey::ComparisonNoDifferences))
                        .size(18.0)
                        .color(COLOR_SUCCESS),
                );
            });
            return;
        }

        show_difference_legend(
            ui,
            locale,
            Some(locale.text(TextKey::ComparisonColorLegend)),
        );
        let column_count = comparison.documents.len() + 1;
        let column_width = comparison_column_width(ui.available_width(), column_count);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("comparison_grid")
                    .num_columns(column_count)
                    .striped(true)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new(locale.text(TextKey::ComparisonPath)).strong());
                        for document in &comparison.documents {
                            let header = format!(
                                "{} ({}, {:.1} KB, {} ms)",
                                document.path.display(),
                                document.format,
                                document.size_bytes as f64 / 1024.0,
                                document.load_time_ms
                            );
                            ui.label(RichText::new(header).strong().monospace());
                        }
                        ui.end_row();

                        for difference in &comparison.differences {
                            ui.label(
                                RichText::new(&difference.path)
                                    .color(COLOR_MATCH)
                                    .monospace(),
                            );
                            let reference = difference.values.first().and_then(Option::as_ref);
                            for value in &difference.values {
                                let text = value
                                    .as_ref()
                                    .map(|value| format_value(Some(value)))
                                    .unwrap_or_else(|| {
                                        locale.text(TextKey::MissingValue).to_string()
                                    });
                                let color = comparison_value_color(
                                    reference,
                                    value.as_ref(),
                                    ui.visuals().text_color(),
                                );
                                ui.label(RichText::new(text).color(color).monospace());
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    /// Отрисовать виртуализированную область с деревом и вернуть отложенные действия.
    fn show_tree(&mut self, ui: &mut Ui) -> TreeOutcome {
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

        // Клонируем состояние поиска, чтобы одновременно держать `&mut self.root`.
        let search = self.search.clone();
        let selected_paths = self.selected_paths.clone();
        let mode = self.mode;
        let options = RenderOptions {
            search: &search,
            mode,
            scroll_to_path: scroll_to_path.as_deref(),
            selected_paths: &selected_paths,
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

    /// Загрузить файл, перетащенный в окно приложения.
    fn handle_dropped_files(&mut self, ui: &Ui) {
        let dropped_paths = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        match dropped_paths.as_slice() {
            [] => {}
            [path] => self.load_file(path.clone()),
            _ => self.load_comparison(dropped_paths),
        }
    }
}

pub(super) fn visualization_label(mode: VisualizationMode, locale: Locale) -> &'static str {
    let key = match mode {
        VisualizationMode::Tree => TextKey::TreeView,
        VisualizationMode::Graph => TextKey::GraphView,
        VisualizationMode::Table => TextKey::TableView,
        VisualizationMode::Schema => TextKey::SchemaView,
        VisualizationMode::Comparison => TextKey::ComparisonView,
        VisualizationMode::Diff => TextKey::DiffView,
    };
    locale.text(key)
}

/// Отрисовать подсказку, показываемую, пока файл не открыт.
fn show_placeholder(ui: &mut Ui, locale: Locale) {
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new(locale.text(TextKey::DropFilePlaceholder))
                .size(18.0)
                .color(Color32::GRAY),
        );
    });
}

/// Отрисовать сообщение об ошибке разбора данных.
fn show_parse_error(ui: &mut Ui, message: &str, locale: Locale) {
    ui.add_space(8.0);
    ui.colored_label(COLOR_ERROR, locale.text(TextKey::DataParseError));
    ui.add_space(4.0);
    egui::ScrollArea::both().show(ui, |ui| {
        ui.label(RichText::new(message).monospace());
    });
}
