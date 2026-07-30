//! # Модуль приложения JSON Viewer
//!
//! Содержит структуру [`JsonViewerApp`] — основное состояние приложения —
//! и реализацию трейта [`eframe::App`], управляющего жизненным циклом GUI.
//!
//! ## Архитектура UI
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  Top Panel: меню + строка поиска             │
//! ├──────────────────────────────────────────────┤
//! │  Central Panel: ScrollArea + дерево JSON     │
//! ├──────────────────────────────────────────────┤
//! │  Bottom Panel (Status Bar): имя/размер файла │
//! └──────────────────────────────────────────────┘
//! ```

use std::path::PathBuf;
use std::time::Instant;

use egui::{Color32, RichText, Ui};
use serde_json::Value;

use crate::clipboard::copy_to_clipboard;
use crate::parser::{JsonNode, JsonValueType, ParseError, parse_json, set_expanded_all};
use crate::search::SearchState;

// ─── Цветовая схема ──────────────────────────────────────────────────────────

/// Цвет для строковых значений (зелёный).
const COLOR_STRING: Color32 = Color32::from_rgb(106, 177, 112);
/// Цвет для числовых значений (голубой).
const COLOR_NUMBER: Color32 = Color32::from_rgb(100, 163, 220);
/// Цвет для булевых значений (оранжевый).
const COLOR_BOOL: Color32 = Color32::from_rgb(209, 154, 102);
/// Цвет для значений null (серый).
const COLOR_NULL: Color32 = Color32::from_rgb(150, 150, 150);
/// Цвет для ключей объектов (белый/светлый).
const COLOR_KEY: Color32 = Color32::from_rgb(224, 224, 224);
/// Цвет для подсветки совпадений при поиске (жёлтый).
const COLOR_MATCH: Color32 = Color32::from_rgb(229, 192, 73);
/// Цвет для активного совпадения при поиске (ярко-оранжевый).
const COLOR_ACTIVE_MATCH: Color32 = Color32::from_rgb(255, 120, 50);

// ─── Состояние приложения ─────────────────────────────────────────────────────

/// Состояние загруженного файла.
#[derive(Debug, Default)]
struct FileState {
    /// Путь к файлу на диске.
    path: Option<PathBuf>,
    /// Размер файла в байтах.
    size_bytes: u64,
    /// Время загрузки (мс).
    load_time_ms: u128,
}

/// Режим работы приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AppMode {
    /// Только просмотр данных без изменения значений.
    #[default]
    View,
    /// Разрешено редактирование примитивных значений JSON.
    Edit,
}

/// Основное состояние приложения JSON Viewer.
///
/// Хранит разобранное JSON-дерево, параметры поиска, информацию о файле
/// и временные сообщения для пользователя (уведомления, ошибки).
pub struct JsonViewerApp {
    /// Корневой узел разобранного JSON-дерева. `None` если файл ещё не загружен.
    root: Option<JsonNode>,
    /// Ошибка последнего парсинга. `None` если файл разобран успешно.
    parse_error: Option<ParseError>,
    /// Состояние поиска.
    search: SearchState,
    /// Буфер для строки поиска в UI.
    search_query_buf: String,
    /// Мета-информация о загруженном файле.
    file_state: FileState,
    /// Временное уведомление (например, «Скопировано»).
    toast: Option<(String, Instant)>,
    /// Флаг тёмной темы.
    dark_mode: bool,
    /// Текущий режим работы приложения.
    mode: AppMode,
}

impl Default for JsonViewerApp {
    fn default() -> Self {
        Self {
            root: None,
            parse_error: None,
            search: SearchState::default(),
            search_query_buf: String::new(),
            file_state: FileState::default(),
            toast: None,
            dark_mode: true,
            mode: AppMode::default(),
        }
    }
}

impl JsonViewerApp {
    /// Создать новый экземпляр приложения.
    ///
    /// Инициализирует тему оформления на основе системных предпочтений,
    /// если они доступны через [`eframe::CreationContext`].
    ///
    /// # Arguments
    ///
    /// * `cc` — контекст создания `eframe`, предоставляющий доступ к [`egui::Context`].
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // В eframe 0.35 IntegrationInfo больше не содержит system_theme;
        // читаем начальную тему из текущих визуальных настроек egui
        // (egui сам определяет системную тему при инициализации).
        let dark_mode = cc.egui_ctx.theme() == egui::Theme::Dark;

