use super::*;
use crate::app::edit::{
    DeleteError, add_typed_child_at_path, delete_selected_structures, edit_child_at_path,
    rename_at_path,
};
use crate::app::state::{default_field_value, field_value_types};

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
                let key_editable = node
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
        let mut selected_paths = std::collections::BTreeSet::new();
        for &id in &self.selected_nodes {
            match self.selected_source_node_at(id) {
                Ok((path, _)) if !path.is_empty() => {
                    selected_paths.insert(path);
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
        let Some(document) = &self.document else {
            return;
        };
        let format = document.format;
        let (mut root, _) = match struct_view_core::parser::parse_data(&self.source, Some(format)) {
            Ok(document) => document,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
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
        let document = self
            .document
            .as_ref()
            .ok_or_else(|| "Нет построенной схемы".to_string())?;
        let (mut root, _) =
            struct_view_core::parser::parse_data(&self.source, Some(document.format))
                .map_err(|error| error.to_string())?;
        document
            .nodes
            .get(selected)
            .ok_or_else(|| "Выбранный узел не найден".to_string())?;

        let mut ancestors = Vec::new();
        let mut cursor = Some(selected);
        while let Some(id) = cursor {
            ancestors.push(id);
            cursor = document.nodes[id].parent;
        }
        ancestors.reverse();

        let mut source_node = &mut root;
        for &id in ancestors.iter().skip(1) {
            let key = &document.nodes[id].key;
            let mut matching_children = source_node.children.iter_mut().filter(|child| {
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
        let path = source_node.path.clone();
        let source_node = source_node.clone();
        Ok((path, source_node))
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
                            for value_type in field_value_types(format, false, false) {
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
        self.source = source;
        self.reset(&document);
        let node_count = document.nodes.len();
        self.document = Some(document);
        if selected < node_count {
            self.select(selected, true);
        }
        self.source_changed = false;
        self.unsaved = true;
        self.edit_dialog = None;
        Ok(())
    }
}
