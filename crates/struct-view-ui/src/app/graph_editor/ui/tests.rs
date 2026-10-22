use super::*;

fn sample() -> GraphDocument {
    let mut document = GraphDocument::empty();
    document.nodes = vec![
        json!({"id":"a","label":"A","partition":"first","attributes":{}}),
        json!({"id":"b","label":"B","attributes":{}}),
    ];
    document.partitions = vec!["first".into()];
    document
}

#[test]
fn graph_editor_forms_create_connected_entities_and_preserve_attributes() {
    let mut document = sample();
    let mut editor = GraphEditorState::default();
    editor.open(&document, Target::Node(None));
    let draft = editor.draft.as_mut().unwrap();
    draft.source = "a".into();
    draft.direction = "bidirectional".into();
    draft.attributes = r#"{"active":true,"tags":["one"],"nested":{"x":2}}"#.into();
    draft.apply(&mut document).unwrap();
    assert_eq!(document.nodes.len(), 3);
    assert_eq!(document.edges.len(), 1);
    assert_eq!(document.edges[0]["source"], "a");
    assert_eq!(document.edges[0]["target"], draft.id);
    assert_eq!(document.nodes[2]["attributes"]["active"], true);
    assert_eq!(document.nodes[2]["attributes"]["nested"]["x"], 2);
}

#[test]
fn graph_editor_live_json_matches_applied_records_without_duplicating_attributes() {
    for target in [Target::Node(None), Target::Edge(None), Target::Settings] {
        let mut document = sample();
        let mut editor = GraphEditorState::default();
        editor.open(&document, target);
        let draft = editor.draft.as_mut().unwrap();
        draft.id = "new-id".into();
        draft.label = "New label".into();
        draft.attributes = r#"{"count":2,"active":true,"nested":{"values":[1,null]}}"#.into();
        if matches!(target, Target::Edge(_)) {
            draft.source = "a".into();
            draft.target_id = "b".into();
            draft.weight = "2.5".into();
        }
        let preview = draft.build_record().unwrap();
        assert_eq!(preview["attributes"]["count"], 2);
        assert_eq!(preview["attributes"]["active"], true);
        assert!(preview["attributes"].get("id").is_none());
        draft.apply(&mut document).unwrap();
        let applied = match target {
            Target::Node(_) => document.nodes.last().unwrap().clone(),
            Target::Edge(_) => document.edges.last().unwrap().clone(),
            Target::Settings => Value::Object(document.metadata),
            _ => unreachable!(),
        };
        assert_eq!(preview, applied);
        if !matches!(target, Target::Settings) {
            assert_eq!(preview["id"], "new-id");
            assert_eq!(preview["label"], "New label");
        }
        draft.attributes = "[]".into();
        assert!(draft.build_record().is_err());
        draft.attributes = "{".into();
        assert!(draft.build_record().is_err());
    }
}

#[test]
fn graph_editor_group_rename_and_delete_update_memberships() {
    let mut document = sample();
    let mut editor = GraphEditorState::default();
    editor.open(&document, Target::Partition(Some(0)));
    let draft = editor.draft.as_mut().unwrap();
    draft.label = "renamed".into();
    draft.apply(&mut document).unwrap();
    assert_eq!(document.nodes[0]["partition"], "renamed");
    draft.delete(&mut document).unwrap();
    assert!(document.partitions.is_empty());
    assert!(document.nodes[0].get("partition").is_none());
}

#[test]
fn graph_editor_settings_and_edge_forms_use_numeric_weights() {
    let mut document = sample();
    let mut editor = GraphEditorState::default();
    editor.open(&document, Target::Settings);
    let draft = editor.draft.as_mut().unwrap();
    draft.direction = "undirected".into();
    draft.weight = "ms".into();
    draft.apply(&mut document).unwrap();
    editor.open(&document, Target::Edge(None));
    let draft = editor.draft.as_mut().unwrap();
    assert_eq!(draft.direction, "undirected");
    draft.weight = "2.5".into();
    draft.apply(&mut document).unwrap();
    assert_eq!(document.edges[0]["weight"], 2.5);
    draft.weight = "\"not a number\"".into();
    assert!(draft.apply(&mut document).is_err());
    draft.weight.clear();
    draft.target = Target::Edge(Some(0));
    draft.apply(&mut document).unwrap();
    assert!(document.edges[0].get("weight").is_none());
}

