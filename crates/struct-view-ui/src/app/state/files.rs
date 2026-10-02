use super::*;

use std::sync::mpsc::{self, TryRecvError};
use std::thread;

impl StructViewApp {
    /// Загрузить файл поддерживаемого формата по указанному пути.
    ///
    /// Читает файл, измеряет время парсинга и сохраняет результат
    /// (корневой узел или ошибку) в состоянии приложения.
    ///
    /// # Errors
    ///
    /// Ошибки чтения файла и парсинга записываются в `self.parse_error`;
    /// метод не возвращает `Result` — ошибки отображаются в UI.
    #[cfg(test)]
    pub(in crate::app) fn load_file(&mut self, path: PathBuf) {
        self.file_load_receiver = None;
        let loaded = read_document(path, self.locale);
        self.apply_loaded_document(loaded);
    }

    pub(in crate::app) fn request_file_load(&mut self, path: PathBuf) {
        self.file_load_receiver = None;
        self.parse_error = None;
        let locale = self.locale;
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("struct-view-file-loader".to_string())
            .spawn(move || {
                let _ = sender.send(read_document(path, locale));
            });

        match worker {
            Ok(_) => self.file_load_receiver = Some(receiver),
            Err(error) => self.show_error(&format!(
                "{} {error}",
                locale.text(TextKey::BackgroundOperationFailed)
            )),
        }
    }

    pub(super) fn poll_file_load(&mut self, ctx: &egui::Context) {
        let result = match self.file_load_receiver.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => return,
        };
        match result {
            Ok(loaded) => {
                self.file_load_receiver = None;
                self.apply_loaded_document(loaded);
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            Err(TryRecvError::Disconnected) => {
                self.file_load_receiver = None;
                self.report_document_load_error(ParseError {
                    message: self
                        .locale
                        .text(TextKey::BackgroundOperationFailed)
                        .to_string(),
                    line: None,
                    column: None,
                });
            }
        }
    }

    fn apply_loaded_document(&mut self, loaded: Result<LoadedDocument, ParseError>) {
        match loaded {
            Ok(document) => {
                let mode = self.mode;
                self.clear_document_state();
                self.mode = mode;
                self.root = Some(document.root);
                self.visible_rows = document.visible_rows;
                self.visible_rows_dirty = false;
                self.file_state = FileState {
                    path: Some(document.path),
                    size_bytes: document.size_bytes,
                    load_time_ms: document.load_time_ms,
                    format: Some(document.format),
                };
            }
            Err(error) => self.report_document_load_error(error),
        }
    }

    fn report_document_load_error(&mut self, error: ParseError) {
        if self.root.is_some() || self.comparison.is_some() {
            self.show_error(&error.to_string());
        } else {
            self.clear_document_state();
            self.parse_error = Some(error);
        }
    }

