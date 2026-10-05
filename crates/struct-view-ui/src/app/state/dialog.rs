use super::dialog_helpers::{default_field_value, field_value_types};
use super::*;
use crate::app::docking::{DetachedViewportAction, show_detached_viewport, show_docking_controls};

impl StructViewApp {
    /// Отрисовать конструктор и добавить или изменить узел после подтверждения.
    pub(in crate::app) fn show_field_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.field_dialog.take() else {
            self.field_dialog_docking.reset_if_active();
            return;
        };

        let mut submit = false;
        let mut cancel = false;
        let locale = self.locale;
        let format = self.file_state.format.unwrap_or(DataFormat::Json);
        let is_toml_root = format == DataFormat::Toml
            && matches!(&dialog.target, FieldDialogTarget::Edit { path, .. } if path.is_empty());
        let is_edit = matches!(&dialog.target, FieldDialogTarget::Edit { .. });
        let is_comment_edit = is_edit && dialog.value_type == JsonValueType::Comment;
        let show_key_input = match &dialog.target {
            FieldDialogTarget::Add { is_object, .. } => *is_object,
            FieldDialogTarget::Edit { key_editable, .. } => *key_editable,
        } && dialog.value_type != JsonValueType::Comment;
        let show_readonly_key = is_edit && !show_key_input && !dialog.key.is_empty();
        let title = match &dialog.target {
            FieldDialogTarget::Add {
                is_object: true, ..
            } => locale.text(TextKey::AddFieldTitle),
            FieldDialogTarget::Add {
                is_object: false, ..
            } => locale.text(TextKey::AddElementTitle),
            FieldDialogTarget::Edit { .. } => locale.text(TextKey::EditFieldTitle),
        };