#[test]
fn flowchart_node_editor_changes_the_shape_attribute() {
    let mut document = sample();
    document
        .metadata
        .insert("type".into(), json!("flowchart_multigraph"));
    document.nodes[0]["attributes"]["shape"] = json!("decision");
    let mut editor = GraphEditorState::default();
    editor.open(&document, Target::Node(Some(0)));
    let draft = editor.draft.as_mut().unwrap();

    assert!(draft.flowchart);
    assert_eq!(draft.shape, "decision");
    draft.shape = "terminator".into();
    draft.label = "Finished successfully".into();
    let preview = draft.build_record().unwrap();
    assert_eq!(preview["attributes"]["shape"], "terminator");
    draft.apply(&mut document).unwrap();

    let root = document.to_root().unwrap();
    let graph = crate::app::visualization::try_build_relationship_graph(&root).unwrap();
    assert_eq!(
        graph.nodes[0].flowchart_shape(),
        Some(crate::app::visualization::FlowchartNodeShape::Terminator)
    );
    assert_eq!(graph.nodes[0].label, "Finished successfully");
}

fn render(
    ctx: &egui::Context,
    app: &mut StructViewApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.show_top_panel(ui);
            app.show_central_panel(ui);
            app.show_graph_editor(ui.ctx());
            app.show_graph_editor_dialog(ui.ctx());
        },
    )
}

fn open_menu(ctx: &egui::Context, app: &mut StructViewApp, key: TextKey) -> egui::FullOutput {
    let output = render(ctx, app, vec![]);
    let events = click(&output, app.locale.text(key));
    output.drop_without_applying_deltas();
    render(ctx, app, events).drop_without_applying_deltas();
    render(ctx, app, vec![])
}

fn click(output: &egui::FullOutput, label: &str) -> Vec<egui::Event> {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let point = egui::pos2(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn click_last(output: &egui::FullOutput, label: &str) -> Vec<egui::Event> {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .rev()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let point = egui::pos2(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn graph_editor_native_ui_add_apply_cancel_and_history_work() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    app.open_graph_document(GraphDocument::empty().to_root().unwrap());
    let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
    let events = click(&output, "Add entity");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    assert!(app.graph_editor.is_busy());
    assert!(!app.can_undo());
    let output = render(&ctx, &mut app, vec![]);
    let events = click(&output, "Apply");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    assert!(!app.graph_editor.is_busy());
    assert_eq!(
        GraphDocument::read(app.root.as_ref().unwrap())
            .unwrap()
            .unwrap()
            .nodes
            .len(),
        1
    );
    assert!(app.can_undo());
    app.undo();
    assert!(
        GraphDocument::read(app.root.as_ref().unwrap())
            .unwrap()
            .unwrap()
            .nodes
            .is_empty()
    );
    app.redo();
    let original = node_to_value(app.root.as_ref().unwrap()).unwrap();
    let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
    let events = click(&output, "Add entity");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, vec![]);
    let events = click(&output, "Cancel");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    assert!(!app.graph_editor.is_busy());
    assert_eq!(node_to_value(app.root.as_ref().unwrap()).unwrap(), original);
}

#[test]
fn graph_editor_read_only_view_has_no_mutation_controls() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    app.open_graph_document(sample().to_root().unwrap());
    app.mode = AppMode::View;
    let output = render(&ctx, &mut app, vec![]);
    let nodes = &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    assert!(
        !nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Add entity"))
    );
    assert!(
        nodes
            .iter()
            .all(|(_, node)| node.label() != Some(app.locale.text(TextKey::GraphEditor)))
    );
    output.drop_without_applying_deltas();
    let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
    for key in [
        TextKey::GraphAddNode,
        TextKey::GraphAddEdge,
        TextKey::GraphAddGroup,
        TextKey::GraphSettings,
    ] {
        assert!(!has_label(&output, app.locale.text(key)));
    }
    output.drop_without_applying_deltas();
}

fn has_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some(label))
}

