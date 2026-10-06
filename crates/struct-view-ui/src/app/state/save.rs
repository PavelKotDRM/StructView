use super::*;

impl StructViewApp {
    /// Сохранить текущие данные в форматированном виде.
    ///
    /// Формат результата определяется по расширению выбранного файла.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    pub(in crate::app) fn save_as(&mut self) -> bool {
        if self.root.is_none() {
            return false;
        }

        let current_format = self
            .file_state
            .format
            .filter(|format| format.is_serializable())
            .unwrap_or(DataFormat::Json);
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"]);
        let suggested_name = if self
            .file_state
            .format
            .is_some_and(|format| !format.is_serializable())
        {
            Some(self.conversion_file_name(current_format))
        } else {
            self.file_state
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_string)
        };
        if let Some(name) = suggested_name {
            dialog = dialog.set_file_name(&name);
        }
        if let Some(save_path) = dialog.save_file() {
            let format = DataFormat::from_path(&save_path).unwrap_or(current_format);
            match self.save_document_to_path(save_path, format) {
                Ok(()) => {
                    self.show_toast(self.locale.text(TextKey::FileSaved));
                    true
                }
                Err(error) => {
                    self.show_error(&error);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Преобразовать открытый документ в выбранный формат и сохранить его
    /// отдельным файлом.
    ///
    /// Исходный документ остаётся открытым. Расширение результата определяется
    /// выбранным форматом, даже если пользователь ввёл в диалоге другое
    /// расширение.
    pub(in crate::app) fn convert_to_format(&mut self, format: DataFormat) {
        if self.root.is_none() {
            return;
        }

        let format_name = format.to_string();
        let mut dialog = rfd::FileDialog::new().add_filter(&format_name, format.extensions());
        let file_name = self.conversion_file_name(format);
        dialog = dialog.set_file_name(&file_name);

        if let Some(save_path) = dialog.save_file() {
            let save_path = with_format_extension(save_path, format);
            match self.write_root_to_path(&save_path, format) {
                Ok(_) => self.show_toast(self.locale.text(TextKey::FileConverted)),
                Err(error) => self.show_error(&error),
            }
        }
    }

    /// Сохранить текущие данные в открытый файл без запроса нового пути.
    ///
    /// Если файл ещё не был сохранён, открывается диалог «Сохранить как…».
    pub(in crate::app) fn save_current(&mut self) -> bool {
        let Some(path) = self.file_state.path.clone() else {
            return self.save_as();
        };

        let format = DataFormat::from_path(&path)
            .or(self.file_state.format)
            .unwrap_or(DataFormat::Json);
        match self.save_document_to_path(path, format) {
            Ok(()) => {
                self.show_toast(self.locale.text(TextKey::FileSaved));
                true
            }
            Err(error) => {
                self.show_error(&error);
                false
            }
        }
    }

    pub(super) fn save_document_to_path(
        &mut self,
        path: PathBuf,
        format: DataFormat,
    ) -> Result<(), String> {
        let (size_bytes, saved_content_fingerprint) =
            self.write_root_to_path_with_fingerprint(&path, format)?;
        self.file_state.path = Some(path);
        self.file_state.format = Some(format);
        self.file_state.size_bytes = size_bytes;
        self.file_state.saved_content_fingerprint = Some(saved_content_fingerprint);
        Ok(())
    }

    /// Отложить сохранение до завершения текущей отрисовки дерева.
    pub(in crate::app) fn request_save_current(&mut self) {
        self.save_requested = true;
    }

    pub(in crate::app) fn request_close_file(&mut self) {
        self.exit_after_close_confirmation = false;
        if self.has_unsaved_changes_before_close() {
            self.close_file_confirmation_open = true;
        } else {
            self.close_file_after_confirmation();
        }
    }

    pub(in crate::app) fn request_exit(&mut self, ctx: &egui::Context) {
        if self.has_unsaved_changes_before_exit() {
            self.close_file_confirmation_open = true;
            self.exit_after_close_confirmation = true;
        } else {
            self.close_file_confirmation_open = false;
            self.exit_after_close_confirmation = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(in crate::app) fn cancel_close_file_confirmation(&mut self) {
        self.close_file_confirmation_open = false;
        self.exit_after_close_confirmation = false;
    }

    pub(in crate::app) fn close_file_without_saving(&mut self) {
        self.close_file_after_confirmation();
    }

    pub(in crate::app) fn continue_without_saving(&mut self, ctx: &egui::Context) {
        if self.exit_after_close_confirmation {
            self.close_file_confirmation_open = false;
            self.exit_after_close_confirmation = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.close_file_without_saving();
        }
    }

    pub(in crate::app) fn save_changes_and_close_file(&mut self) -> bool {
        let exiting = self.exit_after_close_confirmation;
        let previous_document_changed = exiting && self.has_unsaved_previous_document_changes();
        let document_changed = self.has_unsaved_document_changes() || previous_document_changed;
        let structure_changed = (exiting || self.visualization == VisualizationMode::Structure)
            && self.structure_view.has_unsaved_changes();
        if document_changed && structure_changed {
            self.show_error(self.locale.text(TextKey::SaveViewsSeparately));
            return false;
        }

        if previous_document_changed {
            self.restore_previous_document_for_exit();
        }

        let document_changed = self.has_unsaved_document_changes();
        let structure_changed = (exiting || self.visualization == VisualizationMode::Structure)
            && self.structure_view.has_unsaved_changes();
        let saved = if document_changed {
            self.save_current()
        } else if structure_changed {
            self.structure_view.save_changes(self.locale)
        } else {
            true
        };

        if saved {
            self.close_file_after_confirmation();
        }
        saved
    }

    fn has_unsaved_changes_before_close(&self) -> bool {
        self.has_unsaved_document_changes()
            || (self.comparison.is_none()
                && self.visualization == VisualizationMode::Structure
                && self.structure_view.has_unsaved_changes())
    }

    fn has_unsaved_changes_before_exit(&self) -> bool {
        self.has_unsaved_document_changes()
            || self.has_unsaved_previous_document_changes()
            || self.structure_view.has_unsaved_changes()
    }

    fn has_unsaved_document_changes(&self) -> bool {
        if self.pending_inline_edit.is_some() {
            return true;
        }

        self.root.as_ref().is_some_and(|root| {
            Self::document_state_has_unsaved_changes(root, &self.file_state, &self.undo_history)
        })
    }

    fn has_unsaved_previous_document_changes(&self) -> bool {
        self.comparison
            .as_ref()
            .and_then(|comparison| comparison.previous_document.as_ref())
            .is_some_and(|previous| {
                Self::document_state_has_unsaved_changes(
                    &previous.root,
                    &previous.file_state,
                    &previous.undo_history,
                )
            })
    }

    fn document_state_has_unsaved_changes(
        root: &JsonNode,
        file_state: &FileState,
        undo_history: &[JsonNode],
    ) -> bool {
        let current_fingerprint = file_state
            .format
            .and_then(|format| super::document_content_fingerprint(root, format));
        match (file_state.saved_content_fingerprint, current_fingerprint) {
            (Some(saved), Some(current)) => saved != current,
            _ => !undo_history.is_empty(),
        }
    }

    fn restore_previous_document_for_exit(&mut self) {
        if let Some(previous_document) = self
            .comparison
            .take()
            .and_then(|comparison| comparison.previous_document)
        {
            self.restore_previous_document(previous_document);
        }
    }

    fn close_file_after_confirmation(&mut self) {
        self.close_file_confirmation_open = false;
        self.exit_after_close_confirmation = false;
        if self.comparison.is_none() && self.visualization == VisualizationMode::Structure {
            self.structure_view = crate::app::views::structure::StructureView::default();
        }
        self.close_file();
    }

    /// Закрыть сравнение или текущий документ и очистить связанные состояния.
    ///
    /// Если сравнение было открыто из документа, вместо очистки восстанавливает
    /// этот документ.
    pub(in crate::app) fn close_file(&mut self) {
        self.close_file_confirmation_open = false;
        self.exit_after_close_confirmation = false;
        if let Some(previous_document) = self
            .comparison
            .take()
            .and_then(|comparison| comparison.previous_document)
        {
            self.restore_previous_document(previous_document);
            return;
        }

        self.clear_document_state();
    }

    pub(super) fn clear_document_state(&mut self) {
        self.clear_history();
        self.file_load_receiver = None;
        self.root = None;
        self.comparison = None;
        self.visualization = VisualizationMode::Tree;
        self.visualization_cache = VisualizationCache::default();
        self.graph_calculation = super::super::views::GraphCalculationState::default();
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.parse_error = None;
        self.search = SearchState::default();
        self.search_query_buf.clear();
        self.regex_builder_literal.clear();
        self.search_window_open = false;
        self.search_scroll_target = None;
        self.save_requested = false;
        self.close_file_confirmation_open = false;
        self.exit_after_close_confirmation = false;
        self.file_state = FileState::default();
        self.field_dialog = None;
        self.mode = AppMode::View;
        self.selected_paths.clear();
        self.copy_structures_requested = false;
        self.paste_requested = false;
        self.toast = None;
    }

    fn restore_previous_document(&mut self, previous_document: PreviousDocumentState) {
        self.clear_history();
        self.file_load_receiver = None;
        self.root = Some(previous_document.root);
        self.file_state = previous_document.file_state;
        self.comparison = None;
        self.visualization = previous_document.visualization;
        self.visualization_cache = VisualizationCache::default();
        self.graph_calculation = super::super::views::GraphCalculationState::default();
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.parse_error = None;
        self.search = previous_document.search;
        self.search_query_buf = previous_document.search_query_buf;
        self.regex_builder_literal = previous_document.regex_builder_literal;
        self.search_window_open = previous_document.search_window_open;
        self.search_scroll_target = previous_document.search_scroll_target;
        self.save_requested = false;
        self.field_dialog = None;
        self.mode = previous_document.mode;
        self.selected_paths = previous_document.selected_paths;
        self.undo_history = previous_document.undo_history;
        self.redo_history = previous_document.redo_history;
        self.copy_structures_requested = false;
        self.paste_requested = false;
    }

    /// Проверить, отображается ли сейчас режим сравнения.
    pub(in crate::app) fn is_comparing(&self) -> bool {
        self.comparison.is_some()
    }

    /// Сериализовать корень и записать его в указанный путь.
    pub(super) fn write_root_to_path(
        &mut self,
        path: &Path,
        format: DataFormat,
    ) -> Result<u64, String> {
        self.write_root_to_path_with_fingerprint(path, format)
            .map(|(size_bytes, _)| size_bytes)
    }

    fn write_root_to_path_with_fingerprint(
        &mut self,
        path: &Path,
        format: DataFormat,
    ) -> Result<(u64, u64), String> {
        self.commit_pending_inline_edit()?;
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
        self.write_node_to_path(root, path, format)
    }

    pub(super) fn write_node_to_path(
        &self,
        root: &JsonNode,
        path: &Path,
        format: DataFormat,
    ) -> Result<(u64, u64), String> {
        let formatted = serialize_node(root, format, false)?;
        let fingerprint = super::content_fingerprint(&formatted);
        let size_bytes = formatted.len() as u64;
        write_text_atomic(path, &formatted)
            .map_err(|error| self.locale.save_error(&error.to_string()))?;
        Ok((size_bytes, fingerprint))
    }

    /// Сформировать имя результата преобразования рядом с именем открытого
    /// файла, заменив его расширение на расширение целевого формата.
    fn conversion_file_name(&self, format: DataFormat) -> String {
        let stem = self
            .file_state
            .path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("converted");
        format!("{}.{}", stem, format.extension())
    }
}