        let app = Self {
            dark_mode,
            ..Default::default()
        };
        app.apply_theme(&cc.egui_ctx);
        app
    }

    /// Создать приложение и сразу загрузить файл, переданный через командную строку.
    ///
    /// # Arguments
    ///
    /// * `cc` — контекст создания `eframe`.
    /// * `path` — путь к JSON-файлу; `None` — стартовать с пустым состоянием.
    pub fn new_with_file(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut app = Self::new(cc);
        if let Some(path) = path {
            app.load_file(path);
        }
        app
    }

    /// Применить текущую тему оформления к контексту egui.
    fn apply_theme(&self, ctx: &egui::Context) {
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
    }

    /// Загрузить JSON-файл по указанному пути.
    ///
    /// Читает файл, измеряет время парсинга и сохраняет результат
    /// (корневой узел или ошибку) в состоянии приложения.
    ///
    /// # Errors
    ///
    /// Ошибки чтения файла и парсинга JSON записываются в `self.parse_error`;
    /// метод не возвращает `Result` — ошибки отображаются в UI.
    fn load_file(&mut self, path: PathBuf) {
        let t0 = Instant::now();
        match std::fs::read_to_string(&path) {
            Err(e) => {
                self.parse_error = Some(ParseError {
                    message: format!("Ошибка чтения файла: {}", e),
                    line: None,
                    column: None,
                });
                self.root = None;
            }
            Ok(content) => {
                let size = content.len() as u64;
                match parse_json(&content) {
                    Ok(node) => {
                        self.root = Some(node);
                        self.parse_error = None;
                        self.file_state = FileState {
                            path: Some(path),
                            size_bytes: size,
                            load_time_ms: t0.elapsed().as_millis(),
                        };
                        // Сбрасываем поиск при загрузке нового файла
                        self.search = SearchState::default();
                        self.search_query_buf.clear();
                    }
                    Err(e) => {
                        self.parse_error = Some(e);
                        self.root = None;
                    }
                }
            }
        }
    }

    /// Открыть системный диалог выбора файла и загрузить выбранный JSON.
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON files", &["json"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_file(path);
        }
    }

    /// Сохранить текущий JSON в форматированном виде (pretty-print).
    ///
    /// Открывает диалог сохранения и записывает JSON с отступами 2 пробела.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    fn save_pretty(&mut self) {
        let Some(root) = self.root.as_ref() else {
            return;
        };

        match node_to_value(root).and_then(|value| {
            serde_json::to_string_pretty(&value).map_err(|e| format!("Ошибка сериализации: {}", e))
        }) {
            Ok(formatted) => {
                if let Some(save_path) = rfd::FileDialog::new()
                    .add_filter("JSON files", &["json"])
                    .save_file()
                {
                    match std::fs::write(&save_path, formatted) {
                        Ok(_) => self.show_toast("Файл сохранён"),
                        Err(e) => self.show_toast(&format!("Ошибка сохранения: {}", e)),
                    }
                }
            }
            Err(err) => self.show_toast(&err),
        }
    }

    /// Показать кратковременное уведомление.
    fn show_toast(&mut self, message: &str) {
        self.toast = Some((message.to_string(), Instant::now()));
    }

    /// Отрисовать верхнюю панель с меню и строкой поиска.
    fn show_top_panel(&mut self, ui: &mut Ui) {
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                // ── Меню ──────────────────────────────────────────────────
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
                        if let Some(root) = &mut self.root {
                            set_expanded_all(root, true);
                        }
                    }
                    if ui.button("<  Свернуть все").clicked() {
                        ui.close();
                        if let Some(root) = &mut self.root {
                            set_expanded_all(root, false);
                        }
                    }
                });

                ui.menu_button("Помощь", |ui| {
                    ui.label("JSON Viewer v0.1.0");
                    ui.label("Написан на Rust + egui");
                    ui.separator();
                    ui.label("Drag & Drop файла поддерживается");
                });

                ui.separator();

                // ── Кнопки дерева ─────────────────────────────────────────
                if ui.button(">> Развернуть все").clicked()
                    && let Some(root) = &mut self.root
                {
                    set_expanded_all(root, true);
                }
                if ui.button("<< Свернуть все").clicked()
                    && let Some(root) = &mut self.root
                {
                    set_expanded_all(root, false);
                }

                ui.separator();

                // ── Режим ──────────────────────────────────────────────────
                ui.label("Режим:");
                ui.selectable_value(&mut self.mode, AppMode::View, "Просмотр");
                ui.selectable_value(&mut self.mode, AppMode::Edit, "Редактирование");

                ui.separator();

                // ── Поиск ──────────────────────────────────────────────────
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
                    let q = self.search_query_buf.clone();
                    if let Some(root) = &self.root {
                        self.search.search(root, &q);
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
            });
        });
    }

    /// Отрисовать нижнюю панель (статус-бар).
    fn show_bottom_panel(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.parse_error {
                    ui.label(
                        RichText::new(format!("⚠ Ошибка: {}", err))
                            .color(Color32::from_rgb(220, 80, 80)),
                    );
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

                // Всплывающее уведомление
                if let Some((msg, t)) = &self.toast {
                    if t.elapsed().as_secs() < 3 {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("✔ {}", msg))
                                    .color(Color32::from_rgb(100, 200, 100)),
                            );
                        });
                    } else {
                        self.toast = None;
                    }
                }
            });
        });
    }

    /// Отрисовать центральную панель с деревом JSON.
    fn show_central_panel(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            // Drag-and-Drop
            if !ui.ctx().input(|i| i.raw.dropped_files.is_empty()) {
                let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
                if let Some(file) = files.first()
                    && let Some(path) = &file.path
                {
                    let p = path.clone();
                    self.load_file(p);
                }
            }

            // Подсказка при отсутствии файла
            if self.root.is_none() && self.parse_error.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(
                            "Перетащите JSON-файл сюда\nили используйте Файл -> Открыть…",
                        )
                        .size(18.0)
                        .color(Color32::GRAY),
                    );
                });
                return;
            }

            // Ошибка парсинга
            if let Some(err) = &self.parse_error.clone() {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(220, 80, 80), "Ошибка разбора JSON:");
                ui.add_space(4.0);
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.label(RichText::new(err.to_string()).monospace());
                });
                return;
            }

            // Дерево JSON
            let search = self.search.clone();
            let mut copy_action: Option<String> = None;
            let mut edit_error: Option<String> = None;
            let mut tree_changed = false;

            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if let Some(root) = &mut self.root {
                        render_node(
                            ui,
                            root,
                            &search,
                            self.mode,
                            &mut copy_action,
                            &mut edit_error,
                            &mut tree_changed,
                        );
                    }
                });

            if tree_changed && !self.search_query_buf.is_empty() {
                if let Some(root) = &self.root {
                    let q = self.search_query_buf.clone();
                    self.search.search(root, &q);
                }
            }

            if let Some(text) = copy_action {
                match copy_to_clipboard(&text) {
                    Ok(_) => self.show_toast("Скопировано в буфер обмена"),
                    Err(e) => self.show_toast(&format!("Ошибка копирования: {}", e)),
                }
            }

            if let Some(err) = edit_error {
                self.show_toast(&err);
            }
        });
    }
}

