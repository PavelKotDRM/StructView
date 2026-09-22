//! Отрисовка панелей главного окна: меню и поиск, статус-бар, дерево данных.

use egui::{Color32, RichText, Ui};

use crate::build_info;
use crate::clipboard::copy_to_clipboard;
use crate::diff::format_value;
use crate::parser::set_expanded_all;

use super::i18n::{Locale, TextKey};
use super::state::{AppMode, JsonViewerApp};
use super::theme::{COLOR_ERROR, COLOR_MATCH, COLOR_SUCCESS};
use super::tree::{
    RenderOptions, TreeOutcome, VisibleRows, focus_match_path, render_visible_rows,
    tree_row_height, visible_row_index,
};

/// Время показа всплывающего уведомления в секундах.
const TOAST_LIFETIME_SECS: u64 = 3;
/// Минимальная ширина поля поиска, достаточная для отображения подсказки.
const SEARCH_FIELD_MIN_WIDTH: f32 = 260.0;

impl JsonViewerApp {
    /// Отрисовать верхнюю панель с меню, переключателем режима и строкой поиска.
    pub(super) fn show_top_panel(&mut self, ui: &mut Ui) {
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
                    });
                });
        });
    }

    /// Отрисовать строку меню («Файл», «Вид», «Помощь»).
    fn show_menu_bar(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::FileMenu), |ui| {
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
            ui.label(format!("JSON Viewer {}", build_info::VERSION));
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

    /// Обработать горячие клавиши копирования и вставки структур.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let (copy, paste) = ctx.input(|input| {
            (
                input.modifiers.command && input.key_pressed(egui::Key::C),
                input.modifiers.command && input.key_pressed(egui::Key::V),
            )
        });
        self.copy_structures_requested |= copy;
        self.paste_requested |= paste;
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

    /// Отрисовать нижнюю панель (статус-бар) с информацией о файле и уведомлениями.
    pub(super) fn show_bottom_panel(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.parse_error {
                    ui.label(
                        RichText::new(format!("⚠ {}: {}", locale.text(TextKey::Error), err))
                            .color(COLOR_ERROR),
                    );
                } else if let Some(comparison) = &self.comparison {
                    ui.label(locale.comparison_status(
                        comparison.documents.len(),
                        comparison.differences.len(),
                    ));
                } else if let Some(path) = &self.file_state.path {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(locale.text(TextKey::UnknownFile));
                    let size_kb = self.file_state.size_bytes as f64 / 1024.0;
                    let format = self
                        .file_state
                        .format
                        .map(|format| format.to_string())
                        .unwrap_or_else(|| locale.text(TextKey::UnknownFormat).to_string());
                    ui.label(locale.loaded_file_status(
                        name,
                        &format,
                        size_kb,
                        self.file_state.load_time_ms,
                    ));
                } else {
                    ui.label(RichText::new(locale.text(TextKey::Placeholder)).color(Color32::GRAY));
                }

                self.show_toast_label(ui);
            });
        });
    }

    /// Отрисовать всплывающее уведомление, если оно ещё не устарело.
    fn show_toast_label(&mut self, ui: &mut Ui) {
        let Some((message, shown_at)) = &self.toast else {
            return;
        };
        if shown_at.elapsed().as_secs() >= TOAST_LIFETIME_SECS {
            self.toast = None;
            return;
        }
        let text = RichText::new(format!("✔ {}", message)).color(COLOR_SUCCESS);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(text);
        });
    }

    /// Отрисовать центральную панель с деревом JSON.
    pub(super) fn show_central_panel(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.handle_dropped_files(ui);
            let save_requested = std::mem::take(&mut self.save_requested);
            let copy_structures_requested = std::mem::take(&mut self.copy_structures_requested);
            let paste_requested = std::mem::take(&mut self.paste_requested);

            if self.comparison.is_some() {
                self.show_comparison(ui);
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

            let outcome = self.show_tree(ui);

            if let Some(request) = outcome.selection_request {
                self.apply_selection_request(request);
            }
            if let Some(request) = outcome.add_child_request {
                self.open_add_child_dialog(request);
            }
            if outcome.tree_changed {
                self.refresh_search();
            }
            if save_requested {
                self.save_current();
            }
            if copy_structures_requested {
                self.copy_structures_at_paths(self.selected_paths.iter().cloned().collect());
            }
            if let Some(paths) = outcome.copy_structure_paths {
                self.copy_structures_at_paths(paths);
            }
            if paste_requested {
                self.paste_into_selected();
            }
            if let Some(path) = outcome.paste_target_path {
                self.paste_into_path(path);
            }
            if let Some(text) = outcome.copy_request {
                self.clipboard_payload = None;
                match copy_to_clipboard(&text) {
                    Ok(_) => self.show_toast(self.locale.text(TextKey::Copied)),
                    Err(e) => self.show_toast(&self.locale.copy_error(&e)),
                }
            }
            if let Some(err) = outcome.edit_error {
                self.show_toast(&err);
            }
            self.show_add_child_dialog(ui.ctx());
        });
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

        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("comparison_grid")
                .striped(true)
                .min_col_width(140.0)
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
                        for value in &difference.values {
                            let text = value
                                .as_ref()
                                .map(|value| format_value(Some(value)))
                                .unwrap_or_else(|| locale.text(TextKey::MissingValue).to_string());
                            ui.label(RichText::new(text).monospace());
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
            self.visible_rows_dirty = true;
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
