use super::*;

impl StructViewApp {
    pub(super) fn save_table_csv(&mut self) {
        let Some(table) = self.visualization_cache.table.as_ref() else {
            return;
        };
        let content = table_csv(table, &self.search);
        if let Some(mut path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name("table.csv")
            .save_file()
        {
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("csv"))
            {
                path.set_extension("csv");
            }
            match struct_view_core::files::write_text_atomic(&path, &content) {
                Ok(()) => self.show_toast(self.locale.text(TextKey::TableExported)),
                Err(error) => self.show_error(&self.locale.save_error(&error.to_string())),
            }
        }
    }
}
