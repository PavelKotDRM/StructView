use super::*;

impl StructViewApp {
    /// Открыть системный диалог выбора нескольких файлов для сравнения.
    pub(in crate::app) fn open_comparison_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Supported files",
                &[
                    "json", "yaml", "yml", "toml", "json5", "dot", "gv", "graphml", "gexf", "xml",
                ],
            )
            .add_filter("JSON", &["json", "json5"])
            .add_filter("YAML", &["yaml", "yml"])
            .add_filter("TOML", &["toml"])
            .add_filter("Graph files", &["dot", "gv", "graphml", "gexf", "xml"])
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
