use super::*;
use crate::app::edit::{
    DeleteError, add_typed_child_at_path, delete_selected_structures, edit_child_at_path,
    paste_structures_at_path, rename_at_path, selected_structures,
};
use crate::app::state::{default_field_value, field_value_types};
use crate::clipboard::{
    copy_to_clipboard, decode_structures, encode_structures, read_from_clipboard,
};

impl StructureView {
    pub(in crate::app) fn can_edit(&self) -> bool {
        self.document.is_some() && !self.source_changed && self.pending.is_none()
    }

    pub(in crate::app) fn is_editing(&self) -> bool {
        self.editing
    }

    pub(in crate::app) fn can_delete_selected(&self) -> bool {
        self.editing
            && self.can_edit()
            && self.document.as_ref().is_some_and(|document| {
                !self.selected_nodes.is_empty()
                    && self.selected_nodes.iter().all(|&id| {
                        document
                            .nodes
                            .get(id)
                            .is_some_and(|node| node.parent.is_some())
                    })
            })
    }

    pub(in crate::app) fn can_copy_selected(&self) -> bool {
        self.can_edit() && !self.selected_nodes.is_empty()
    }

    pub(in crate::app) fn can_paste_into_selected(&self) -> bool {
        self.editing
            && self.can_edit()
            && self.selected_nodes.len() == 1
            && self.document.as_ref().is_some_and(|document| {
                self.selected_nodes.iter().next().is_some_and(|&id| {
                    document
                        .nodes
                        .get(id)
                        .is_some_and(|node| matches!(node.kind, Kind::Object | Kind::Array))
                })
            })
    }