        let mut docking = std::mem::take(&mut self.field_dialog_docking);
        docking.begin();
        let viewport_id = egui::ViewportId::from_hash_of("field_dialog");
        let mut show_content = |ui: &mut egui::Ui| {
            let colors = SyntaxColors::new(ui.visuals());

            if show_key_input {
                ui.label(locale.text(TextKey::FieldName));
                ui.add(egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0));
            } else if show_readonly_key {
                ui.horizontal(|ui| {
                    ui.label(locale.text(TextKey::FieldName));
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(&mut dialog.key).desired_width(320.0),
                    );
                });
            }

            ui.horizontal(|ui| {
                ui.label(locale.text(TextKey::FieldType));
                let previous_type = dialog.value_type.clone();
                egui::ComboBox::from_id_salt("field_dialog_type")
                    .selected_text(locale.value_type_label(&dialog.value_type))
                    .show_ui(ui, |ui| {
                        for value_type in field_value_types(format, is_toml_root, is_comment_edit) {
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
                JsonValueType::String => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type)),
                    );
                }
                JsonValueType::Comment => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text(locale.text(TextKey::CommentHint)),
                    );
                }
                JsonValueType::Metadata => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.value)
                            .desired_width(420.0)
                            .desired_rows(4)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text(locale.text(TextKey::MetadataHint)),
                    );
                }
                JsonValueType::DateTime => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.value)
                            .desired_width(320.0)
                            .font(egui::TextStyle::Monospace)
                            .text_color(colors.value_color(&dialog.value_type))
                            .hint_text("1979-05-27T07:32:00Z"),
                    );
                }
                JsonValueType::Number | JsonValueType::Float => {
                    ui.label(locale.text(TextKey::Value));
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.value)
                            .desired_width(320.0)
                            .font(egui::TextStyle::Monospace)
                            .text_color(colors.value_color(&dialog.value_type)),
                    );
                }
                JsonValueType::Bool => {
                    ui.horizontal(|ui| {
                        ui.label(locale.text(TextKey::Value));
                        egui::ComboBox::from_id_salt("field_dialog_bool")
                            .selected_text(
                                egui::RichText::new(&dialog.value)
                                    .color(colors.value_color(&dialog.value_type)),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut dialog.value,
                                    "true".to_string(),
                                    egui::RichText::new("true")
                                        .color(colors.value_color(&dialog.value_type)),
                                );
                                ui.selectable_value(
                                    &mut dialog.value,
                                    "false".to_string(),
                                    egui::RichText::new("false")
                                        .color(colors.value_color(&dialog.value_type)),
                                );
                            });
                    });
                }
                JsonValueType::Null => {
                    ui.horizontal(|ui| {
                        ui.label(locale.text(TextKey::Value));
                        ui.colored_label(colors.value_color(&dialog.value_type), "null");
                    });
                }
                JsonValueType::Object => {
                    ui.colored_label(
                        colors.value_color(&dialog.value_type),
                        locale.text(TextKey::EmptyObject),
                    );
                }
                JsonValueType::Array => {
                    ui.colored_label(
                        colors.value_color(&dialog.value_type),
                        locale.text(TextKey::EmptyArray),
                    );
                }
            }

            if let Some(error) = &dialog.error {
                ui.colored_label(colors.error, error);
            }
            ui.horizontal(|ui| {
                let action = if is_edit {
                    TextKey::Apply
                } else {
                    TextKey::Add
                };
                if ui.button(locale.text(action)).clicked() {
                    submit = true;
                }
                if ui.button(locale.text(TextKey::Cancel)).clicked() {
                    cancel = true;
                }
            });
        };

        let action = if docking.detached {
            show_detached_viewport(
                ctx,
                &mut docking,
                viewport_id,
                title,
                egui::vec2(460.0, 320.0),
                locale.text(TextKey::DockWindow),
                |ui| show_content(ui),
            )
        } else {
            let mut open = true;
            let mut window = egui::Window::new(title)
                .id(docking.area_id("field_dialog"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true);
            if let Some(size) = docking.embedded_size() {
                window = window.default_size(size);
            }
            if let Some(position) = docking.take_embedded_position() {
                window = window.current_pos(position);
            }
            let mut detach_requested = false;
            let response = window.show(ctx, |ui| {
                detach_requested = show_docking_controls(
                    ui,
                    false,
                    locale.text(TextKey::DetachWindow),
                    locale.text(TextKey::WindowOptions),
                );
                show_content(ui);
            });
            if !open {
                docking.reset_if_active();
                DetachedViewportAction::Closed
            } else {
                if let Some(response) = response {
                    docking.remember_embedded_size(response.response.rect.size());
                    if detach_requested {
                        let viewport_origin =
                            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min));
                        docking.detach(
                            ctx.viewport_rect(),
                            response.response.rect,
                            viewport_origin,
                        );
                        ctx.request_repaint();
                    }
                }
                DetachedViewportAction::KeepOpen
            }
        };
        drop(show_content);
        self.field_dialog_docking = docking;

        if action == DetachedViewportAction::Closed {
            cancel = true;
        }
        if cancel {
            self.field_dialog_docking.reset_if_active();
            return;
        }
        if !submit {
            self.field_dialog = Some(dialog);
            return;
        }

        let target = dialog.target.clone();
        let result = self.apply_document_change(|root, format| match target {
            FieldDialogTarget::Add { parent_path, .. } => add_typed_child_at_path(
                root,
                &parent_path,
                &dialog.key,
                &dialog.value_type,
                &dialog.value,
                format,
            ),
            FieldDialogTarget::Edit { path, key_editable } => edit_child_at_path(
                root,
                &path,
                key_editable.then_some(dialog.key.as_str()),
                &dialog.value_type,
                &dialog.value,
                format,
            ),
        });

        match result {
            Ok(()) => {
                self.field_dialog_docking.reset_if_active();
                let message = if is_edit {
                    TextKey::FieldUpdated
                } else {
                    TextKey::DataAdded
                };
                self.show_toast(self.locale.text(message));
            }
            Err(error) => {
                dialog.error = Some(error);
                self.field_dialog = Some(dialog);
            }
        }
    }
}
