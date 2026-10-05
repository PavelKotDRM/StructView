use super::*;

impl StructureView {
    pub(in crate::app) fn status_summary(&mut self, locale: Locale) -> Option<Vec<String>> {
        self.ensure_layout();
        let doc = self.document.as_ref()?;
        let mut fields = Vec::new();
        if let Some(path) = &self.origin
            && let Some(name) = path.file_name()
        {
            fields.push(format!("📄 {}", name.to_string_lossy()));
        }
        fields.extend([
            doc.format.to_string(),
            format!(
                "{} / {} {}",
                doc.nodes.len(),
                self.layout.nodes.len(),
                locale.text(TextKey::StructureNodes)
            ),
            format!("{:.0}%", self.zoom * 100.0),
        ]);
        Some(fields)
    }

    pub(in crate::app) fn status_selection(&self) -> Option<(String, String, String)> {
        let doc = self.document.as_ref()?;
        let node = &doc.nodes[self.selected];
        let path = if node.path.is_empty() {
            "/"
        } else {
            &node.path
        };
        Some((
            path.to_string(),
            format!("{}: {}", node.key, shorten(&node.value, 100)),
            format!("{}: {}\n{}", node.key, node.value, path),
        ))
    }

    pub(super) fn export(&mut self, png: bool, dark: bool) {
        self.ensure_layout();
        let Some(doc) = &self.document else {
            return;
        };
        let extension = if png { "png" } else { "svg" };
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(extension, &[extension])
            .set_file_name(format!("structure.{extension}"))
            .save_file()
        else {
            return;
        };
        path.set_extension(extension);
        let svg = export::styled_svg(doc, &self.layout, &self.collapsed, &self.limits, dark);
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("struct-view-structure-export".into())
            .spawn(move || {
                let result = export::write(&path, &svg, png).map_err(|e| e.to_string());
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.export_pending = Some(receiver);
                self.error = None;
                self.notice = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}
