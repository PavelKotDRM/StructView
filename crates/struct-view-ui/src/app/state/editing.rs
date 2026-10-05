use super::*;

mod search;

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
}
