use super::*;

impl StructureView {
    pub(in crate::app::views::structure) fn start(
        &mut self,
        work: impl FnOnce() -> ParseResult + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("struct-view-data-structure".into())
            .spawn(move || {
                let _ = sender.send(work());
            }) {
            Ok(_) => {
                self.pending = Some(receiver);
                self.edit_dialog = None;
                self.error = None;
                self.notice = None;
            }
            Err(error) => {
                self.pending_origin = None;
                self.error = Some(error.to_string());
            }
        }
    }

    pub(in crate::app) fn open(&mut self, path: PathBuf) {
        self.pending_origin = Some(path.clone());
        let format = DataFormat::from_path(&path);
        self.start(move || {
            let source =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            parse_text(source, format)
        });
    }

    pub(in crate::app) fn cancel_pending_open(&mut self) {
        if self.pending_origin.take().is_some() {
            self.pending = None;
        }
    }

    pub(in crate::app::views::structure) fn poll(&mut self, ui: &Ui, locale: Locale) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    self.apply_parse_result(result);
                }
                Err(TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.pending_origin = None;
                    self.error = Some(locale.text(TextKey::BackgroundOperationFailed).into());
                }
                Err(TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(40))
                }
            }
        }
        if let Some(receiver) = &self.export_pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.export_pending = None;
                    match result {
                        Ok(()) => self.notice = Some(locale.text(TextKey::GraphExported).into()),
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.export_pending = None;
                    self.error = Some(locale.text(TextKey::BackgroundOperationFailed).into());
                }
                Err(TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(40))
                }
            }
        }
    }

    fn apply_parse_result(&mut self, result: ParseResult) {
        match result {
            Ok((source, result)) => {
                self.source = source;
                self.source_preview = None;
                self.edit_dialog = None;
                if let Some(path) = self.pending_origin.take() {
                    self.format = DataFormat::from_path(&path);
                    self.origin = Some(path);
                    self.unsaved = false;
                    self.set_editing(false);
                }
                match result {
                    Ok(document) => {
                        self.format = Some(document.format);
                        self.reset(&document);
                        self.document = Some(document);
                        self.source_open = false;
                        self.source_changed = false;
                    }
                    Err(error) => {
                        self.mark_source_changed();
                        self.error = Some(error.to_string());
                        self.source_open = true;
                        self.source_show_full = true;
                    }
                }
            }
            Err(error) => {
                self.pending_origin = None;
                self.error = Some(error);
                self.source_open = true;
                self.source_show_full = true;
            }
        }
    }
}
