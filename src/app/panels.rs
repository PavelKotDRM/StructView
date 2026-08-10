//! Отрисовка панелей главного окна: меню и поиск, статус-бар, дерево JSON.

use egui::{Color32, RichText, Ui};

use crate::build_info;
use crate::clipboard::copy_to_clipboard;
use crate::parser::set_expanded_all;

use super::state::{AppMode, JsonViewerApp};
use super::theme::{COLOR_ERROR, COLOR_MATCH, COLOR_SUCCESS};
use super::tree::{TreeOutcome, render_node};

/// Время показа всплывающего уведомления в секундах.
const TOAST_LIFETIME_SECS: u64 = 3;

impl JsonViewerApp {
    /// Отрисовать верхнюю панель с меню, переключателем режима и строкой поиска.
    pub(super) fn show_top_panel(&mut self, ui: &mut Ui) {
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                self.show_menu_bar(ui);
                ui.separator();
                self.show_tree_buttons(ui);
                ui.separator();
                self.show_mode_switch(ui);
                ui.separator();
                self.show_search_bar(ui);
            });
        });
    }

    /// Отрисовать строку меню («Файл», «Вид», «Помощь»).
    fn show_menu_bar(&mut self, ui: &mut Ui) {
        ui.menu_button("Файл", |ui| {
            if ui.button("📂  Открыть…").clicked() {
                ui.close();
                self.open_file_dialog();
            }
            if ui.button("💾  Сохранить как…").clicked() {
                ui.close();
                self.save_pretty();
            }
            ui.separator();
            if ui.button("❌  Выход").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        ui.menu_button("Вид", |ui| {
            let theme_label = if self.dark_mode {
                "☀  Светлая тема"
            } else {
                "🌙  Тёмная тема"
            };
            if ui.button(theme_label).clicked() {
                ui.close();
                self.dark_mode = !self.dark_mode;
                let ctx = ui.ctx().clone();
                self.apply_theme(&ctx);
            }
            ui.separator();
            if ui.button(">  Развернуть все").clicked() {
                ui.close();
                self.set_all_expanded(true);
            }
            if ui.button("<  Свернуть все").clicked() {
                ui.close();
                self.set_all_expanded(false);
            }
        });

        ui.menu_button("Помощь", |ui| {
            ui.label(format!("JSON Viewer {}", build_info::VERSION));
            ui.separator();
            egui::Grid::new("about_build_info")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    for (name, value) in [
                        ("Время сборки", build_info::BUILD_TIMESTAMP),
                        ("Целевая платформа", build_info::TARGET_TRIPLE),
                        ("Платформа сборки", build_info::HOST_TRIPLE),
                        ("Уровень оптимизации", build_info::OPT_LEVEL),
                        ("Отладочная сборка", build_info::DEBUG),
                        ("Компилятор rustc", build_info::RUSTC_SEMVER),
                        ("Канал rustc", build_info::RUSTC_CHANNEL),
                    ] {
                        ui.label(name);
                        ui.label(RichText::new(value).monospace());
                        ui.end_row();
                    }
                });
        });
    }

    /// Отрисовать быстрые кнопки разворачивания и сворачивания дерева.
    fn show_tree_buttons(&mut self, ui: &mut Ui) {
        if ui.button(">> Развернуть все").clicked() {
            self.set_all_expanded(true);
        }
        if ui.button("<< Свернуть все").clicked() {
            self.set_all_expanded(false);
        }
    }

    /// Развернуть или свернуть все узлы дерева.
    fn set_all_expanded(&mut self, expanded: bool) {
        if let Some(root) = &mut self.root {
            set_expanded_all(root, expanded);
        }
    }

    /// Отрисовать переключатель режима «Просмотр» / «Редактирование».
    fn show_mode_switch(&mut self, ui: &mut Ui) {
        ui.label("Режим:");
        ui.selectable_value(&mut self.mode, AppMode::View, "Просмотр");
        ui.selectable_value(&mut self.mode, AppMode::Edit, "Редактирование");
    }

    /// Отрисовать строку поиска и навигацию по совпадениям.
    fn show_search_bar(&mut self, ui: &mut Ui) {
        ui.label("🔍");
        let search_response = ui.add(
            egui::TextEdit::singleline(&mut self.search_query_buf)
                .hint_text("Поиск по ключам и значениям…")
                .desired_width(220.0),
        );

        let query_changed = search_response.changed();
        let enter_pressed =
            search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        if query_changed || enter_pressed {
            let query = self.search_query_buf.clone();
            if let Some(root) = &self.root {
                self.search.search(root, &query);
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
            }
            if ui.button(">").clicked() {
                self.search.next();
            }
        } else if !self.search_query_buf.is_empty() {
            ui.label(RichText::new("Не найдено").color(Color32::GRAY));
        }
    }

    /// Отрисовать нижнюю панель (статус-бар) с информацией о файле и уведомлениями.
    pub(super) fn show_bottom_panel(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.parse_error {
                    ui.label(RichText::new(format!("⚠ Ошибка: {}", err)).color(COLOR_ERROR));
                } else if let Some(path) = &self.file_state.path {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("неизвестный файл");
                    let size_kb = self.file_state.size_bytes as f64 / 1024.0;
                    ui.label(format!(
                        "📄 {}  |  {:.1} КБ  |  загружено за {} мс",
                        name, size_kb, self.file_state.load_time_ms
                    ));
                } else {
                    ui.label(
                        RichText::new("Откройте JSON-файл через меню Файл или перетащите его сюда")
                            .color(Color32::GRAY),
                    );
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

            if self.root.is_none() && self.parse_error.is_none() {
                show_placeholder(ui);
                return;
            }

            if let Some(err) = &self.parse_error {
                show_parse_error(ui, &err.to_string());
                return;
            }

            let outcome = self.show_tree(ui);

            if outcome.tree_changed {
                self.refresh_search();
            }
            if let Some(text) = outcome.copy_request {
                match copy_to_clipboard(&text) {
                    Ok(_) => self.show_toast("Скопировано в буфер обмена"),
                    Err(e) => self.show_toast(&format!("Ошибка копирования: {}", e)),
                }
            }
            if let Some(err) = outcome.edit_error {
                self.show_toast(&err);
            }
        });
    }

    /// Отрисовать прокручиваемую область с деревом и вернуть отложенные действия.
    fn show_tree(&mut self, ui: &mut Ui) -> TreeOutcome {
        // Клонируем состояние поиска, чтобы одновременно держать `&mut self.root`.
        let search = self.search.clone();
        let mode = self.mode;
        let mut outcome = TreeOutcome::default();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(root) = &mut self.root {
                    render_node(ui, root, &search, mode, &mut outcome);
                }
            });

        outcome
    }

    /// Загрузить файл, перетащенный в окно приложения.
    fn handle_dropped_files(&mut self, ui: &Ui) {
        let dropped_path = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .first()
                .and_then(|file| Some(file.path().to_path_buf().clone()))
        });
        if let Some(path) = dropped_path {
            self.load_file(path);
        }
    }
}

/// Отрисовать подсказку, показываемую, пока файл не открыт.
fn show_placeholder(ui: &mut Ui) {
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new("Перетащите JSON-файл сюда\nили используйте Файл -> Открыть…")
                .size(18.0)
                .color(Color32::GRAY),
        );
    });
}

/// Отрисовать сообщение об ошибке разбора JSON.
fn show_parse_error(ui: &mut Ui, message: &str) {
    ui.add_space(8.0);
    ui.colored_label(COLOR_ERROR, "Ошибка разбора JSON:");
    ui.add_space(4.0);
    egui::ScrollArea::both().show(ui, |ui| {
        ui.label(RichText::new(message).monospace());
    });
}
