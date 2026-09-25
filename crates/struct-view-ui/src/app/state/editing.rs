use super::*;

impl StructViewApp {
    /// Показать кратковременное уведомление в статус-баре.
    pub(in crate::app) fn show_toast(&mut self, message: &str) {
        self.toast = Some((message.to_string(), Instant::now()));
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
            self.show_toast(self.locale.text(TextKey::DeleteEditOnly));
            return;
        }
        if self.comparison.is_some()
            || self.visualization != VisualizationMode::Tree
            || self.field_dialog.is_some()
        {
            return;
        }
        if self.selected_paths.is_empty() {
            self.show_toast(self.locale.text(TextKey::SelectStructure));
            return;
        }
        self.finalize_pending_inline_edit();

        let Some(root) = self.root.as_mut() else {
            self.show_toast(self.locale.text(TextKey::NoDocument));
            return;
        };
        let snapshot = root.clone();
        let selected_paths = self.selected_paths.clone();
        let result = delete_selected_structures(root, &selected_paths);

        match result {
            Ok(count) => {
                self.push_undo_snapshot(snapshot);
                self.selected_paths.clear();
                self.visible_rows_dirty = true;
                self.invalidate_visualization_cache();
                self.refresh_search();
                self.show_toast(&self.locale.structures_deleted(count));
            }
            Err(error) => {
                let message = match error {
                    DeleteError::EmptySelection => TextKey::SelectStructure,
                    DeleteError::RootSelected => TextKey::CannotDeleteRoot,
                    DeleteError::SelectionNotFound => TextKey::SelectedStructureNotFound,
                };
                self.show_toast(self.locale.text(message));
            }
        }
    }

    /// Скопировать структуры по указанным путям в системный буфер обмена.
    pub(in crate::app) fn copy_structures_at_paths(&mut self, paths: Vec<String>) {
        let selected_paths = paths.into_iter().collect::<BTreeSet<_>>();
        let entries = match self.root.as_ref() {
            Some(root) => selected_structures(root, &selected_paths),
            None => Err(self.locale.text(TextKey::NoDocument).to_string()),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };

        let encoded = match encode_structures(&entries) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.show_toast(&error);
                return;
            }
        };
        self.clipboard_payload = Some(entries.clone());
        match copy_to_clipboard(&encoded) {
            Ok(()) => self.show_toast(&self.locale.structures_copied(entries.len())),
            Err(error) => self.show_toast(&self.locale.system_copy_error(&error)),
        }
    }

    /// Вставить структуры в единственный выбранный контейнер.
    pub(in crate::app) fn paste_into_selected(&mut self) {
        let Some(path) = self.selected_paths.iter().next().cloned() else {
            self.show_toast(self.locale.text(TextKey::SelectContainer));
            return;
        };
        if self.selected_paths.len() != 1 {
            self.show_toast(self.locale.text(TextKey::SelectOneContainer));
            return;
        }
        self.paste_into_path(path);
    }

    /// Вставить структуры в контейнер по пути.
    pub(in crate::app) fn paste_into_path(&mut self, target_path: String) {
        if self.mode != AppMode::Edit {
            self.show_toast(self.locale.text(TextKey::PasteEditOnly));
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
                self.show_toast(&error);
                return;
            }
        };

        let history_before = self.root.clone();
        let result = self
            .root
            .as_mut()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())
            .and_then(|root| paste_structures_at_path(root, &target_path, &entries));

        match result {
            Ok(count) => {
                if let Some(snapshot) = history_before {
                    self.push_undo_snapshot(snapshot);
                }
                self.retain_valid_selected_paths();
                self.visible_rows_dirty = true;
                self.invalidate_visualization_cache();
                self.refresh_search();
                self.show_toast(&self.locale.structures_pasted(count));
            }
            Err(error) => self.show_toast(&self.locale.paste_error(&error)),
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
        self.field_dialog = Some(request.into());
    }

    /// Открыть конструктор для редактирования существующего узла.
    pub(in crate::app) fn open_edit_field_dialog(&mut self, request: EditFieldRequest) {
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
                self.show_toast(&error);
                return;
            }
        };

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

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
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
                        .selected_text(field_type_label(locale, &dialog.value_type))
                        .show_ui(ui, |ui| {
                            for value_type in
                                field_value_types(format, is_toml_root, is_comment_edit)
                            {
                                ui.selectable_value(
                                    &mut dialog.value_type,
                                    value_type.clone(),
                                    field_type_label(locale, &value_type),
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
                                .text_color(value_color(&dialog.value_type)),
                        );
                    }
                    JsonValueType::Comment => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::multiline(&mut dialog.value)
                                .desired_width(420.0)
                                .desired_rows(4)
                                .text_color(value_color(&dialog.value_type))
                                .hint_text(locale.text(TextKey::CommentHint)),
                        );
                    }
                    JsonValueType::Metadata => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::multiline(&mut dialog.value)
                                .desired_width(420.0)
                                .desired_rows(4)
                                .text_color(value_color(&dialog.value_type))
                                .hint_text(locale.text(TextKey::MetadataHint)),
                        );
                    }
                    JsonValueType::DateTime => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.value)
                                .desired_width(320.0)
                                .font(egui::TextStyle::Monospace)
                                .text_color(value_color(&dialog.value_type))
                                .hint_text("1979-05-27T07:32:00Z"),
                        );
                    }
                    JsonValueType::Number | JsonValueType::Float => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.value)
                                .desired_width(320.0)
                                .font(egui::TextStyle::Monospace)
                                .text_color(value_color(&dialog.value_type)),
                        );
                    }
                    JsonValueType::Bool => {
                        ui.horizontal(|ui| {
                            ui.label(locale.text(TextKey::Value));
                            egui::ComboBox::from_id_salt("field_dialog_bool")
                                .selected_text(
                                    egui::RichText::new(&dialog.value)
                                        .color(value_color(&dialog.value_type)),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "true".to_string(),
                                        egui::RichText::new("true")
                                            .color(value_color(&dialog.value_type)),
                                    );
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "false".to_string(),
                                        egui::RichText::new("false")
                                            .color(value_color(&dialog.value_type)),
                                    );
                                });
                        });
                    }
                    JsonValueType::Null => {
                        ui.horizontal(|ui| {
                            ui.label(locale.text(TextKey::Value));
                            ui.colored_label(value_color(&dialog.value_type), "null");
                        });
                    }
                    JsonValueType::Object => {
                        ui.colored_label(
                            value_color(&dialog.value_type),
                            locale.text(TextKey::EmptyObject),
                        );
                    }
                    JsonValueType::Array => {
                        ui.colored_label(
                            value_color(&dialog.value_type),
                            locale.text(TextKey::EmptyArray),
                        );
                    }
                }

                if let Some(error) = &dialog.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
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
            });

        if cancel {
            return;
        }
        if !submit {
            self.field_dialog = Some(dialog);
            return;
        }

        let target = dialog.target.clone();
        let history_before = self.root.clone();
        let result = self
            .root
            .as_mut()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())
            .and_then(|root| match target {
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
                if let Some(snapshot) = history_before {
                    self.push_undo_snapshot(snapshot);
                }
                if is_edit {
                    self.retain_valid_selected_paths();
                }
                self.visible_rows_dirty = true;
                self.invalidate_visualization_cache();
                self.refresh_search();
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