impl eframe::App for JsonViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Принудительное обновление, если показано уведомление (чтобы оно исчезло вовремя)
        if self.toast.is_some() {
            ui.ctx().request_repaint();
        }

        self.show_top_panel(ui);
        self.show_bottom_panel(ui);
        self.show_central_panel(ui);
    }
}

// ─── Рекурсивная отрисовка дерева ────────────────────────────────────────────

/// Рекурсивно отрисовать узел JSON в [`Ui`].
///
/// Объекты и массивы отображаются как раскрывающийся [`egui::CollapsingHeader`].
/// Листовые узлы отображаются как строки с цветной подписью типа.
///
/// При правом клике на узел показывается контекстное меню с опциями копирования.
///
/// # Arguments
///
/// * `ui` — текущий [`Ui`]-контекст egui.
/// * `node` — узел для отрисовки.
/// * `search` — текущее состояние поиска (для подсветки).
/// * `copy_action` — буфер, в который записывается текст для копирования.
fn render_node(
    ui: &mut Ui,
    node: &mut JsonNode,
    search: &SearchState,
    mode: AppMode,
    copy_action: &mut Option<String>,
    edit_error: &mut Option<String>,
    tree_changed: &mut bool,
) {
    let is_match = !search.query.is_empty() && search.is_match(&node.path);
    let is_active = !search.query.is_empty() && search.is_active(&node.path);

    match node.value_type {
        JsonValueType::Object | JsonValueType::Array => {
            // Заголовок раскрывающегося блока
            let header_text = make_header_text(node, is_match, is_active);

            let id = ui.make_persistent_id(&node.path);
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                node.expanded,
            );

            // Если Expand All / Collapse All изменили node.expanded — принудительно
            // обновляем персистентное состояние egui.
            if state.is_open() != node.expanded {
                state.set_open(node.expanded);
                state.store(ui.ctx());
            }

            let (header_resp, _inner, _body) = state
                .show_header(ui, |ui| {
                    ui.label(header_text);
                })
                .body(|ui| {
                    for child in &mut node.children {
                        render_node(ui, child, search, mode, copy_action, edit_error, tree_changed);
                    }
                });

            // Считываем актуальное состояние (пользователь мог кликнуть по заголовку)
            let updated = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                node.expanded,
            );
            node.expanded = updated.is_open();

            // Контекстное меню
            header_resp.context_menu(|ui| {
                context_menu(ui, node, copy_action);
            });
        }

        _ => {
            // Листовой узел
            ui.horizontal(|ui| {
                // Ключ
                if let Some(key) = &node.key {
                    let key_text = if is_active {
                        RichText::new(format!("{}: ", key))
                            .color(COLOR_ACTIVE_MATCH)
                            .strong()
                    } else if is_match {
                        RichText::new(format!("{}: ", key))
                            .color(COLOR_MATCH)
                            .strong()
                    } else {
                        RichText::new(format!("{}: ", key)).color(COLOR_KEY)
                    };
                    let key_resp = ui.label(key_text);
                    // Клик на ключ — контекстное меню
                    key_resp.context_menu(|ui| {
                        context_menu(ui, node, copy_action);
                    });
                }

                // Значение
                let value_color = value_color(&node.value_type);
                let is_editable = mode == AppMode::Edit
                    && matches!(
                        node.value_type,
                        JsonValueType::String
                            | JsonValueType::Number
                            | JsonValueType::Bool
                            | JsonValueType::Null
                    );

                if is_editable {
                    let mut edited = node.display_value.clone();
                    let edit_resp = ui.add(
                        egui::TextEdit::singleline(&mut edited)
                            .desired_width(300.0)
                            .font(egui::TextStyle::Monospace),
                    );

                    if edit_resp.lost_focus() && edited != node.display_value {
                        match apply_primitive_edit(node, &edited) {
                            Ok(()) => {
                                *tree_changed = true;
                            }
                            Err(err) => {
                                *edit_error = Some(err);
                            }
                        }
                    }

                    edit_resp.context_menu(|ui| {
                        context_menu(ui, node, copy_action);
                    });
                } else {
                    let value_text = if is_active {
                        RichText::new(&node.display_value)
                            .color(COLOR_ACTIVE_MATCH)
                            .strong()
                    } else if is_match {
                        RichText::new(&node.display_value).color(COLOR_MATCH)
                    } else {
                        RichText::new(&node.display_value).color(value_color)
                    };
                    let val_resp = ui.label(value_text);
                    val_resp.context_menu(|ui| {
                        context_menu(ui, node, copy_action);
                    });
                }
            });
        }
    }
}

