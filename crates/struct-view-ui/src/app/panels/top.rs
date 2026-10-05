use super::*;
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

    /// Показывать обычные разделы меню на широком экране и общий список на узком.
    fn show_menu_bar(&mut self, ui: &mut Ui, compact: bool) {
        let locale = self.locale;
        if compact {
            ui.menu_button(locale.text(TextKey::MainMenu), |ui| {
                self.show_file_menu(ui);
                ui.separator();
                self.show_edit_menu(ui);
                ui.separator();
                self.show_view_menu(ui);
                ui.separator();
                self.show_settings_menu(ui);
                ui.separator();
                self.show_help_menu(ui);
            });
        } else {
            self.show_file_menu(ui);
            self.show_edit_menu(ui);
            self.show_view_menu(ui);
            self.show_settings_menu(ui);
            self.show_help_menu(ui);
        }
    }

    fn show_file_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::FileMenu), |ui| {
            if ui.button(locale.text(TextKey::NewFile)).clicked() {
                ui.close();
                self.open_new_file_dialog();
            }
            if ui.button(locale.text(TextKey::Open)).clicked() {
                ui.close();
                if self.visualization == VisualizationMode::Structure {
                    self.structure_view.open_file_dialog();
                } else {
                    self.open_file_dialog();
                }
            }
            if self.visualization == VisualizationMode::Structure {
                self.structure_view.file_menu(ui, locale);
                self.structure_view.export_menu(ui, locale);
            } else if self.visualization == VisualizationMode::Graph {
                self.show_graph_export_menu(ui);
            }
            if ui.button(locale.text(TextKey::CompareFiles)).clicked() {
                ui.close();
                self.open_comparison_dialog();
            }
            if ui
                .add_enabled(
                    self.root.is_some()
                        && self.visualization != VisualizationMode::Structure
                        && self
                            .file_state
                            .format
                            .is_none_or(DataFormat::is_serializable),
                    egui::Button::new(locale.text(TextKey::Save)),
                )
                .clicked()
            {
                ui.close();
                self.request_save_current();
            }
            if ui
                .add_enabled(
                    self.root.is_some() && self.visualization != VisualizationMode::Structure,
                    egui::Button::new(locale.text(TextKey::SaveAs)),
                )
                .clicked()
            {
                ui.close();
                self.save_as();
            }
            ui.menu_button(locale.text(TextKey::ConvertTo), |ui| {
                let current_format = self.file_state.format;
                for format in DataFormat::ALL {
                    if current_format == Some(format) {
                        continue;
                    }

                    if ui
                        .add_enabled(
                            self.root.is_some()
                                && self.visualization != VisualizationMode::Structure,
                            egui::Button::new(format.to_string()),
                        )
                        .clicked()
                    {
                        ui.close();
                        self.convert_to_format(format);
                    }
                }
            });
            if ui.button(locale.text(TextKey::CloseFile)).clicked() {
                ui.close();
                if self.visualization == VisualizationMode::Structure {
                    self.structure_view = super::super::views::structure::StructureView::default();
                }
                self.close_file();
            }
            ui.separator();
            if ui.button(locale.text(TextKey::Exit)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn show_edit_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::EditMenu), |ui| {
            if ui.button(locale.text(TextKey::SearchWindow)).clicked() {
                self.search_window_open = true;
                ui.close();
            }
            if self.visualization == VisualizationMode::Structure {
                return;
            }
            if self.root.is_some() {
                ui.menu_button(locale.text(TextKey::Mode), |ui| {
                    let can_edit = self
                        .file_state
                        .format
                        .is_none_or(DataFormat::is_serializable);
                    if ui
                        .selectable_value(
                            &mut self.mode,
                            AppMode::View,
                            locale.text(TextKey::ViewMode),
                        )
                        .changed()
                    {
                        ui.close();
                    }
                    if can_edit
                        && ui
                            .selectable_value(
                                &mut self.mode,
                                AppMode::Edit,
                                locale.text(TextKey::EditMode),
                            )
                            .changed()
                    {
                        ui.close();
                    }
                });
                ui.separator();
            }

            if ui
                .add_enabled(
                    self.can_undo(),
                    egui::Button::new(locale.text(TextKey::Undo)),
                )
                .clicked()
            {
                self.undo_requested = true;
                ui.close();
            }
            if ui
                .add_enabled(
                    self.can_redo(),
                    egui::Button::new(locale.text(TextKey::Redo)),
                )
                .clicked()
            {
                self.redo_requested = true;
                ui.close();
            }
            ui.separator();

            let copy_enabled = !self.selected_paths.is_empty();
            if ui
                .add_enabled(
                    copy_enabled,
                    egui::Button::new(locale.text(TextKey::CopySelectedStructures)),
                )
                .clicked()
            {
                self.copy_structures_requested = true;
                ui.close();
            }

            let paste_enabled = self.mode == AppMode::Edit && self.can_paste_into_selected();
            if ui
                .add_enabled(
                    paste_enabled,
                    egui::Button::new(locale.text(TextKey::PasteSelectedContainer)),
                )
                .clicked()
            {
                self.paste_requested = true;
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    self.can_delete_selected(),
                    egui::Button::new(locale.text(TextKey::DeleteSelectedStructures)),
                )
                .clicked()
            {
                self.delete_requested = true;
                ui.close();
            }
        });
    }

    fn show_view_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        let comparing = self.is_comparing();
        ui.menu_button(locale.text(TextKey::ViewMenu), |ui| {
            ui.menu_button(locale.text(TextKey::Visualization), |ui| {
                if comparing {
                    for (mode, text_key) in [
                        (VisualizationMode::Comparison, TextKey::ComparisonView),
                        (VisualizationMode::Diff, TextKey::DiffView),
                    ] {
                        if ui
                            .selectable_value(&mut self.visualization, mode, locale.text(text_key))
                            .changed()
                        {
                            ui.close();
                        }
                    }
                } else {
                    for (mode, text_key) in [
                        (VisualizationMode::Tree, TextKey::TreeView),
                        (VisualizationMode::Graph, TextKey::GraphView),
                        (VisualizationMode::Structure, TextKey::StructureView),
                        (VisualizationMode::Table, TextKey::TableView),
                        (VisualizationMode::Schema, TextKey::SchemaView),
                    ] {
                        if ui
                            .selectable_value(&mut self.visualization, mode, locale.text(text_key))
                            .changed()
                        {
                            self.refresh_search();
                            ui.close();
                        }
                    }
                }
            });

            if self.root.is_some() && self.visualization != VisualizationMode::Structure {
                ui.add_enabled_ui(self.graph_calculation.has_started(), |ui| {
                    ui.menu_button(locale.text(TextKey::GraphTimings), |ui| {
                        self.graph_calculation.show_progress(ui, locale);
                    });
                });
            }
            if self.root.is_some() || self.visualization == VisualizationMode::Structure {
                ui.menu_button(locale.text(TextKey::TreeActions), |ui| {
                    if ui.button(locale.text(TextKey::ExpandAll)).clicked() {
                        ui.close();
                        self.set_all_expanded(true);
                    }
                    if ui.button(locale.text(TextKey::CollapseAll)).clicked() {
                        ui.close();
                        self.set_all_expanded(false);
                    }
                });
            }

            if self.visualization == VisualizationMode::Structure {
                ui.separator();
                self.structure_view.view_menu(ui, locale);
            } else if self.visualization == VisualizationMode::Graph {
                ui.separator();
                ui.add_enabled_ui(self.graph_calculation.result().is_some(), |ui| {
                    if let Some(calculation) = self.graph_calculation.result() {
                        super::super::views::graph_view_menu(ui, &calculation.routing, locale);
                    }
                });
            }
            ui.separator();
            let theme_label = if self.dark_mode {
                locale.text(TextKey::LightTheme)
            } else {
                locale.text(TextKey::DarkTheme)
            };
            if ui.button(theme_label).clicked() {
                ui.close();
                self.dark_mode = !self.dark_mode;
                let ctx = ui.ctx().clone();
                self.apply_theme(&ctx);
            }
        });
    }

    fn show_settings_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::SettingsMenu), |ui| {
            if self.visualization == VisualizationMode::Graph {
                let previous_setting = self.graph_routing_workers;
                let automatic_count = super::super::views::graph_routing_worker_count(
                    usize::MAX,
                    super::super::views::GraphRoutingWorkerSetting::Automatic,
                );
                ui.label(locale.text(TextKey::GraphRoutingWorkers));
                ui.selectable_value(
                    &mut self.graph_routing_workers,
                    super::super::views::GraphRoutingWorkerSetting::Automatic,
                    format!(
                        "{} ({automatic_count})",
                        locale.text(TextKey::GraphRoutingAutomatic)
                    ),
                );
                let maximum = super::super::views::available_graph_routing_workers();
                let mut manual_count = match self.graph_routing_workers {
                    super::super::views::GraphRoutingWorkerSetting::Manual(count) => count,
                    super::super::views::GraphRoutingWorkerSetting::Automatic => automatic_count,
                };
                ui.horizontal(|ui| {
                    let manual_selected = matches!(
                        self.graph_routing_workers,
                        super::super::views::GraphRoutingWorkerSetting::Manual(_)
                    );
                    let manual_clicked = ui
                        .selectable_label(manual_selected, locale.text(TextKey::GraphRoutingCustom))
                        .clicked();
                    let count_changed = ui
                        .add(egui::DragValue::new(&mut manual_count).range(1..=maximum))
                        .changed();
                    if manual_clicked || count_changed {
                        self.graph_routing_workers =
                            super::super::views::GraphRoutingWorkerSetting::Manual(manual_count);
                    }
                });
                if self.graph_routing_workers != previous_setting {
                    self.graph_calculation = super::super::views::GraphCalculationState::default();
                }
                ui.separator();
            }
            if self.visualization == VisualizationMode::Structure {
                self.structure_view.settings_menu(ui, locale);
                ui.separator();
            }
            ui.label(locale.text(TextKey::Language));
            for available_locale in Locale::ALL {
                if ui
                    .selectable_label(locale == available_locale, available_locale.language_name())
                    .clicked()
                {
                    self.locale = available_locale;
                    ui.close();
                }
            }
        });
    }

    fn show_help_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::HelpMenu), |ui| {
            ui.menu_button(locale.text(TextKey::DiagramLegend), |ui| {
                ui.set_max_width(480.0);
                egui::ScrollArea::vertical()
                    .max_height(ui.ctx().content_rect().height() * 0.75)
                    .show(ui, |ui| {
                        crate::app::views::structure::show_legend(ui, locale);
                        ui.separator();
                        crate::app::views::diagram::show_controls_help(ui, locale);
                        ui.separator();
                        ui.label(locale.text(TextKey::StructureDisk));
                    });
            });
            ui.menu_button(locale.text(TextKey::BuildInformation), |ui| {
                ui.label(format!("StructView {}", build_info::VERSION));
                ui.separator();
                egui::Grid::new("about_build_info")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        for (name, value) in [
                            (locale.text(TextKey::BuildTime), build_info::BUILD_TIMESTAMP),
                            (
                                locale.text(TextKey::TargetPlatform),
                                build_info::TARGET_TRIPLE,
                            ),
                            (locale.text(TextKey::HostPlatform), build_info::HOST_TRIPLE),
                            (
                                locale.text(TextKey::OptimizationLevel),
                                build_info::OPT_LEVEL,
                            ),
                            (locale.text(TextKey::DebugBuild), build_info::DEBUG),
                            (
                                locale.text(TextKey::RustcCompiler),
                                build_info::RUSTC_SEMVER,
                            ),
                            (
                                locale.text(TextKey::RustcChannel),
                                build_info::RUSTC_CHANNEL,
                            ),
                        ] {
                            ui.label(name);
                            ui.label(RichText::new(value).monospace());
                            ui.end_row();
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

    fn show_search_window(&mut self, ctx: &egui::Context) {
        if !self.search_window_open {
            self.search_window_docking.reset_if_active();
            return;
        }

        let locale = self.locale;
        let mut docking = std::mem::take(&mut self.search_window_docking);
        docking.begin();
        let mut show_content = |ui: &mut egui::Ui| {
            if if self.visualization == VisualizationMode::Structure {
                !self.structure_view.has_document()
            } else {
                self.root.is_none()
            } {
                ui.label(locale.text(TextKey::NoDocument));
                return;
            }

            ui.horizontal(|ui| {
                ui.label("🔍");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query_buf)
                        .hint_text(locale.text(TextKey::SearchPlaceholder))
                        .desired_width(ui.available_width() - 90.0),
                );
                if response.changed()
                    || (response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                {
                    self.refresh_search();
                    self.request_search_scroll();
                }
                if ui.button(locale.text(TextKey::ClearSearch)).clicked() {
                    self.search_query_buf.clear();
                    self.regex_builder_literal.clear();
                    self.refresh_search();
                }
            });

            ui.separator();
            ui.label(locale.text(TextKey::SearchIn));
            let mut scope_changed = false;
            ui.horizontal_wrapped(|ui| {
                scope_changed |= ui
                    .checkbox(
                        &mut self.search.options.search_keys,
                        locale.text(TextKey::SearchKeys),
                    )
                    .changed();
                scope_changed |= ui
                    .checkbox(
                        &mut self.search.options.search_values,
                        locale.text(TextKey::SearchValues),
                    )
                    .changed();
                scope_changed |= ui
                    .checkbox(
                        &mut self.search.options.search_paths,
                        locale.text(TextKey::SearchPaths),
                    )
                    .changed();
            });
            ui.separator();
            ui.label(locale.text(TextKey::SearchOptions));
            let mut matching_changed = false;
            let regex_changed = ui
                .checkbox(
                    &mut self.search.options.use_regex,
                    locale.text(TextKey::RegexSearch),
                )
                .changed();
            matching_changed |= regex_changed;
            if regex_changed && self.search.options.use_regex {
                self.search.options.exact_match = false;
                self.search.options.whole_word = false;
            }
            if self.search.options.use_regex {
                ui.small(locale.text(TextKey::RegexSearchHelp));
                ui.collapsing(locale.text(TextKey::RegexBuilder), |ui| {
                    ui.label(locale.text(TextKey::RegexBuilderHelp));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.regex_builder_literal)
                                .hint_text(locale.text(TextKey::RegexLiteralPlaceholder)),
                        );
                        if ui
                            .add_enabled(
                                !self.regex_builder_literal.is_empty(),
                                egui::Button::new(locale.text(TextKey::RegexAddLiteral)),
                            )
                            .clicked()
                        {
                            let literal = escape_regex_literal(&self.regex_builder_literal);
                            self.search_query_buf.push_str(&literal);
                            self.regex_builder_literal.clear();
                            self.refresh_search();
                            self.request_search_scroll();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for (key, fragment) in [
                            (TextKey::RegexDigit, r"\d"),
                            (TextKey::RegexDigits, r"\d+"),
                            (TextKey::RegexWord, r"\w+"),
                            (TextKey::RegexWhitespace, r"\s+"),
                            (TextKey::RegexAnyCharacter, "."),
                            (TextKey::RegexAnyText, ".*"),
                            (TextKey::RegexStart, "^"),
                            (TextKey::RegexEnd, "$"),
                        ] {
                            if ui.button(locale.text(key)).clicked() {
                                self.search_query_buf.push_str(fragment);
                                self.refresh_search();
                                self.request_search_scroll();
                            }
                        }
                    });
                });
            }
            ui.horizontal_wrapped(|ui| {
                matching_changed |= ui
                    .checkbox(
                        &mut self.search.options.case_sensitive,
                        locale.text(TextKey::CaseSensitive),
                    )
                    .changed();
            });
            ui.add_enabled_ui(!self.search.options.use_regex, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let exact_changed = ui
                        .checkbox(
                            &mut self.search.options.exact_match,
                            locale.text(TextKey::ExactMatch),
                        )
                        .changed();
                    if exact_changed && self.search.options.exact_match {
                        self.search.options.whole_word = false;
                    }
                    let whole_word_changed = ui
                        .checkbox(
                            &mut self.search.options.whole_word,
                            locale.text(TextKey::WholeWord),
                        )
                        .changed();
                    if whole_word_changed && self.search.options.whole_word {
                        self.search.options.exact_match = false;
                    }
                    matching_changed |= exact_changed || whole_word_changed;
                });
            });
            if scope_changed || matching_changed {
                self.refresh_search();
                self.request_search_scroll();
            }

            ui.separator();
            let count = self.search.matches.len();
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{}: {}",
                    locale.text(TextKey::SearchResults),
                    count
                ));
                if count > 0 {
                    ui.label(format!("{}/{}", self.search.current_index + 1, count));
                    if ui.button("<").clicked() {
                        self.search.prev();
                        self.request_search_scroll();
                    }
                    if ui.button(">").clicked() {
                        self.search.next();
                        self.request_search_scroll();
                    }
                }
            });
            if let Some(error) = &self.search.error {
                ui.colored_label(
                    SyntaxColors::new(ui.visuals()).error,
                    format!("{}: {}", locale.text(TextKey::RegexSearchError), error),
                );
            }
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for (index, path) in self.search.matches.clone().into_iter().enumerate() {
                        let label = if path.is_empty() {
                            "$".to_string()
                        } else {
                            path
                        };
                        if ui
                            .selectable_label(index == self.search.current_index, label)
                            .clicked()
                        {
                            self.search.current_index = index;
                            self.request_search_scroll();
                        }
                    }
                });
            if count == 0 && !self.search_query_buf.is_empty() && self.search.error.is_none() {
                ui.colored_label(Color32::GRAY, locale.text(TextKey::NotFound));
            }
        };

        let action = if docking.detached {
            show_detached_viewport(
                ctx,
                &mut docking,
                egui::ViewportId::from_hash_of("advanced_search"),
                locale.text(TextKey::SearchWindow),
                egui::vec2(480.0, 460.0),
                locale.text(TextKey::DockWindow),
                |ui| show_content(ui),
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(locale.text(TextKey::SearchWindow))
                .id(docking.area_id("advanced_search"))
                .open(&mut open)
                .default_width(480.0)
                .resizable(true);
            if let Some(size) = docking.embedded_size() {
                window = window.default_size(size);
            }
            if let Some(position) = docking.take_embedded_position() {
                window = window.current_pos(position);
            }
            let mut detach_requested = false;
            let response = window.show(ctx, |ui| {
                detach_requested = show_docking_controls(
                    ui,
                    false,
                    locale.text(TextKey::DetachWindow),
                    locale.text(TextKey::WindowOptions),
                );
                show_content(ui);
            });
            if !open {
                docking.reset_if_active();
                DetachedViewportAction::Closed
            } else {
                if let Some(response) = response {
                    docking.remember_embedded_size(response.response.rect.size());
                    if detach_requested {
                        let viewport_origin =
                            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min));
                        docking.detach(
                            ctx.viewport_rect(),
                            response.response.rect,
                            viewport_origin,
                        );
                        ctx.request_repaint();
                    }
                }
                DetachedViewportAction::KeepOpen
            }
        };
        drop(show_content);
        self.search_window_docking = docking;
        if action == DetachedViewportAction::Closed {
            self.search_window_open = false;
        }
    }

    /// Развернуть или свернуть все узлы дерева.
    fn set_all_expanded(&mut self, expanded: bool) {
        if self.visualization == VisualizationMode::Structure {
            self.structure_view.set_all_expanded(expanded);
            return;
        }
        if let Some(root) = &mut self.root {
            set_expanded_all(root, expanded);
            self.visible_rows_dirty = true;
        }
    }

    fn show_graph_export_menu(&mut self, ui: &mut Ui) {
        let mut requested = None;
        ui.add_enabled_ui(self.graph_calculation.result().is_some(), |ui| {
            requested = super::super::views::diagram::export_controls(ui, self.locale);
        });
        if let Some((png, dark)) = requested
            && let Some(calculation) = self.graph_calculation.result()
        {
            let result = export_graph_image(
                &calculation.graph,
                &calculation.routing,
                if png {
                    super::super::views::GraphExportFormat::Png
                } else {
                    super::super::views::GraphExportFormat::Svg
                },
                if dark {
                    super::super::views::GraphExportStyle::DarkOpaque
                } else {
                    super::super::views::GraphExportStyle::LightTransparent
                },
            );
            match result {
                Ok(true) => self.show_toast(self.locale.text(TextKey::GraphExported)),
                Ok(false) => {}
                Err(error) => self.show_error(&self.locale.save_error(&error.to_string())),
            }
        }
    }
}