    /// Скопировать выбранные узлы диаграммы в буфер обмена.
    pub(in crate::app) fn copy_selected(&mut self, locale: Locale) {
        if !self.can_copy_selected() {
            return;
        }
        let root = match self.source_root() {
            Ok(root) => root,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let mut selected_paths = std::collections::BTreeSet::new();
        for &id in &self.selected_nodes {
            match self.selected_source_node_in(id, &root) {
                Ok(node) => {
                    selected_paths.insert(node.path.clone());
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        let entries = match selected_structures(&root, &selected_paths) {
            Ok(entries) => entries,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let encoded = match encode_structures(&entries) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        self.clipboard_payload = Some(entries.clone());
        match copy_to_clipboard(&encoded) {
            Ok(()) => {
                self.notice = Some(locale.structures_copied(entries.len()));
                self.error = None;
            }
            Err(error) => self.error = Some(locale.system_copy_error(&error)),
        }
    }

    /// Вставить структуры из буфера обмена в выбранный контейнер диаграммы.
    pub(in crate::app) fn paste_into_selected(&mut self, locale: Locale) {
        if !self.can_paste_into_selected() {
            return;
        }
        let Some(&id) = self.selected_nodes.iter().next() else {
            return;
        };
        let target_path = match self.selected_source_node_at(id) {
            Ok((path, _)) => path,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };

        let entries = match read_from_clipboard() {
            Ok(text) => decode_structures(&text),
            Err(system_error) => self
                .clipboard_payload
                .clone()
                .ok_or_else(|| locale.clipboard_read_error(&system_error)),
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };

        let Some(document) = &self.document else {
            return;
        };
        let format = document.format;
        let (mut root, _) = match struct_view_core::parser::parse_data(&self.source, Some(format)) {
            Ok(result) => result,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let count = match paste_structures_at_path(&mut root, &target_path, &entries) {
            Ok(count) => count,
            Err(error) => {
                self.error = Some(locale.paste_error(&error));
                return;
            }
        };
        let source = match struct_view_core::parser::serialize_node(&root, format, false) {
            Ok(source) => source,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let document = match struct_view_core::structure::parse(&source, Some(format)) {
            Ok(document) => document,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        self.source = source;
        self.reset(&document);
        self.document = Some(document);
        self.unsaved = true;
        self.source_changed = false;
        self.notice = Some(locale.structures_pasted(count));
        self.error = None;
    }

    pub(in crate::app) fn set_editing(&mut self, editing: bool) {
        self.editing = editing;
        if !editing {
            self.edit_dialog = None;
        }
    }

    pub(super) fn open_edit_dialog(&mut self) {
        let Some(document) = &self.document else {
            return;
        };
        let Some(node) = document.nodes.get(self.selected) else {
            return;
        };
        if node.parent.is_none() {
            return;
        }

        let result = self.selected_source_node();
        match result {
            Ok((path, source_node)) => {
                let key_editable = node.kind != Kind::Comment
                    && node
                        .parent
                        .is_some_and(|parent| document.nodes[parent].kind == Kind::Object);
                let value = node.value.clone();
                let value_type = source_node.value_type.clone();
                self.edit_dialog = Some(StructureEditDialog {
                    target: StructureEditTarget::Edit {
                        path,
                        key_editable,
                        current_type: value_type.clone(),
                    },
                    key: if key_editable {
                        node.key.clone()
                    } else {
                        String::new()
                    },
                    value_type,
                    value,
                    error: None,
                });
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn open_add_dialog(&mut self) {
        let Some(document) = &self.document else {
            return;
        };
        let Some(node) = document.nodes.get(self.selected) else {
            return;
        };
        let is_object = match node.kind {
            Kind::Object => true,
            Kind::Array => false,
            _ => return,
        };

        match self.selected_source_node() {
            Ok((parent_path, _)) => {
                self.edit_dialog = Some(StructureEditDialog {
                    target: StructureEditTarget::Add {
                        parent_path,
                        is_object,
                    },
                    key: String::new(),
                    value_type: JsonValueType::String,
                    value: String::new(),
                    error: None,
                });
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(in crate::app) fn delete_selected(&mut self, locale: Locale) {
        if !self.can_edit() {
            return;
        }
        let Some(document) = &self.document else {
            return;
        };
        let format = document.format;
        let mut root = match self.source_root() {
            Ok(root) => root,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let mut selected_paths = std::collections::BTreeSet::new();
        for &id in &self.selected_nodes {
            match self.selected_source_node_in(id, &root) {
                Ok(node) if !node.path.is_empty() => {
                    selected_paths.insert(node.path.clone());
                }
                Ok(_) => {
                    self.error = Some(locale.text(TextKey::CannotDeleteRoot).to_string());
                    return;
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        let count = match delete_selected_structures(&mut root, &selected_paths) {
            Ok(count) => count,
            Err(error) => {
                let key = match error {
                    DeleteError::EmptySelection => TextKey::SelectStructure,
                    DeleteError::RootSelected => TextKey::CannotDeleteRoot,
                    DeleteError::MetadataValueSelected => TextKey::CannotDeleteMetadataValue,
                    DeleteError::SelectionNotFound => TextKey::SelectedStructureNotFound,
                };
                self.error = Some(locale.text(key).to_string());
                return;
            }
        };
        let source = match struct_view_core::parser::serialize_node(&root, format, false) {
            Ok(source) => source,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let document = match struct_view_core::structure::parse(&source, Some(format)) {
            Ok(document) => document,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        self.source = source;
        self.reset(&document);
        self.document = Some(document);
        self.unsaved = true;
        self.source_changed = false;
        self.notice = Some(locale.structures_deleted(count));
        self.error = None;
    }

    pub(super) fn selected_source_node(
        &self,
    ) -> Result<(String, struct_view_core::parser::JsonNode), String> {
        self.selected_source_node_at(self.selected)
    }

    pub(super) fn selected_source_node_at(
        &self,
        selected: usize,
    ) -> Result<(String, struct_view_core::parser::JsonNode), String> {
        let root = self.source_root()?;
        let node = self.selected_source_node_in(selected, &root)?;
        Ok((node.path.clone(), node.clone()))
    }

    pub(super) fn source_root(&self) -> Result<struct_view_core::parser::JsonNode, String> {
        let document = self
            .document
            .as_ref()
            .ok_or_else(|| "Нет построенной схемы".to_string())?;
        struct_view_core::parser::parse_data(&self.source, Some(document.format))
            .map(|(root, _)| root)
            .map_err(|error| error.to_string())
    }

    pub(super) fn selected_source_node_in<'a>(
        &self,
        selected: usize,
        root: &'a struct_view_core::parser::JsonNode,
    ) -> Result<&'a struct_view_core::parser::JsonNode, String> {
        let document = self
            .document
            .as_ref()
            .ok_or_else(|| "Нет построенной схемы".to_string())?;
        let selected_node = document
            .nodes
            .get(selected)
            .ok_or_else(|| "Выбранный узел не найден".to_string())?;
        if selected_node.kind == Kind::Comment {
            let source_node = root
                .children
                .iter()
                .find(|child| {
                    child.value_type == JsonValueType::Comment && child.path == selected_node.path
                })
                .ok_or_else(|| "Не найден комментарий в исходном тексте".to_string())?;
            return Ok(source_node);
        }

        let mut ancestors = Vec::new();
        let mut cursor = Some(selected);
        while let Some(id) = cursor {
            ancestors.push(id);
            cursor = document.nodes[id].parent;
        }
        ancestors.reverse();

        let mut source_node = root;
        for &id in ancestors.iter().skip(1) {
            let key = &document.nodes[id].key;
            let mut matching_children = source_node.children.iter().filter(|child| {
                child.value_type != JsonValueType::Comment
                    && child.key.as_deref() == Some(key.as_str())
            });
            source_node = matching_children
                .next()
                .ok_or_else(|| format!("Не найдено поле «{key}» в исходном тексте"))?;
            let duplicated_in_schema = document.nodes[id].parent.is_some_and(|parent| {
                document.nodes[parent]
                    .children
                    .iter()
                    .filter(|&sibling| document.nodes[*sibling].key == *key)
                    .count()
                    > 1
            });
            if matching_children.next().is_some() || duplicated_in_schema {
                return Err(format!(
                    "Поле «{key}» повторяется в исходном тексте и не может быть однозначно изменено"
                ));
            }
        }
        Ok(source_node)
    }

    pub(super) fn show_edit_dialog(&mut self, ctx: &egui::Context, locale: Locale) {
        let Some(mut dialog) = self.edit_dialog.take() else {
            return;
        };
        let format = self
            .document
            .as_ref()
            .map_or(self.format.unwrap_or(DataFormat::Json), |doc| doc.format);
        let is_edit = matches!(&dialog.target, StructureEditTarget::Edit { .. });
        let is_comment_edit = is_edit && dialog.value_type == JsonValueType::Comment;
        let key_editable = match &dialog.target {
            StructureEditTarget::Add { is_object, .. } => *is_object,
            StructureEditTarget::Edit { key_editable, .. } => *key_editable,
        };
        let title = match &dialog.target {
            StructureEditTarget::Add {
                is_object: true, ..
            } => locale.text(TextKey::AddFieldTitle),
            StructureEditTarget::Add {
                is_object: false, ..
            } => locale.text(TextKey::AddElementTitle),
            StructureEditTarget::Edit { .. } => locale.text(TextKey::EditFieldTitle),
        };
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new(title)
            .id(egui::Id::new("structure-edit-dialog"))
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                if key_editable {
                    ui.label(locale.text(TextKey::FieldName));
                    ui.add(egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0));
                }
                ui.horizontal(|ui| {
                    ui.label(locale.text(TextKey::FieldType));
                    let previous_type = dialog.value_type.clone();
                    egui::ComboBox::from_id_salt("structure_edit_type")
                        .selected_text(locale.value_type_label(&dialog.value_type))
                        .show_ui(ui, |ui| {
                            for value_type in field_value_types(format, false, is_comment_edit) {
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
                    JsonValueType::String | JsonValueType::Comment | JsonValueType::Metadata => {
                        ui.label(locale.text(TextKey::Value));
                        let mut text = egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4);
                        if dialog.value_type == JsonValueType::Comment {
                            text = text.hint_text(locale.text(TextKey::CommentHint));
                        } else if dialog.value_type == JsonValueType::Metadata {
                            text = text.hint_text(locale.text(TextKey::MetadataHint));
                        }
                        ui.add(text);
                    }
                    JsonValueType::DateTime | JsonValueType::Number | JsonValueType::Float => {
                        ui.label(locale.text(TextKey::Value));
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.value)
                                .desired_width(320.0)
                                .font(egui::TextStyle::Monospace),
                        );
                    }
                    JsonValueType::Bool => {
                        ui.horizontal(|ui| {
                            ui.label(locale.text(TextKey::Value));
                            egui::ComboBox::from_id_salt("structure_edit_bool")
                                .selected_text(&dialog.value)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "true".to_string(),
                                        "true",
                                    );
                                    ui.selectable_value(
                                        &mut dialog.value,
                                        "false".to_string(),
                                        "false",
                                    );
                                });
                        });
                    }
                    JsonValueType::Null => {
                        ui.label(format!("{}: null", locale.text(TextKey::Value)));
                    }
                    JsonValueType::Object => {
                        ui.label(locale.text(TextKey::EmptyObject));
                    }
                    JsonValueType::Array => {
                        ui.label(locale.text(TextKey::EmptyArray));
                    }
                }
                if let Some(error) = &dialog.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(locale.text(if is_edit {
                            TextKey::Apply
                        } else {
                            TextKey::Add
                        }))
                        .clicked()
                    {
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
            self.edit_dialog = Some(dialog);
            return;
        }

        match self.apply_edit_dialog(&dialog) {
            Ok(()) => self.notice = Some(locale.text(TextKey::FieldUpdated).to_string()),
            Err(error) => {
                dialog.error = Some(error);
                self.edit_dialog = Some(dialog);
            }
        }
    }

    pub(super) fn apply_edit_dialog(&mut self, dialog: &StructureEditDialog) -> Result<(), String> {
        let selected = self.selected;
        let selected_path = self
            .document
            .as_ref()
            .and_then(|document| document.nodes.get(selected))
            .map(|node| node.path.clone());
        let format = self
            .document
            .as_ref()
            .map_or(self.format.unwrap_or(DataFormat::Json), |doc| doc.format);
        let (mut root, _) = struct_view_core::parser::parse_data(&self.source, Some(format))
            .map_err(|error| error.to_string())?;
        match &dialog.target {
            StructureEditTarget::Add {
                parent_path,
                is_object: _,
            } => add_typed_child_at_path(
                &mut root,
                parent_path,
                &dialog.key,
                &dialog.value_type,
                &dialog.value,
                format,
            )?,
            StructureEditTarget::Edit {
                path,
                key_editable,
                current_type,
            } if matches!(current_type, JsonValueType::Object | JsonValueType::Array)
                && *current_type == dialog.value_type =>
            {
                if *key_editable {
                    rename_at_path(&mut root, path, &dialog.key)?;
                }
            }
            StructureEditTarget::Edit {
                path, key_editable, ..
            } => edit_child_at_path(
                &mut root,
                path,
                key_editable.then_some(dialog.key.as_str()),
                &dialog.value_type,
                &dialog.value,
                format,
            )?,
        }

        let source = struct_view_core::parser::serialize_node(&root, format, false)?;
        let document = struct_view_core::structure::parse(&source, Some(format))
            .map_err(|error| error.to_string())?;
        let selected_after_edit = selected_path
            .and_then(|path| document.nodes.iter().position(|node| node.path == path))
            .or_else(|| (selected < document.nodes.len()).then_some(selected));
        self.source = source;
        self.reset(&document);
        self.document = Some(document);
        if let Some(selected) = selected_after_edit {
            self.select(selected, true);
        }
        self.source_changed = false;
        self.unsaved = true;
        self.edit_dialog = None;
        Ok(())
    }
}