/// Преобразовать JSON-узел в [`serde_json::Value`].
fn node_to_value(node: &JsonNode) -> Result<Value, String> {
    match node.value_type {
        JsonValueType::Object => {
            let mut map = serde_json::Map::new();
            for child in &node.children {
                let key = child.key.clone().unwrap_or_default();
                map.insert(key, node_to_value(child)?);
            }
            Ok(Value::Object(map))
        }
        JsonValueType::Array => {
            let mut values = Vec::with_capacity(node.children.len());
            for child in &node.children {
                values.push(node_to_value(child)?);
            }
            Ok(Value::Array(values))
        }
        JsonValueType::String => {
            let text = serde_json::from_str::<String>(&node.display_value)
                .map_err(|e| format!("Некорректная строка в {}: {}", node.path, e))?;
            Ok(Value::String(text))
        }
        JsonValueType::Number => {
            let number = serde_json::from_str::<serde_json::Number>(&node.display_value)
                .map_err(|e| format!("Некорректное число в {}: {}", node.path, e))?;
            Ok(Value::Number(number))
        }
        JsonValueType::Bool => {
            let boolean = node
                .display_value
                .parse::<bool>()
                .map_err(|e| format!("Некорректное bool в {}: {}", node.path, e))?;
            Ok(Value::Bool(boolean))
        }
        JsonValueType::Null => Ok(Value::Null),
    }
}

