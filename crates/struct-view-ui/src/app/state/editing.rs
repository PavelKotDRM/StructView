use super::*;
use crate::app::docking::{DetachedViewportAction, show_detached_viewport, show_docking_controls};

impl StructViewApp {
    /// Показать кратковременное уведомление в статус-баре.
    pub(in crate::app) fn show_toast(&mut self, message: &str) {
        self.toast = Some(Toast {
            message: message.to_string(),
            shown_at: Instant::now(),
            kind: ToastKind::Success,
        });
    }

    pub(in crate::app) fn show_error(&mut self, message: &str) {
        self.toast = Some(Toast {
            message: message.to_string(),
            shown_at: Instant::now(),
            kind: ToastKind::Error,
        });
    }

    pub(super) fn apply_document_change<T>(
        &mut self,
        change: impl FnOnce(&mut JsonNode, DataFormat) -> Result<T, String>,
    ) -> Result<T, String> {
        self.commit_pending_inline_edit()?;
        let format = self.file_state.format.unwrap_or(DataFormat::Json);
        let root = self
            .root
            .as_mut()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
        let before = root.clone();
        let result = change(root, format)
            .and_then(|value| serialize_node(root, format, true).map(|_| value));
        match result {
            Ok(value) => {
                self.push_undo_snapshot(before);
                self.retain_valid_selected_paths();
                self.visible_rows_dirty = true;
                self.invalidate_visualization_cache();
                self.refresh_search();
                Ok(value)
            }
            Err(error) => {
                self.root = Some(before);
                Err(error)
            }
        }
    }

    /// Применить к текущему выбору действие клика по узлу.
    pub(in crate::app) fn apply_selection_request(&mut self, request: SelectionRequest) {
        if !request.additive {
            self.selected_paths.clear();
            self.selected_paths.insert(request.path);
            return;
        }

        if !self.selected_paths.insert(request.path.clone()) {
            self.selected_paths.remove(&request.path);
        }
    }

    /// Проверить, можно ли вставить структуры в текущий выбор.
    pub(in crate::app) fn can_paste_into_selected(&self) -> bool {
        let Some(path) = self.selected_paths.iter().next() else {
            return false;
        };
        self.selected_paths.len() == 1
            && self
                .root
                .as_ref()
                .and_then(|root| find_node(root, path))
                .is_some_and(|node| {
                    matches!(
                        node.value_type,
                        JsonValueType::Object | JsonValueType::Array
                    )
                })
    }

    /// Проверить, можно ли удалить текущий выбор.
    pub(in crate::app) fn can_delete_selected(&self) -> bool {
        self.mode == AppMode::Edit
            && self.comparison.is_none()
            && self.visualization == VisualizationMode::Tree
            && self.field_dialog.is_none()
            && self.root.is_some()
            && !self.selected_paths.is_empty()
    }

    /// Удалить выбранные структуры с сохранением возможности отмены.
    pub(in crate::app) fn delete_selected(&mut self) {
        if self.mode != AppMode::Edit {
            self.show_error(self.locale.text(TextKey::DeleteEditOnly));
            return;
        }
        if self.comparison.is_some()
            || self.visualization != VisualizationMode::Tree
            || self.field_dialog.is_some()
        {
            return;
        }
        if self.selected_paths.is_empty() {
            self.show_error(self.locale.text(TextKey::SelectStructure));
            return;
        }
        let selected_paths = self.selected_paths.clone();
        let locale = self.locale;
        let result = self.apply_document_change(|root, _| {
            delete_selected_structures(root, &selected_paths).map_err(|error| {
                let message = match error {
                    DeleteError::EmptySelection => TextKey::SelectStructure,
                    DeleteError::RootSelected => TextKey::CannotDeleteRoot,
                    DeleteError::MetadataValueSelected => TextKey::CannotDeleteMetadataValue,
                    DeleteError::SelectionNotFound => TextKey::SelectedStructureNotFound,
                };
                locale.text(message).to_string()
            })
        });

        match result {
            Ok(count) => {
                self.selected_paths.clear();
                self.show_toast(&self.locale.structures_deleted(count));
            }
            Err(error) => {
                self.show_error(&error);
            }
        }
    }

