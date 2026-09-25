use super::{AppMode, StructViewApp};
use struct_view_core::parser::parse_json;

#[test]
fn delete_key_requests_deletion_of_selected_structure_in_edit_mode() {
    let mut app = StructViewApp::default();
    app.root = Some(parse_json(r#"{"value":1}"#).unwrap());
    app.mode = AppMode::Edit;
    app.selected_paths = std::collections::BTreeSet::from(["value".to_string()]);
    let context = egui::Context::default();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: Some(egui::Key::Delete),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..Default::default()
    };

    context
        .run_ui(input, |ui| app.handle_shortcuts(ui.ctx()))
        .drop_without_applying_deltas();

    assert!(app.delete_requested);
}