#[test]
fn graph_editor_commands_are_in_menus_and_hidden_in_other_modes() {
    for mode in [
        VisualizationMode::Graph,
        VisualizationMode::Tree,
        VisualizationMode::Table,
        VisualizationMode::Schema,
    ] {
        for key in [TextKey::FileMenu, TextKey::EditMenu, TextKey::ViewMenu] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = StructViewApp::default();
            app.locale = Locale::English;
            app.open_graph_document(sample().to_root().unwrap());
            app.visualization = mode;
            app.graph_editor.open = false;
            let output = open_menu(&ctx, &mut app, key);
            let graph = mode == VisualizationMode::Graph;
            match key {
                TextKey::FileMenu => {
                    assert_eq!(has_label(&output, "New graph"), graph);
                    assert!(!has_label(&output, "Create editable graph copy"));
                }
                TextKey::EditMenu => {
                    assert_eq!(has_label(&output, "Add entity"), graph);
                    assert_eq!(has_label(&output, "Add link"), graph);
                    assert_eq!(has_label(&output, "Add group"), graph);
                    assert_eq!(has_label(&output, "Graph properties"), graph);
                    assert_eq!(
                        has_label(&output, app.locale.text(TextKey::CopySelectedStructures)),
                        mode == VisualizationMode::Tree
                    );
                }
                TextKey::ViewMenu => {
                    assert_eq!(has_label(&output, "Graph editor"), graph);
                    assert_eq!(
                        has_label(&output, app.locale.text(TextKey::TreeActions)),
                        mode == VisualizationMode::Tree,
                        "Mode {mode:?}, labels: {:?}",
                        output
                            .platform_output
                            .accesskit_update
                            .as_ref()
                            .unwrap()
                            .nodes
                            .iter()
                            .filter_map(|(_, node)| node.label())
                            .collect::<Vec<_>>()
                    );
                }
                _ => unreachable!(),
            }
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn edit_mode_creates_an_editable_copy_for_imported_flowcharts() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    let root = struct_view_core::parser::parse_data(
        "flowchart TD\n  start([Start]) --> check{Valid?}\n",
        Some(DataFormat::Mermaid),
    )
    .unwrap()
    .0;
    app.root = Some(root.clone());
    app.file_state.format = Some(DataFormat::Mermaid);
    app.visualization = VisualizationMode::Graph;
    app.mode = AppMode::View;

    let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
    let events = click(&output, "Mode");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, Vec::new());
    let events = click_last(&output, "Edit");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();

    assert_eq!(app.mode, AppMode::Edit);
    let document = GraphDocument::read(app.root.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert!(document.is_flowchart());
    assert_eq!(document.nodes[0]["attributes"]["shape"], "terminator");
    assert_eq!(
        app.file_state.format,
        Some(DataFormat::Json),
        "The imported Mermaid source remains read-only; the editable copy uses JSON"
    );

    let output = open_menu(&ctx, &mut app, TextKey::EditMenu);
    let events = click(&output, "Mode");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, Vec::new());
    let events = click_last(&output, "View");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();

    assert_eq!(app.mode, AppMode::View);
    assert_eq!(app.file_state.format, Some(DataFormat::Mermaid));
    assert!(
        GraphDocument::read(app.root.as_ref().unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        struct_view_core::parser::node_to_value(app.root.as_ref().unwrap()).unwrap(),
        struct_view_core::parser::node_to_value(&root).unwrap()
    );
}

#[test]
fn graph_editor_window_is_toggleable_and_supports_detach_and_dock() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    app.open_graph_document(sample().to_root().unwrap());
    // Since egui 0.37 a newly shown window is excluded from the AccessKit tree during its sizing pass.
    render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, vec![]);
    assert!(has_label(&output, "Graph editor"));
    assert!(has_label(&output, "Add entity"));
    assert!(!app.graph_editor.docking.detached);
    output.drop_without_applying_deltas();

    app.graph_editor.open = false;
    let output = open_menu(&ctx, &mut app, TextKey::ViewMenu);
    let events = click(&output, "Graph editor");
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    assert!(app.graph_editor.open);
    // Exercise the same immediate viewport fallback used by docking tests.
    let main = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let window = egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(420.0, 440.0));
    app.graph_editor.docking.detach(main, window, None);
    render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, vec![]);
    assert!(has_label(&output, app.locale.text(TextKey::DockWindow)));
    let events = click(&output, app.locale.text(TextKey::DockWindow));
    output.drop_without_applying_deltas();
    render(&ctx, &mut app, events).drop_without_applying_deltas();
    assert!(!app.graph_editor.docking.detached);
    assert!(app.graph_editor.open);
    app.visualization = VisualizationMode::Tree;
    let output = render(&ctx, &mut app, vec![]);
    assert!(!has_label(&output, "Graph editor"));
    output.drop_without_applying_deltas();
    app.visualization = VisualizationMode::Graph;
    render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
    let output = render(&ctx, &mut app, vec![]);
    assert!(has_label(&output, "Graph editor"));
    assert!(!app.graph_editor.docking.detached);
    output.drop_without_applying_deltas();
}

