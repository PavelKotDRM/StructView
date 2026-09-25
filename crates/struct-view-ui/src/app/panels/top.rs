use super::*;

impl StructViewApp {
    /// Отрисовать верхнюю панель с меню, переключателем режима и строкой поиска.
    pub(in crate::app) fn show_top_panel(&mut self, ui: &mut Ui) {
        self.handle_shortcuts(ui.ctx());
        egui::Panel::top("top_panel").show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("top_panel_controls_scroll")
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.show_menu_bar(ui);
                        ui.separator();
                        if self.is_comparing() {
                            self.show_comparison_buttons(ui);
                        } else {
                            self.show_tree_buttons(ui);
                            ui.separator();
                            self.show_mode_switch(ui);
                            ui.separator();
                            self.show_search_bar(ui);
                        }
                        if self.root.is_some() || self.comparison.is_some() {
                            ui.separator();
                            self.show_visualization_selector(ui);
                        }
                    });
                });
        });
    }

    /// Отрисовать строку меню («Файл», «Вид», «Помощь»).
    fn show_menu_bar(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::FileMenu), |ui| {
            if ui.button(locale.text(TextKey::NewFile)).clicked() {
                ui.close();
                self.open_new_file_dialog();
            }
            if ui.button(locale.text(TextKey::Open)).clicked() {
                ui.close();
                self.open_file_dialog();
            }
            if ui.button(locale.text(TextKey::CompareFiles)).clicked() {
                ui.close();
                self.open_comparison_dialog();
            }
            if ui
                .add_enabled(
                    self.root.is_some(),
                    egui::Button::new(locale.text(TextKey::Save)),
                )
                .clicked()
            {
                ui.close();
                self.request_save_current();
            }
            if ui
                .add_enabled(
                    self.root.is_some(),
                    egui::Button::new(locale.text(TextKey::SaveAs)),
                )
                .clicked()
            {
                ui.close();
                self.save_pretty();
            }
            ui.menu_button(locale.text(TextKey::ConvertTo), |ui| {
                let current_format = self.file_state.format;
                for format in DataFormat::ALL {
                    if current_format == Some(format) {
                        continue;
                    }

                    if ui
                        .add_enabled(self.root.is_some(), egui::Button::new(format.to_string()))
                        .clicked()
                    {
                        ui.close();
                        self.convert_to_format(format);
                    }
                }
            });
            if ui.button(locale.text(TextKey::CloseFile)).clicked() {
                ui.close();
                self.close_file();
            }
            ui.separator();
            if ui.button(locale.text(TextKey::Exit)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        ui.menu_button(locale.text(TextKey::EditMenu), |ui| {
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

        ui.menu_button(locale.text(TextKey::ViewMenu), |ui| {
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
            ui.separator();
            if ui.button(locale.text(TextKey::ExpandAll)).clicked() {
                ui.close();
                self.set_all_expanded(true);
            }
            if ui.button(locale.text(TextKey::CollapseAll)).clicked() {
                ui.close();
                self.set_all_expanded(false);
            }
        });

        ui.menu_button(locale.text(TextKey::SettingsMenu), |ui| {
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

        ui.menu_button(locale.text(TextKey::HelpMenu), |ui| {
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
    }

    /// Отрисовать кнопки управления режимом сравнения.
    fn show_comparison_buttons(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        if ui.button(locale.text(TextKey::CompareFiles)).clicked() {
            self.open_comparison_dialog();
        }
        if ui.button(locale.text(TextKey::Close)).clicked() {
            self.close_file();
        }
    }

    /// Выбрать представление открытого документа или сравниваемой пары.
    fn show_visualization_selector(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        let comparing = self.is_comparing();
        let mut selected = self.visualization;
        let label = visualization_label(selected, locale);
        ui.label(locale.text(TextKey::Visualization));
        egui::ComboBox::from_id_salt("visualization_mode")
            .selected_text(label)
            .show_ui(ui, |ui| {
                if comparing {
                    for (mode, text_key) in [
                        (VisualizationMode::Comparison, TextKey::ComparisonView),
                        (VisualizationMode::Diff, TextKey::DiffView),
                    ] {
                        ui.selectable_value(&mut selected, mode, locale.text(text_key));
                    }
                } else {
                    for (mode, text_key) in [
                        (VisualizationMode::Tree, TextKey::TreeView),
                        (VisualizationMode::Graph, TextKey::GraphView),
                        (VisualizationMode::Table, TextKey::TableView),
                        (VisualizationMode::Schema, TextKey::SchemaView),
                    ] {
                        ui.selectable_value(&mut selected, mode, locale.text(text_key));
                    }
                }
            });
        self.visualization = selected;
    }

    /// Отрисовать быстрые кнопки дерева и сохранения файла.
    fn show_tree_buttons(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        if ui.button(locale.text(TextKey::ToolbarExpandAll)).clicked() {
            self.set_all_expanded(true);
        }
        if ui
            .button(locale.text(TextKey::ToolbarCollapseAll))
            .clicked()
        {
            self.set_all_expanded(false);
        }
        if ui.button(locale.text(TextKey::Save)).clicked() {
            self.request_save_current();
        }
        if ui
            .add_enabled(
                !self.selected_paths.is_empty(),
                egui::Button::new(locale.text(TextKey::Copy)),
            )
            .clicked()
        {
            self.copy_structures_requested = true;
        }
        if ui
            .add_enabled(
                self.can_delete_selected(),
                egui::Button::new(locale.text(TextKey::Delete)),
            )
            .clicked()
        {
            self.delete_requested = true;
        }
        if ui
            .add_enabled(
                self.mode == AppMode::Edit && self.can_paste_into_selected(),
                egui::Button::new(locale.text(TextKey::Paste)),
            )
            .clicked()
        {
            self.paste_requested = true;
        }
        if ui.button(locale.text(TextKey::Close)).clicked() {
            self.close_file();
        }
    }

    /// Обработать горячие клавиши команд редактирования и работы со структурами.
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let (copy, paste, undo, redo, delete) = ctx.input(|input| {
            let command = input.modifiers.command;
            (
                command && input.key_pressed(egui::Key::C),
                command && input.key_pressed(egui::Key::V),
                command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
                command
                    && (input.key_pressed(egui::Key::Y)
                        || (input.modifiers.shift && input.key_pressed(egui::Key::Z))),
                input.key_pressed(egui::Key::Delete),
            )
        });
        self.copy_structures_requested |= copy;
        self.paste_requested |= paste;
        self.undo_requested |= undo && self.can_undo();
        self.redo_requested |= redo && self.can_redo();
        self.delete_requested |= delete && self.can_delete_selected();
    }

    /// Развернуть или свернуть все узлы дерева.
    fn set_all_expanded(&mut self, expanded: bool) {
        if let Some(root) = &mut self.root {
            set_expanded_all(root, expanded);
            self.visible_rows_dirty = true;
        }
    }

    /// Отрисовать переключатель режима «Просмотр» / «Редактирование».
    fn show_mode_switch(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.label(locale.text(TextKey::Mode));
        ui.selectable_value(
            &mut self.mode,
            AppMode::View,
            locale.text(TextKey::ViewMode),
        );
        ui.selectable_value(
            &mut self.mode,
            AppMode::Edit,
            locale.text(TextKey::EditMode),
        );
    }

    /// Отрисовать строку поиска и навигацию по совпадениям.
    fn show_search_bar(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.label("🔍");
        let search_response = ui.add(
            egui::TextEdit::singleline(&mut self.search_query_buf)
                .hint_text(locale.text(TextKey::SearchPlaceholder))
                .desired_width(SEARCH_FIELD_MIN_WIDTH)
                .min_size(egui::vec2(SEARCH_FIELD_MIN_WIDTH, 0.0)),
        );

        let query_changed = search_response.changed();
        let enter_pressed =
            search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut options_changed = false;
        ui.menu_button(locale.text(TextKey::SearchOptions), |ui| {
            options_changed |= ui
                .checkbox(
                    &mut self.search.options.search_keys,
                    locale.text(TextKey::SearchKeys),
                )
                .changed();
            options_changed |= ui
                .checkbox(
                    &mut self.search.options.search_values,
                    locale.text(TextKey::SearchValues),
                )
                .changed();
            ui.separator();
            options_changed |= ui
                .checkbox(
                    &mut self.search.options.case_sensitive,
                    locale.text(TextKey::CaseSensitive),
                )
                .changed();
            options_changed |= ui
                .checkbox(
                    &mut self.search.options.exact_match,
                    locale.text(TextKey::ExactMatch),
                )
                .changed();
        });

        if query_changed || enter_pressed || options_changed {
            self.refresh_search();
            if self.root.is_some() {
                self.request_search_scroll();
            }
        }

        let match_count = self.search.matches.len();
        if match_count > 0 {
            ui.label(
                RichText::new(format!("{}/{}", self.search.current_index + 1, match_count))
                    .color(COLOR_MATCH),
            );
            if ui.button("<").clicked() {
                self.search.prev();
                self.request_search_scroll();
            }
            if ui.button(">").clicked() {
                self.search.next();
                self.request_search_scroll();
            }
        } else if !self.search_query_buf.is_empty() {
            ui.label(RichText::new(locale.text(TextKey::NotFound)).color(Color32::GRAY));
        }
    }
}