    /// Открыть системный диалог выбора и загрузить выбранный файл.
    pub(in crate::app) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.request_file_load(path);
        }
    }

    /// Открыть диалог выбора имени и создать новый пустой файл.
    ///
    /// Формат определяется по расширению выбранного пути. Новый документ
    /// содержит пустой объект, сразу открывается в режиме редактирования и
    /// записывается на диск, чтобы файл был создан до добавления данных.
    pub(in crate::app) fn open_new_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("untitled.json")
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("JSON5", &["json5"])
            .save_file()
        {
            let Some(format) = DataFormat::from_path(&path) else {
                self.show_error(self.locale.text(TextKey::UnsupportedFileExtension));
                return;
            };
            self.create_new_file(path, format);
        }
    }

    /// Инициализировать новый документ с пустым объектом и сохранить его.
    pub(super) fn create_new_file(&mut self, path: PathBuf, format: DataFormat) {
        let started_at = Instant::now();
        let root = match parse_data("{}", Some(DataFormat::Json)) {
            Ok((root, _)) => root,
            Err(error) => {
                self.show_error(&format!(
                    "{} {}",
                    self.locale.text(TextKey::DataParseError),
                    error
                ));
                return;
            }
        };

        match self.write_node_to_path(&root, &path, format) {
            Ok(size_bytes) => {
                self.clear_document_state();
                self.root = Some(root);
                self.mode = AppMode::Edit;
                self.file_state = FileState {
                    path: Some(path),
                    size_bytes,
                    load_time_ms: started_at.elapsed().as_millis(),
                    format: Some(format),
                };
                self.show_toast(self.locale.text(TextKey::FileCreated));
            }
            Err(error) => self.show_error(&error),
        }
    }

    /// Открыть системный диалог выбора нескольких файлов для сравнения.
    pub(in crate::app) fn open_comparison_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Supported files", &["json", "yaml", "yml", "toml", "json5"])
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("All files", &["*"])
            .pick_files()
        {
            self.load_comparison(paths);
        }
    }

    /// Загрузить файлы и показать отличия между ними.
    ///
    /// Если выбран ровно один файл и документ уже открыт, открытый документ
    /// сравнивается с выбранным и сразу показывается парный diff. При закрытии
    /// сравнения исходный документ восстанавливается.
    pub(in crate::app) fn load_comparison(&mut self, paths: Vec<PathBuf>) {
        if let Err(error) = self.commit_pending_inline_edit() {
            self.show_error(&error);
            return;
        }
        let mut open_document = None;
        if paths.len() == 1
            && let (Some(path), Some(format), Some(root)) = (
                self.file_state.path.as_ref(),
                self.file_state.format,
                self.root.as_ref(),
            )
        {
            let value = match node_to_value(root) {
                Ok(value) => value,
                Err(error) => {
                    self.show_error(&format!("{}: {}", path.display(), error));
                    return;
                }
            };
            open_document = Some((
                ComparisonDocument {
                    path: path.clone(),
                    size_bytes: self.file_state.size_bytes,
                    load_time_ms: self.file_state.load_time_ms,
                    format,
                },
                value,
            ));
        }

        if paths.len() + usize::from(open_document.is_some()) < 2 {
            self.show_error(self.locale.text(TextKey::ComparisonRequiresFiles));
            return;
        }

        let visualization = if open_document.is_some() {
            VisualizationMode::Diff
        } else {
            VisualizationMode::Comparison
        };
        let locale = self.locale;
        let capacity = paths.len() + usize::from(open_document.is_some());
        let mut documents = Vec::with_capacity(capacity);
        let mut values = Vec::with_capacity(capacity);
        if let Some((document, value)) = open_document {
            documents.push(document);
            values.push(value);
        }
        for path in paths {
            let started_at = Instant::now();
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(error) => {
                    self.report_document_load_error(ParseError {
                        message: locale.file_read_error(&format!("{}: {}", path.display(), error)),
                        line: None,
                        column: None,
                    });
                    return;
                }
            };
            let size_bytes = content.len() as u64;
            let format_hint = DataFormat::from_path(&path);
            let (node, format) = match parse_data(&content, format_hint) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.report_document_load_error(ParseError {
                        message: format!("{}: {}", path.display(), error),
                        ..error
                    });
                    return;
                }
            };
            let value = match node_to_value(&node) {
                Ok(value) => value,
                Err(error) => {
                    self.report_document_load_error(ParseError {
                        message: format!("{}: {}", path.display(), error),
                        line: None,
                        column: None,
                    });
                    return;
                }
            };

            documents.push(ComparisonDocument {
                path,
                size_bytes,
                load_time_ms: started_at.elapsed().as_millis(),
                format,
            });
            values.push(value);
        }

        let previous_document = if self.root.is_some() {
            self.take_previous_document()
        } else {
            self.comparison
                .as_mut()
                .and_then(|comparison| comparison.previous_document.take())
        };
        self.reset_for_comparison(visualization);
        self.comparison = Some(ComparisonState {
            documents,
            differences: compare_values(&values),
            left_index: 0,
            right_index: 1,
            pair_cache: None,
            previous_document,
        });
    }

    fn take_previous_document(&mut self) -> Option<PreviousDocumentState> {
        let root = self.root.take()?;
        Some(PreviousDocumentState {
            root,
            file_state: std::mem::take(&mut self.file_state),
            search: std::mem::take(&mut self.search),
            search_query_buf: std::mem::take(&mut self.search_query_buf),
            regex_builder_literal: std::mem::take(&mut self.regex_builder_literal),
            search_window_open: self.search_window_open,
            search_scroll_target: self.search_scroll_target.take(),
            mode: self.mode,
            visualization: self.visualization,
            selected_paths: std::mem::take(&mut self.selected_paths),
            undo_history: std::mem::take(&mut self.undo_history),
            redo_history: std::mem::take(&mut self.redo_history),
        })
    }

    fn reset_for_comparison(&mut self, visualization: VisualizationMode) {
        self.clear_document_state();
        self.visualization = visualization;
    }
}

fn read_document(path: PathBuf, locale: Locale) -> Result<LoadedDocument, ParseError> {
    let started_at = Instant::now();
    let content = std::fs::read_to_string(&path).map_err(|error| ParseError {
        message: locale.file_read_error(&format!("{}: {error}", path.display())),
        line: None,
        column: None,
    })?;
    let size_bytes = content.len() as u64;
    let (root, format) = parse_data(&content, DataFormat::from_path(&path))?;
    let visible_rows = VisibleRows::from_root(&root);
    Ok(LoadedDocument {
        path,
        root,
        size_bytes,
        load_time_ms: started_at.elapsed().as_millis(),
        format,
        visible_rows,
    })
}