/// Применить правку к примитивному узлу (string/number/bool/null).
fn apply_primitive_edit(node: &mut JsonNode, edited: &str) -> Result<(), String> {
    let trimmed = edited.trim();
    if trimmed.is_empty() {
        return Err("Значение не может быть пустым".to_string());
    }

    let new_value = if trimmed == "null" {
        (JsonValueType::Null, "null".to_string())
    } else if trimmed == "true" || trimmed == "false" {
        (JsonValueType::Bool, trimmed.to_string())
    } else if trimmed.starts_with('"') {
        let parsed = serde_json::from_str::<String>(trimmed)
            .map_err(|e| format!("Некорректная строка: {}", e))?;
        let normalized = serde_json::to_string(&parsed)
            .map_err(|e| format!("Ошибка сериализации строки: {}", e))?;
        (JsonValueType::String, normalized)
    } else if let Ok(number) = serde_json::from_str::<serde_json::Number>(trimmed) {
        (JsonValueType::Number, number.to_string())
    } else {
        return Err(
            "Некорректный JSON-литерал. Допустимо: строка в кавычках, число, true/false или null"
                .to_string(),
        );
    };

    node.value_type = new_value.0;
    node.display_value = new_value.1;
    Ok(())
}

/// Сформировать текст заголовка для объекта/массива с учётом подсветки поиска.
fn make_header_text(node: &JsonNode, is_match: bool, is_active: bool) -> RichText {
    let label = match &node.key {
        Some(k) => format!("{}: {}", k, node.display_value),
        None => node.display_value.clone(),
    };
    if is_active {
        RichText::new(label).color(COLOR_ACTIVE_MATCH).strong()
    } else if is_match {
        RichText::new(label).color(COLOR_MATCH).strong()
    } else {
        RichText::new(label).color(COLOR_KEY)
    }
}

/// Контекстное меню для узла с опциями копирования.
fn context_menu(ui: &mut Ui, node: &JsonNode, copy_action: &mut Option<String>) {
    if ui.button("📋  Копировать значение").clicked() {
        *copy_action = Some(node.display_value.clone());
        ui.close();
    }
    if let Some(key) = &node.key
        && ui.button("🔑  Копировать ключ").clicked()
    {
        *copy_action = Some(key.clone());
        ui.close();
    }
    if ui.button("📍  Копировать путь").clicked() {
        *copy_action = Some(node.path.clone());
        ui.close();
    }
}

/// Вернуть цвет для типа значения.
fn value_color(vtype: &JsonValueType) -> Color32 {
    match vtype {
        JsonValueType::String => COLOR_STRING,
        JsonValueType::Number => COLOR_NUMBER,
        JsonValueType::Bool => COLOR_BOOL,
        JsonValueType::Null => COLOR_NULL,
        JsonValueType::Object | JsonValueType::Array => COLOR_KEY,
    }
}
