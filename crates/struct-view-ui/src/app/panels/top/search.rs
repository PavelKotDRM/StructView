use super::*;

impl StructViewApp {
    pub(super) fn show_search_window(&mut self, ctx: &egui::Context) {
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
}
