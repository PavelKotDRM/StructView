use std::collections::BTreeSet;

use egui::Ui;

use struct_view_core::parser::{JsonNode, JsonValueType};

use super::super::i18n::{Locale, TextKey};
use super::super::state::AppMode;
use super::model::{AddChildRequest, EditFieldRequest, SelectionRequest, TreeOutcome};

pub(super) fn selection_request(ui: &Ui, path: &str) -> SelectionRequest {
    SelectionRequest {
        path: path.to_string(),
        additive: ui.input(|input| input.modifiers.command),
    }
}

/// Контекстное меню узла с опциями копирования значения, ключа, пути и структуры.
pub(super) fn context_menu(
    ui: &mut Ui,
    node: &JsonNode,
    mode: AppMode,
    locale: Locale,
    selected_paths: &BTreeSet<String>,
    outcome: &mut TreeOutcome,
) {
    if ui.button(locale.text(TextKey::CopyValue)).clicked() {
        outcome.copy_request = Some(node.display_value.clone());
        ui.close();
    }
    if let Some(key) = &node.key
        && ui.button(locale.text(TextKey::CopyKey)).clicked()
    {
        outcome.copy_request = Some(key.clone());
        ui.close();
    }
    if ui.button(locale.text(TextKey::CopyPath)).clicked() {
        outcome.copy_request = Some(node.path.clone());
        ui.close();
    }
    if ui.button(locale.text(TextKey::CopyStructure)).clicked() {
        outcome.copy_structure_paths = Some(vec![node.path.clone()]);
        ui.close();
    }
    if !selected_paths.is_empty() && ui.button(locale.text(TextKey::CopySelected)).clicked() {
        outcome.copy_structure_paths = Some(selected_paths.iter().cloned().collect());
        ui.close();
    }
    if mode == AppMode::Edit && ui.button(locale.text(TextKey::EditField)).clicked() {
        outcome.edit_field_request = Some(EditFieldRequest {
            path: node.path.clone(),
        });
        ui.close();
    }
}

/// Контекстное меню контейнера с командами копирования и добавления данных.
pub(super) fn container_context_menu(
    ui: &mut Ui,
    node: &JsonNode,
    mode: AppMode,
    locale: Locale,
    selected_paths: &BTreeSet<String>,
    outcome: &mut TreeOutcome,
) {
    context_menu(ui, node, mode, locale, selected_paths, outcome);

    if mode != AppMode::Edit
        || !matches!(
            node.value_type,
            JsonValueType::Object | JsonValueType::Array
        )
    {
        return;
    }

    if ui.button(locale.text(TextKey::PasteHere)).clicked() {
        outcome.paste_target_path = Some(node.path.clone());
        ui.close();
    }

    let is_object = node.value_type == JsonValueType::Object;
    let label = if is_object {
        locale.text(TextKey::AddField)
    } else {
        locale.text(TextKey::AddElement)
    };
    if ui.button(label).clicked() {
        outcome.add_child_request = Some(AddChildRequest {
            parent_path: node.path.clone(),
            is_object,
        });
        ui.close();
    }
}
