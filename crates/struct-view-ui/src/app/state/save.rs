use super::*;

impl StructViewApp {
    /// Сохранить текущие данные в форматированном виде.
    ///
    /// Формат результата определяется по расширению выбранного файла.
    ///
    /// # Errors
    ///
    /// Ошибки записи файла отображаются во всплывающем уведомлении.
    pub(in crate::app) fn save_pretty(&mut self) {
        if self.root.is_none() {
            return;
        }

        let current_format = self.file_state.format.unwrap_or(DataFormat::Json);
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"]);
        if let Some(name) = self
            .file_state
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
        {
            dialog = dialog.set_file_name(name);
        }
        if let Some(save_path) = dialog.save_file() {
            let format = DataFormat::from_path(&save_path).unwrap_or(current_format);
            match self.write_root_to_path(&save_path, format) {
                Ok(_) => self.show_toast(self.locale.text(TextKey::FileSaved)),
                Err(error) => self.show_toast(&error),
            }
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
                Err(error) => self.show_toast(&error),
            }
        }
    }

    /// Сохранить текущие данные в открытый файл без запроса нового пути.
    ///
    /// Если файл ещё не был сохранён, открывается диалог «Сохранить как…».
    pub(in crate::app) fn save_current(&mut self) {
        let Some(path) = self.file_state.path.clone() else {
            self.save_pretty();
            return;
        };

        let format = DataFormat::from_path(&path)
            .or(self.file_state.format)
            .unwrap_or(DataFormat::Json);
        match self.write_root_to_path(&path, format) {
            Ok(size_bytes) => {
                self.file_state.size_bytes = size_bytes;
                self.show_toast(self.locale.text(TextKey::FileSaved));
            }
            Err(error) => self.show_toast(&error),
        }
    }

    /// Отложить сохранение до завершения текущей отрисовки дерева.
    pub(in crate::app) fn request_save_current(&mut self) {
        self.save_requested = true;
    }

    /// Закрыть сравнение или текущий документ и очистить связанные состояния.
    ///
    /// Если сравнение было открыто из документа, вместо очистки восстанавливает
    /// этот документ.
    pub(in crate::app) fn close_file(&mut self) {
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
        self.root = None;
        self.comparison = None;
        self.visualization = VisualizationMode::Tree;
        self.visualization_cache = VisualizationCache::default();
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.parse_error = None;
        self.search = SearchState::default();
        self.search_query_buf.clear();
        self.search_scroll_target = None;
        self.save_requested = false;
        self.file_state = FileState::default();
        self.field_dialog = None;
        self.mode = AppMode::View;
        self.selected_paths.clear();
        self.copy_structures_requested = false;
        self.paste_requested = false;
    }

    fn restore_previous_document(&mut self, previous_document: PreviousDocumentState) {
        self.clear_history();
        self.root = Some(previous_document.root);
        self.file_state = previous_document.file_state;
        self.comparison = None;
        self.visualization = previous_document.visualization;
        self.visualization_cache = VisualizationCache::default();
        self.visible_rows = VisibleRows::default();
        self.visible_rows_dirty = true;
        self.parse_error = None;
        self.search = previous_document.search;
        self.search_query_buf = previous_document.search_query_buf;
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
        &self,
        path: &Path,
        format: DataFormat,
    ) -> Result<u64, String> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| self.locale.text(TextKey::NoDocument).to_string())?;
        let formatted = serialize_node(root, format, false)?;
        let size_bytes = formatted.len() as u64;
        std::fs::write(path, formatted)
            .map_err(|error| self.locale.save_error(&error.to_string()))?;
        Ok(size_bytes)
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