    /// Скопировать структуры по указанным путям в системный буфер обмена.
    pub(in crate::app) fn copy_structures_at_paths(&mut self, paths: Vec<String>) {
        if let Err(error) = self.commit_pending_inline_edit() {
            self.show_error(&error);
            return;
        }
        let selected_paths = paths.into_iter().collect::<BTreeSet<_>>();
        let entries = match self.root.as_ref() {
            Some(root) => selected_structures(root, &selected_paths),
            None => Err(self.locale.text(TextKey::NoDocument).to_string()),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };

        let encoded = match encode_structures(&entries) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };
        self.clipboard_payload = Some(entries.clone());
        match copy_to_clipboard(&encoded) {
            Ok(()) => self.show_toast(&self.locale.structures_copied(entries.len())),
            Err(error) => self.show_error(&self.locale.system_copy_error(&error)),
        }
    }

    /// Вставить структуры в единственный выбранный контейнер.
    pub(in crate::app) fn paste_into_selected(&mut self) {
        let Some(path) = self.selected_paths.iter().next().cloned() else {
            self.show_error(self.locale.text(TextKey::SelectContainer));
            return;
        };
        if self.selected_paths.len() != 1 {
            self.show_error(self.locale.text(TextKey::SelectOneContainer));
            return;
        }
        self.paste_into_path(path);
    }

    /// Вставить структуры в контейнер по пути.
    pub(in crate::app) fn paste_into_path(&mut self, target_path: String) {
        if self.mode != AppMode::Edit {
            self.show_error(self.locale.text(TextKey::PasteEditOnly));
            return;
        }

        let entries = match read_from_clipboard() {
            Ok(text) => decode_structures(&text),
            Err(system_error) => self
                .clipboard_payload
                .clone()
                .ok_or_else(|| self.locale.clipboard_read_error(&system_error)),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };

        let result = self.apply_document_change(|root, _| {
            paste_structures_at_path(root, &target_path, &entries)
        });

        match result {
            Ok(count) => {
                self.show_toast(&self.locale.structures_pasted(count));
            }
            Err(error) => self.show_error(&self.locale.paste_error(&error)),
        }
    }

    pub(super) fn retain_valid_selected_paths(&mut self) {
        if let Some(root) = &self.root {
            self.selected_paths
                .retain(|path| find_node(root, path).is_some());
        } else {
            self.selected_paths.clear();
        }
    }

    /// Пересчитать результаты поиска по текущему запросу.
    ///
    /// Вызывается после правки дерева, чтобы подсветка оставалась актуальной.
    pub(in crate::app) fn refresh_search(&mut self) {
        self.search_scroll_target = None;
        let Some(root) = &self.root else {
            return;
        };

        let query = self.search_query_buf.clone();
        self.search.search(root, &query);
    }

    /// Сбросить производные модели после изменения документа.
    pub(in crate::app) fn invalidate_visualization_cache(&mut self) {
        self.visualization_cache = VisualizationCache::default();
    }

    /// Запланировать прокрутку к текущему совпадению, если оно существует.
    pub(in crate::app) fn request_search_scroll(&mut self) {
        self.search_scroll_target = self.search.current_match_path().map(str::to_owned);
    }

    /// Открыть диалог добавления данных в выбранный контейнер.
    pub(in crate::app) fn open_add_child_dialog(&mut self, request: AddChildRequest) {
        if let Err(error) = self.commit_pending_inline_edit() {
            self.show_error(&error);
            return;
        }
        self.field_dialog_docking.reset_if_active();
        self.field_dialog = Some(request.into());
    }

    /// Открыть конструктор для редактирования существующего узла.
    pub(in crate::app) fn open_edit_field_dialog(&mut self, request: EditFieldRequest) {
        if let Err(error) = self.commit_pending_inline_edit() {
            self.show_error(&error);
            return;
        }
        let result: Result<(String, JsonValueType, String, bool), String> = (|| {
            let root = self
                .root
                .as_ref()
                .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
            let node = find_node(root, &request.path)
                .ok_or_else(|| "Не удалось найти поле для редактирования".to_string())?;
            let value_type = node.value_type.clone();
            let value = match value_type {
                JsonValueType::String => serde_json::from_str::<String>(&node.display_value)
                    .map_err(|error| format!("Некорректная строка: {}", error))?,
                JsonValueType::DateTime => node.display_value.clone(),
                JsonValueType::Comment => comment_input(&node.display_value),
                JsonValueType::Metadata => serialize_node(node, DataFormat::Yaml, true)?,
                _ => node.display_value.clone(),
            };
            Ok((
                node.key.clone().unwrap_or_default(),
                node.value_type.clone(),
                value,
                node.value_type != JsonValueType::Comment && is_object_child(root, &request.path),
            ))
        })();

        let (key, value_type, value, key_editable) = match result {
            Ok(data) => data,
            Err(error) => {
                self.show_error(&error);
                return;
            }
        };

        self.field_dialog_docking.reset_if_active();
        self.field_dialog = Some(FieldDialog {
            target: FieldDialogTarget::Edit {
                path: request.path,
                key_editable,
            },
            key,
            value_type,
            value,
            error: None,
        });
    }

    /// Отрисовать конструктор и добавить или изменить узел после подтверждения.
    pub(in crate::app) fn show_field_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.field_dialog.take() else {
            self.field_dialog_docking.reset_if_active();
            return;
        };

        let mut submit = false;
        let mut cancel = false;
        let locale = self.locale;
        let format = self.file_state.format.unwrap_or(DataFormat::Json);
        let is_toml_root = format == DataFormat::Toml
            && matches!(&dialog.target, FieldDialogTarget::Edit { path, .. } if path.is_empty());
        let is_edit = matches!(&dialog.target, FieldDialogTarget::Edit { .. });
        let is_comment_edit = is_edit && dialog.value_type == JsonValueType::Comment;
        let show_key_input = match &dialog.target {
            FieldDialogTarget::Add { is_object, .. } => *is_object,
            FieldDialogTarget::Edit { key_editable, .. } => *key_editable,
        } && dialog.value_type != JsonValueType::Comment;
        let show_readonly_key = is_edit && !show_key_input && !dialog.key.is_empty();
        let title = match &dialog.target {
            FieldDialogTarget::Add {
                is_object: true, ..
            } => locale.text(TextKey::AddFieldTitle),
            FieldDialogTarget::Add {
                is_object: false, ..
            } => locale.text(TextKey::AddElementTitle),
            FieldDialogTarget::Edit { .. } => locale.text(TextKey::EditFieldTitle),
        };

        let mut docking = std::mem::take(&mut self.field_dialog_docking);
        docking.begin();
        let viewport_id = egui::ViewportId::from_hash_of("field_dialog");
        let mut show_content = |ui: &mut egui::Ui| {
            let colors = SyntaxColors::new(ui.visuals());

            if show_key_input {
                ui.label(locale.text(TextKey::FieldName));
                ui.add(egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0));
            } else if show_readonly_key {
                ui.horizontal(|ui| {
                    ui.label(locale.text(TextKey::FieldName));
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0),
                    );
                });
            }

            ui.horizontal(|ui| {
                ui.label(locale.text(TextKey::FieldType));
                let previous_type = dialog.value_type.clone();
                egui::ComboBox::from_id_salt("field_dialog_type")
                    .selected_text(locale.value_type_label(&dialog.value_type))
                    .show_ui(ui, |ui| {
                        for value_type in field_value_types(format, is_toml_root, is_comment_edit) {
                            ui.selectable_value(
                                &mut dialog.value_type,
                                value_type.clone(),
                                locale.value_type_label(&value_type),
                            );
                        }
                    });
                if dialog.value_type != previous_type {
                    dialog.value = default_field_value(&dialog.value_type);
                }
            });

            match &dialog.value_type {
                JsonValueType::String => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type)),
                    );
                }
                JsonValueType::Comment => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text(locale.text(TextKey::CommentHint)),
                    );
                }
                JsonValueType::Metadata => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text(locale.text(TextKey::MetadataHint)),
                    );
                }
                JsonValueType::DateTime => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.value)
                            .desired_width(320.0)
                            .font(egui::TextStyle::Monospace)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text("1979-05-27T07:32:00Z"),
                    );
                }
                JsonValueType::Number | JsonValueType::Float => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.value)
                            .desired_width(320.0)
                            .font(egui::TextStyle::Monospace)
                            .text_color(colors.value_color(&dialog.value_type)),
                    );
                }
                JsonValueType::Bool => {
                    ui.horizontal(|ui| {
                        ui.label(locale.text(TextKey::Value));
                        egui::ComboBox::from_id_salt("field_dialog_bool")
                            .selected_text(
                                egui::RichText::new(&dialog.value)
                                    .color(colors.value_color(&dialog.value_type)),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut dialog.value,
                                    "true".to_string(),
                                    egui::RichText::new("true")
                                        .color(colors.value_color(&dialog.value_type)),
                                );
                                ui.selectable_value(
                                    &mut dialog.value,
                                    "false".to_string(),
                                    egui::RichText::new("false")
                                        .color(colors.value_color(&dialog.value_type)),
                                );
                            });
                    });
                }
                JsonValueType::Null => {
                    ui.horizontal(|ui| {
                        ui.label(locale.text(TextKey::Value));
                        ui.colored_label(colors.value_color(&dialog.value_type), "null");
                    });
                }
                JsonValueType::Object => {
                    ui.colored_label(
                        colors.value_color(&dialog.value_type),
                        locale.text(TextKey::EmptyObject),
                    );
                }
                JsonValueType::Array => {
                    ui.colored_label(
                        colors.value_color(&dialog.value_type),
                        locale.text(TextKey::EmptyArray),
                    );
                }
            }

            if let Some(error) = &dialog.error {
                ui.colored_label(colors.error, error);
            }
            ui.horizontal(|ui| {
                let action = if is_edit {
                    TextKey::Apply
                } else {
                    TextKey::Add
                };
                if ui.button(locale.text(action)).clicked() {
                    submit = true;
                }
                if ui.button(locale.text(TextKey::Cancel)).clicked() {
                    cancel = true;
                }
            });
        };

        let action = if docking.detached {
            show_detached_viewport(
                ctx,
                &mut docking,
                viewport_id,
                title,
                egui::vec2(460.0, 320.0),
                locale.text(TextKey::DockWindow),
                |ui| show_content(ui),
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(title)
                .id(docking.area_id("field_dialog"))
                .open(&mut open)
                .collapsible(false)
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
        self.field_dialog_docking = docking;

        if action == DetachedViewportAction::Closed {
            cancel = true;
        }
        if cancel {
            self.field_dialog_docking.reset_if_active();
            return;
        }
        if !submit {
            self.field_dialog = Some(dialog);
            return;
        }

        let target = dialog.target.clone();
        let result = self.apply_document_change(|root, format| match target {
            FieldDialogTarget::Add { parent_path, .. } => add_typed_child_at_path(
                root,
                &parent_path,
                &dialog.key,
                &dialog.value_type,
                &dialog.value,
                format,
            ),
            FieldDialogTarget::Edit { path, key_editable } => edit_child_at_path(
                root,
                &path,
                key_editable.then_some(dialog.key.as_str()),
                &dialog.value_type,
                &dialog.value,
                format,
            ),
        });

        match result {
            Ok(()) => {
                self.field_dialog_docking.reset_if_active();
                let message = if is_edit {
                    TextKey::FieldUpdated
                } else {
                    TextKey::DataAdded
                };
                self.show_toast(self.locale.text(message));
            }
            Err(error) => {
                dialog.error = Some(error);
                self.field_dialog = Some(dialog);
            }
        }
    }
}