#[test]
fn graph_editor_window_can_collapse_and_expand_without_losing_document() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    ctx.style_mut_of(ctx.theme(), |style| style.animation_time = 0.0);
    let mut app = StructViewApp::default();
    app.locale = Locale::English;
    app.open_graph_document(sample().to_root().unwrap());
    let before = node_to_value(app.root.as_ref().unwrap()).unwrap();
    let frame = |app: &mut StructViewApp, time, events| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| app.show_graph_editor(ui.ctx()),
        )
    };
    frame(&mut app, 0.0, vec![]).drop_without_applying_deltas();
    let events = |title| {
        vec![
            egui::Event::PointerMoved(title),
            egui::Event::PointerButton {
                pos: title,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: title,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let id = app
        .graph_editor
        .docking
        .area_id("graph-editor-window")
        .with("collapsing");
    for (index, expected) in [false, true].into_iter().enumerate() {
        let time = index as f64 + 1.0;
        let output = frame(&mut app, time, vec![]);
        let title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Graph editor" => {
                    Some(text.pos + egui::vec2(10.0, 5.0))
                }
                _ => None,
            })
            .expect("Editor title is visible");
        output.drop_without_applying_deltas();
        frame(&mut app, time + 0.1, events(title)).drop_without_applying_deltas();
        frame(&mut app, time + 0.2, events(title)).drop_without_applying_deltas();
        assert_eq!(
            egui::collapsing_header::CollapsingState::load(&ctx, id)
                .unwrap()
                .is_open(),
            expected,
        );
        assert!(app.graph_editor.open);
        assert_eq!(node_to_value(app.root.as_ref().unwrap()).unwrap(), before);
    }
}

#[test]
fn graph_editor_window_commands_open_forms_attached_and_detached() {
    for detached in [false, true] {
        for key in [
            TextKey::GraphAddNode,
            TextKey::GraphAddEdge,
            TextKey::GraphAddGroup,
            TextKey::GraphSettings,
        ] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = StructViewApp::default();
            app.locale = Locale::English;
            app.open_graph_document(sample().to_root().unwrap());
            if detached {
                let rect =
                    egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(420.0, 440.0));
                app.graph_editor
                    .docking
                    .detach(ctx.viewport_rect(), rect, None);
            }
            render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
            let output = render(&ctx, &mut app, vec![]);
            let bounds = output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| node.label() == Some(app.locale.text(key)))
                .filter_map(|(_, node)| node.bounds())
                .min_by(|left, right| left.y0.total_cmp(&right.y0))
                .unwrap();
            let events = vec![
                egui::Event::PointerMoved(egui::pos2(
                    bounds.x0 as f32 + 5.0,
                    bounds.y0 as f32 + 5.0,
                )),
                egui::Event::PointerButton {
                    pos: egui::pos2(bounds.x0 as f32 + 5.0, bounds.y0 as f32 + 5.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: egui::pos2(bounds.x0 as f32 + 5.0, bounds.y0 as f32 + 5.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            output.drop_without_applying_deltas();
            render(&ctx, &mut app, events).drop_without_applying_deltas();
            let target = app
                .graph_editor
                .draft
                .as_ref()
                .expect("Command opens a form")
                .target;
            assert!(match key {
                TextKey::GraphAddNode => matches!(target, Target::Node(None)),
                TextKey::GraphAddEdge => matches!(target, Target::Edge(None)),
                TextKey::GraphAddGroup => matches!(target, Target::Partition(None)),
                TextKey::GraphSettings => matches!(target, Target::Settings),
                _ => unreachable!(),
            });
            assert!(
                app.graph_editor.open,
                "Command must not close the editor window"
            );
            assert_eq!(app.graph_editor.docking.detached, detached);
        }
    }
}
