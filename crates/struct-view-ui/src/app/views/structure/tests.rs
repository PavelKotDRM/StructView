use super::*;
mod help;

fn view(text: &str) -> StructureView {
    view_with_format(text, DataFormat::Json)
}

fn view_with_format(text: &str, format: DataFormat) -> StructureView {
    let document = struct_view_core::structure::parse(text, Some(format)).unwrap();
    let mut view = StructureView::default();
    view.reset(&document);
    view.document = Some(document);
    view.source = text.to_string();
    view.format = Some(format);
    view.source_open = false;
    view
}

fn frame(
    view: &mut StructureView,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                view.show(ui, Locale::English, None);
            });
        },
    )
}

#[test]
fn search_reveals_hidden_nodes_and_their_complete_path() {
    let text = format!(
        "[{}]",
        (0..1500)
            .map(|i| format!(
                r#"{{"value":"{}"}}"#,
                if i == 1499 {
                    "needle".into()
                } else {
                    i.to_string()
                }
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut view = view(&text);
    assert!(view.collapsed.contains(&0));
    view.query = "needle".into();
    view.search();
    assert_eq!(view.matches.len(), 1);
    assert_eq!(
        view.document.as_ref().unwrap().nodes[view.selected].path,
        "/1499/value"
    );
    assert_eq!(view.path.len(), 3);
    assert_eq!(view.matched_paths, view.path);
    assert!(!view.collapsed.contains(&0));
    assert_eq!(view.limits[&0], 1500);
    let layout = layout::build(
        view.document.as_ref().unwrap(),
        &view.collapsed,
        &view.limits,
        view.direction,
    );
    assert!(layout.nodes.iter().any(|(id, _)| *id == view.selected));
    view.query.clear();
    view.search();
    assert!(view.matches.is_empty());
    assert!(view.matched_paths.is_empty());
}

#[test]
fn invalid_input_is_retained_and_does_not_destroy_last_diagram() {
    let mut view = view(r#"{"old":1}"#);
    let context = egui::Context::default();
    view.start(|| parse_text("{\ninvalid".into(), Some(DataFormat::Json)));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while view.pending.is_some() && std::time::Instant::now() < deadline {
        frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(view.pending.is_none());
    assert_eq!(view.source, "{\ninvalid");
    assert!(view.error.as_ref().unwrap().contains('2'));
    assert!(view.source_open);
    assert!(view.source_changed);
    assert_eq!(view.document.as_ref().unwrap().nodes[1].key, "old");
    view.start(|| parse_text(r#"{"new":true}"#.into(), Some(DataFormat::Json)));
    while view.pending.is_some() && std::time::Instant::now() < deadline {
        frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(view.pending.is_none());
    assert!(view.error.is_none());
    assert!(!view.source_changed);
    assert_eq!(view.document.as_ref().unwrap().nodes[1].key, "new");
}

#[test]
fn keyboard_changes_selection_and_expansion() {
    let mut view = view(r#"{"a":{"b":1},"c":2}"#);
    let context = egui::Context::default();
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    context
        .run_ui(egui::RawInput::default(), |ui| {
            view.layout = layout::build(
                view.document.as_ref().unwrap(),
                &view.collapsed,
                &view.limits,
                view.direction,
            );
            view.keyboard(ui);
        })
        .drop_without_applying_deltas();
    for (event, expected) in [
        (egui::Key::ArrowRight, 1),
        (egui::Key::ArrowRight, 2),
        (egui::Key::ArrowLeft, 1),
    ] {
        context
            .run_ui(
                egui::RawInput {
                    events: vec![key(event)],
                    ..Default::default()
                },
                |ui| view.keyboard(ui),
            )
            .drop_without_applying_deltas();
        assert_eq!(view.selected, expected);
    }
    context
        .run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::Enter)],
                ..Default::default()
            },
            |ui| view.keyboard(ui),
        )
        .drop_without_applying_deltas();
    assert!(view.collapsed.contains(&1));
    assert_eq!(view.path, HashSet::from([0, 1]));
}

#[test]
fn dedicated_container_button_expands_it_in_edit_mode() {
    let mut view = view(r#"{"a":{"b":1}}"#);
    view.editing = true;
    view.collapsed.insert(1);
    view.dirty = true;
    let context = egui::Context::default();
    context.enable_accesskit();
    let output = frame(&mut view, &context, Vec::new());
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.label().is_some_and(|label| label.starts_with("a:")))
        .unwrap()
        .bounds()
        .unwrap();
    let position = Pos2::new(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    let expand_position = Pos2::new(bounds.x0 as f32 + 13.5, bounds.y0 as f32 + 13.5);
    output.drop_without_applying_deltas();

    frame(&mut view, &context, click_at(position)).drop_without_applying_deltas();
    assert!(view.collapsed.contains(&1));
    assert_eq!(view.selected, 1);

    frame(&mut view, &context, click_at(expand_position)).drop_without_applying_deltas();
    assert!(!view.collapsed.contains(&1));
}

#[test]
fn rendering_thousands_of_nodes_culls_offscreen_widgets() {
    let text = format!(
        "[{}]",
        (0..5000)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut view = view(&text);
    view.collapsed.clear();
    view.limits.insert(0, 5000);
    view.fit = false;
    view.zoom = 1.0;
    let context = egui::Context::default();
    frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
    let started = std::time::Instant::now();
    let output = frame(&mut view, &context, Vec::new());
    assert_eq!(view.layout.nodes.len(), 5001);
    assert!(
        output.shapes.len() < 500,
        "Offscreen nodes must not be painted"
    );
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "A frame must not freeze for a 5000-node document"
    );
    output.drop_without_applying_deltas();
}

#[test]
fn structure_mode_is_available_without_document_and_does_not_start_graph_worker() {
    let mut app = crate::app::StructViewApp::default();
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    let context = egui::Context::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| app.show_central_panel(ui),
        )
        .drop_without_applying_deltas();
    assert!(app.root.is_none());
    assert!(!app.graph_calculation.has_started());
}

#[test]
fn accessible_nodes_expose_full_labels_and_expanded_state() {
    let mut view = view(
        r#"{"a":{"long-key":"a very long value that should remain complete for screen readers"}}"#,
    );
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut output = frame(&mut view, &context, Vec::new());
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let node = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.label().is_some_and(|label| label.starts_with("a: ")))
        .unwrap();
    assert_eq!(node.is_expanded(), Some(true));
    assert!(
        update
            .nodes
            .iter()
            .any(|(_, node)| node.label().is_some_and(|label| label
                .contains("a very long value that should remain complete for screen readers")))
    );
    output.drop_without_applying_deltas();
    view.toggle(1);
    let output = frame(&mut view, &context, Vec::new());
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let node = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.label().is_some_and(|label| label.starts_with("a: ")))
        .unwrap();
    assert_eq!(node.is_expanded(), Some(false));
    output.drop_without_applying_deltas();
}

#[test]
fn replacing_document_invalidates_export_layout_before_using_node_indices() {
    let mut view = view(r#"{"a":[1,2,3],"b":{"c":true}}"#);
    view.ensure_layout();
    assert!(view.layout.nodes.len() > 2);
    let replacement = struct_view_core::structure::parse("false", Some(DataFormat::Json)).unwrap();
    view.reset(&replacement);
    view.document = Some(replacement);
    view.ensure_layout();
    assert_eq!(view.layout.nodes.len(), 1);
    let svg = export::svg(
        view.document.as_ref().unwrap(),
        &view.layout,
        &view.collapsed,
        &view.limits,
    );
    assert_eq!(svg.matches("<g>").count(), 1);
    assert_eq!(svg.matches("<polyline").count(), 0);
}

#[test]
fn canvas_omits_status_selection_legend_and_help() {
    let mut view = view(r#"{"graph":{"edges":[1,2,3,4,5,6,7]}}"#);
    let id = view
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .position(|n| n.path == "/graph/edges")
        .unwrap();
    view.select(id, true);
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut output = frame(&mut view, &context, Vec::new());
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let labels = update
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().or_else(|| node.value()))
        .collect::<Vec<_>>();
    assert!(
        !labels
            .iter()
            .any(|label| label.contains(Locale::English.text(TextKey::LegendObject)))
    );
    assert!(
        !labels
            .iter()
            .any(|label| label.contains("/graph/edges | edges: 7"))
    );
    assert!(!labels.contains(&Locale::English.text(TextKey::HelpCommonControls)));
    assert!(!labels.iter().any(|label| label.contains("nodes / visible")));
    assert!(
        !labels
            .iter()
            .any(|label| label.contains("Data structure diagram |"))
    );
    for key in [
        TextKey::Open,
        TextKey::StructureParse,
        TextKey::GraphFit,
        TextKey::DiagramExport,
    ] {
        assert!(
            !labels.contains(&Locale::English.text(key)),
            "Control {key:?} must only appear in the main menu"
        );
    }
    let is_text = |shape: &egui::epaint::Shape, text: &str| matches!(shape, egui::epaint::Shape::Text(shape) if shape.galley.job.text.contains(text));
    assert!(
        !output
            .shapes
            .iter()
            .any(|shape| is_text(&shape.shape, Locale::English.text(TextKey::LegendObject)))
    );
    assert!(!output.shapes.iter().any(|shape| is_text(
        &shape.shape,
        Locale::English.text(TextKey::HelpCommonControls)
    )));
    output.drop_without_applying_deltas();

    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| view.show(ui, Locale::Russian, None));
        },
    );
    output.textures_delta.clear();
    assert!(
        !output
            .shapes
            .iter()
            .any(|shape| is_text(&shape.shape, "/graph/edges | edges: 7"))
    );
    for text in [
        Locale::Russian.text(TextKey::LegendObject),
        Locale::Russian.text(TextKey::HelpCommonControls),
    ] {
        assert!(
            !output
                .shapes
                .iter()
                .any(|shape| is_text(&shape.shape, text)),
            "Legend must not remain on the canvas: {text}"
        );
    }
    output.drop_without_applying_deltas();
}

#[test]
fn structure_summary_appears_once_in_status_bar_not_canvas() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.locale = locale;
        app.visualization = crate::app::visualization::VisualizationMode::Structure;
        app.structure_view = view(r#"{"name":"example"}"#);
        app.structure_view.origin = Some(PathBuf::from("example.json"));
        app.structure_view.select(1, false);
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| {
                app.show_bottom_panel(ui);
                egui::CentralPanel::default()
                    .show(ui, |ui| app.structure_view.show(ui, locale, None));
            },
        );
        output.textures_delta.clear();
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        let summaries = update
            .nodes
            .iter()
            .filter_map(|(_, node)| node.value().or_else(|| node.label()))
            .filter(|text| text.contains(locale.text(TextKey::StructureNodes)))
            .collect::<Vec<_>>();
        assert!(!summaries.is_empty());
        assert_eq!(
            output
                .shapes
                .iter()
                .filter(|shape| matches!(
                    &shape.shape, egui::epaint::Shape::Text(text)
                        if text.galley.job.text.contains(locale.text(TextKey::StructureNodes))
                ))
                .count(),
            1,
            "Structure metadata must be painted only once"
        );
        assert!(summaries[0].starts_with("2 / 2"));
        for label in ["📄 example.json", "JSON", "/name", "name: example"] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(label)),
                "Missing status field: {label}"
            );
            assert_eq!(output.shapes.iter().filter(|shape| matches!(
                &shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == label
            )).count(), 1, "Status field must appear once: {label}");
        }
        assert!(!summaries[0].contains('|'));
        assert!(
            !update
                .nodes
                .iter()
                .any(|(_, node)| node.value() == Some(locale.text(TextKey::StructureView)))
        );
        output.drop_without_applying_deltas();
    }
}

#[test]
fn collapse_and_expand_center_the_changed_node_in_every_layout() {
    for direction in [
        Direction::Vertical,
        Direction::Horizontal,
        Direction::Compact,
    ] {
        let mut view = view(r#"{"a":{"b":1,"c":2},"other":3}"#);
        view.direction = direction;
        let context = egui::Context::default();
        frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
        let zoom = view.zoom;
        for collapsed in [true, false] {
            view.pan = Vec2::new(-700.0, 250.0);
            view.toggle(1);
            frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
            assert_eq!(view.collapsed.contains(&1), collapsed);
            let rect = view.layout.nodes.iter().find(|(id, _)| *id == 1).unwrap().1;
            assert!(
                (rect.center().to_vec2() * view.zoom + view.pan - view.canvas_size * 0.5).length()
                    < 0.01
            );
            assert_eq!(view.zoom, zoom, "Branch changes must not alter zoom");
            assert!(!view.center_selected);
        }
    }
}

fn app_frame(
    app: &mut crate::app::StructViewApp,
    context: &egui::Context,
    width: f32,
    mut events: Vec<egui::Event>,
) -> egui::FullOutput {
    let modifiers = events
        .iter()
        .find_map(|event| {
            if let egui::Event::Key { modifiers, .. } = event {
                Some(*modifiers)
            } else {
                None
            }
        })
        .unwrap_or_default();
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 800.0))),
            events,
            ..Default::default()
        },
        |ui| {
            app.show_top_panel(ui);
            app.show_central_panel(ui);
        },
    )
}

fn label_center(output: &egui::FullOutput, label: &str) -> Pos2 {
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let bounds = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| {
            node.label()
                .is_some_and(|text| text.trim_end_matches([' ', '⏵']) == label)
        })
        .unwrap_or_else(|| {
            panic!(
                "Missing menu item: {label}; labels: {:?}",
                update
                    .nodes
                    .iter()
                    .filter_map(|(_, node)| node.label())
                    .collect::<Vec<_>>()
            )
        })
        .bounds()
        .unwrap();
    Pos2::new(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    )
}

fn click_at(position: Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn right_click_at(position: Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn main_menu_exposes_structure_controls_and_opens_source() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    app.structure_view = view(r#"{"a":[1,2],"b":true}"#);
    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let position = label_center(&output, "View");
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(position)).drop_without_applying_deltas();
    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    for key in [
        TextKey::StructureSource,
        TextKey::StructureLayout,
        TextKey::GraphZoomIn,
        TextKey::GraphZoomOut,
        TextKey::GraphFit,
        TextKey::StructureToggle,
    ] {
        label_center(&output, Locale::English.text(key));
    }
    let position = label_center(&output, Locale::English.text(TextKey::StructureSource));
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(position)).drop_without_applying_deltas();
    assert!(app.structure_view.source_open);
    app.structure_view.source_open = false;
    egui::Popup::close_all(&context);
    for (menu, item) in [
        ("File", TextKey::StructureParse),
        ("Settings", TextKey::StructureFormat),
        ("Edit", TextKey::SearchWindow),
    ] {
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let position = label_center(&output, menu);
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(position)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        label_center(&output, Locale::English.text(item));
        if menu == "File" {
            label_center(&output, Locale::English.text(TextKey::DiagramExport));
        }
        output.drop_without_applying_deltas();
        egui::Popup::close_all(&context);
    }
}

#[test]
fn find_shortcut_opens_shared_search_window_on_wide_and_narrow_windows() {
    for width in [1000.0, 400.0] {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.locale = Locale::English;
        app.visualization = crate::app::visualization::VisualizationMode::Structure;
        app.structure_view = view(r#"{"needle":1}"#);
        app_frame(&mut app, &context, width, Vec::new()).drop_without_applying_deltas();
        let event = egui::Event::Key {
            key: egui::Key::F,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        app_frame(&mut app, &context, width, vec![event]).drop_without_applying_deltas();
        app_frame(&mut app, &context, width, Vec::new()).drop_without_applying_deltas();
        assert!(
            app.search_window_open,
            "Shared search must be open at width {width}"
        );
        let output = app_frame(&mut app, &context, width, Vec::new());
        label_center(&output, Locale::English.text(TextKey::SearchWindow));
        output.drop_without_applying_deltas();
    }
}

#[test]
fn shared_search_options_navigation_and_mode_switches_preserve_structure_paths() {
    let mut app = crate::app::StructViewApp::default();
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    app.structure_view = view(r#"{"a":{"Name":"Needle"},"b":{"Name":"needle"},"plain":"other"}"#);
    app.search_query_buf = "Name: needle".into();
    app.refresh_search();
    assert_eq!(app.search.matches, ["/a/Name", "/b/Name"]);
    app.search.next();
    app.request_search_scroll();
    assert_eq!(
        app.structure_view.document.as_ref().unwrap().nodes[app.structure_view.selected].path,
        "/b/Name"
    );
    assert_eq!(app.structure_view.path.len(), 3);

    app.search.options.case_sensitive = true;
    app.refresh_search();
    assert_eq!(app.search.matches, ["/b/Name"]);
    app.search.options.use_regex = true;
    app.search_query_buf = "^/a/Name$".into();
    app.search.options.search_keys = false;
    app.search.options.search_values = false;
    app.refresh_search();
    assert_eq!(app.search.matches, ["/a/Name"]);
    app.search_query_buf = "[".into();
    app.refresh_search();
    assert!(app.search.error.is_some());
    assert!(app.structure_view.matches.is_empty());
    assert!(app.structure_view.matched_paths.is_empty());

    app.search.options = SearchOptions::default();
    app.search_query_buf = "needle".into();
    app.root = Some(struct_view_core::parser::parse_json(r#"{"original":"needle"}"#).unwrap());
    app.visualization = crate::app::visualization::VisualizationMode::Graph;
    app.refresh_search();
    assert_eq!(app.search.matches, ["original"]);
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    app.refresh_search();
    assert_eq!(app.search.matches, ["/a/Name", "/b/Name"]);
}

#[test]
fn expand_and_collapse_are_restored_in_common_tree_actions_menu() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    app.structure_view = view(r#"{"a":{"b":{"c":[1,2]}}}"#);
    for expanded in [true, false] {
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(&output, "View");
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(&output, Locale::English.text(TextKey::TreeActions));
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(
            &output,
            Locale::English.text(if expanded {
                TextKey::ExpandAll
            } else {
                TextKey::CollapseAll
            }),
        );
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        assert_eq!(app.structure_view.collapsed.is_empty(), expanded);
        assert_eq!(
            app.structure_view.layout.nodes.len(),
            if expanded { 6 } else { 1 }
        );
        egui::Popup::close_all(&context);
    }
}

#[test]
fn structure_is_only_in_the_visualization_submenu() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let pos = label_center(&output, "View");
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    assert!(
        !output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, n)| n.label() == Some(Locale::English.text(TextKey::StructureView)))
    );
    let pos = label_center(&output, Locale::English.text(TextKey::Visualization));
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let pos = label_center(&output, Locale::English.text(TextKey::StructureView));
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
    assert_eq!(
        app.visualization,
        crate::app::visualization::VisualizationMode::Structure
    );
}

#[test]
fn both_diagrams_use_identical_menu_controls_and_shared_search_window() {
    for mode in [
        crate::app::visualization::VisualizationMode::Structure,
        crate::app::visualization::VisualizationMode::Graph,
    ] {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.locale = Locale::English;
        app.visualization = mode;
        let input = r#"[{"id":"a","name":"Source","depends_on":"b"},{"id":"b","name":"Target"}]"#;
        app.root = Some(struct_view_core::parser::parse_json(input).unwrap());
        app.structure_view = view(input);
        if mode == crate::app::visualization::VisualizationMode::Graph {
            app.graph_calculation.ensure_started(
                app.root.as_ref().unwrap(),
                app.graph_routing_workers,
                app.graph_routing_backend,
                &context,
            );
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while app.graph_calculation.result().is_none() && std::time::Instant::now() < deadline {
                app.graph_calculation.poll(&context);
                std::thread::sleep(Duration::from_millis(1));
            }

            assert!(app.graph_calculation.result().is_some());
        }
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(&output, "View");
        label_center(&output, "🔍");
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        for key in [
            TextKey::GraphZoomIn,
            TextKey::GraphZoomOut,
            TextKey::GraphFit,
            TextKey::GraphSelectAll,
            TextKey::GraphClearSelection,
            TextKey::TreeActions,
        ] {
            label_center(&output, Locale::English.text(key));
        }
        label_center(&output, "100%");
        let pos = label_center(&output, Locale::English.text(TextKey::GraphZoomIn));
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        egui::Popup::close_all(&context);
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(&output, "View");
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(
            update
                .nodes
                .iter()
                .any(|(_, node)| node.value().is_some_and(|s| s.ends_with('%')))
        );
        let pos = label_center(&output, "100%");
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        if mode == crate::app::visualization::VisualizationMode::Structure {
            assert_eq!(app.structure_view.zoom, 1.0);
        }
        egui::Popup::close_all(&context);
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let pos = label_center(&output, "File");
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        label_center(&output, Locale::English.text(TextKey::DiagramExport));
        output.drop_without_applying_deltas();
        egui::Popup::close_all(&context);
        app.search_window_open = true;
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        for key in [
            TextKey::SearchWindow,
            TextKey::SearchKeys,
            TextKey::SearchValues,
            TextKey::SearchPaths,
            TextKey::RegexSearch,
            TextKey::CaseSensitive,
        ] {
            label_center(&output, Locale::English.text(key));
        }
        output.drop_without_applying_deltas();
    }
}

#[test]
fn edit_mode_updates_selected_field_and_marks_source_unsaved() {
    let mut view = view(r#"{"name":"old"}"#);
    view.selected = 1;
    view.open_edit_dialog();
    assert!(view.edit_dialog.is_some(), "{:?}", view.error);
    let mut dialog = view.edit_dialog.take().unwrap();
    dialog.key = "title".into();
    dialog.value = "new".into();

    view.apply_edit_dialog(&dialog).unwrap();

    let document = view.document.as_ref().unwrap();
    assert_eq!(document.nodes[1].key, "title");
    assert_eq!(document.nodes[1].value, "new");
    assert!(view.source.contains("\"title\""));
    assert!(view.unsaved);
    assert!(!view.source_changed);
}

#[test]
fn structure_view_displays_adds_and_edits_comments_in_supported_formats() {
    for (format, source, marker) in [
        (DataFormat::Json5, "// existing\n{value: 1,}", "//"),
        (DataFormat::Yaml, "# existing\nvalue: 1\n", "#"),
        (DataFormat::Toml, "# existing\nvalue = 1\n", "#"),
    ] {
        let mut view = view_with_format(source, format);
        let initial_comment = view
            .document
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.kind == Kind::Comment)
            .unwrap();
        assert_eq!(initial_comment.value, format!("{marker} existing"));

        view.editing = true;
        view.open_add_dialog();
        let mut dialog = view.edit_dialog.take().unwrap();
        dialog.value_type = JsonValueType::Comment;
        dialog.value = "added note".into();
        view.apply_edit_dialog(&dialog).unwrap();

        assert!(view.source.contains(&format!("{marker} added note")));
        assert_eq!(
            view.document
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .filter(|node| node.kind == Kind::Comment)
                .count(),
            2
        );

        let context = egui::Context::default();
        context.enable_accesskit();
        let output = frame(&mut view, &context, Vec::new());
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(update.nodes.iter().any(|(_, node)| {
            node.label()
                .is_some_and(|label| label.contains("Comment") && label.contains("existing"))
        }));
        output.drop_without_applying_deltas();

        let comment_id = view
            .document
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .position(|node| node.kind == Kind::Comment && node.value.contains("added note"))
            .unwrap();
        view.selected = comment_id;
        view.selected_nodes = HashSet::from([comment_id]);
        view.open_edit_dialog();
        let mut comment_dialog = view.edit_dialog.take().unwrap();
        assert_eq!(comment_dialog.value_type, JsonValueType::Comment);
        comment_dialog.value = "edited note".into();
        view.apply_edit_dialog(&comment_dialog).unwrap();

        assert!(view.source.contains(&format!("{marker} edited note")));
        assert!(view.unsaved);
    }
}

#[test]
fn selected_structure_can_be_deleted_and_root_is_protected() {
    let mut view = view(r#"{"keep":1,"remove":{"nested":true}}"#);
    assert!(!view.can_delete_selected());
    view.editing = true;
    view.selected = 2;
    view.selected_nodes = HashSet::from([2]);
    assert!(view.can_delete_selected());

    view.delete_selected(Locale::English);

    let document = view.document.as_ref().unwrap();
    assert_eq!(document.nodes.len(), 2);
    assert_eq!(document.nodes[1].key, "keep");
    assert!(!view.source.contains("remove"));
    assert!(view.unsaved);
    assert_eq!(view.notice.as_deref(), Some("Structures deleted: 1"));

    view.selected = 0;
    view.selected_nodes = HashSet::from([0]);
    assert!(!view.can_delete_selected());
    view.delete_selected(Locale::English);
    assert_eq!(
        view.error.as_deref(),
        Some(Locale::English.text(TextKey::CannotDeleteRoot))
    );
    assert_eq!(view.document.as_ref().unwrap().nodes.len(), 2);
}

#[test]
fn delete_key_removes_selected_structure_from_structure_view() {
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    let mut structure_view = view(r#"{"keep":1,"remove":2}"#);
    structure_view.editing = true;
    app.structure_view = structure_view;
    let context = egui::Context::default();
    context.enable_accesskit();

    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let bounds = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| {
            node.label()
                .is_some_and(|label| label.starts_with("remove:"))
        })
        .unwrap()
        .bounds()
        .unwrap();
    let node_position = Pos2::new(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(node_position)).drop_without_applying_deltas();
    assert_eq!(app.structure_view.selected_nodes, HashSet::from([2]));
    assert!(app.structure_view.can_delete_selected());
    assert!(
        context.egui_wants_keyboard_input(),
        "The canvas focus should exercise the keyboard-input guard"
    );

    let delete_key = egui::Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app_frame(&mut app, &context, 1000.0, vec![delete_key]).drop_without_applying_deltas();

    assert!(!app.structure_view.source.contains("remove"));
    assert_eq!(app.structure_view.document.as_ref().unwrap().nodes.len(), 2);
}

#[test]
fn structure_delete_menu_button_removes_selected_structure() {
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    let mut structure_view = view(r#"{"keep":1,"remove":2}"#);
    structure_view.editing = true;
    structure_view.selected = 2;
    structure_view.selected_nodes = HashSet::from([2]);
    app.structure_view = structure_view;
    let context = egui::Context::default();
    context.enable_accesskit();

    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let edit_menu_position = label_center(&output, "Edit");
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(edit_menu_position))
        .drop_without_applying_deltas();

    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let delete_position = label_center(
        &output,
        Locale::English.text(TextKey::DeleteSelectedStructures),
    );
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(delete_position)).drop_without_applying_deltas();

    assert!(!app.structure_view.source.contains("remove"));
    assert_eq!(app.structure_view.document.as_ref().unwrap().nodes.len(), 2);
}

#[test]
fn structure_context_delete_button_removes_the_context_node() {
    let mut structure_view = view(r#"{"keep":1,"remove":2}"#);
    structure_view.editing = true;
    let context = egui::Context::default();
    context.enable_accesskit();

    let output = frame(&mut structure_view, &context, Vec::new());
    let update = output.platform_output.accesskit_update.as_ref().unwrap();
    let bounds = update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| {
            node.label()
                .is_some_and(|label| label.starts_with("remove:"))
        })
        .unwrap()
        .bounds()
        .unwrap();
    let node_position = Pos2::new(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    output.drop_without_applying_deltas();
    frame(&mut structure_view, &context, right_click_at(node_position))
        .drop_without_applying_deltas();

    let output = frame(&mut structure_view, &context, Vec::new());
    let delete_position = label_center(
        &output,
        Locale::English.text(TextKey::DeleteSelectedStructures),
    );
    output.drop_without_applying_deltas();
    frame(&mut structure_view, &context, click_at(delete_position)).drop_without_applying_deltas();

    assert!(!structure_view.source.contains("remove"));
    assert_eq!(structure_view.document.as_ref().unwrap().nodes.len(), 2);
}

#[test]
fn tree_delete_menu_button_removes_selected_structure() {
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.mode = crate::app::state::AppMode::Edit;
    let (root, format) =
        struct_view_core::parser::parse_data(r#"{"keep":1,"remove":2}"#, None).unwrap();
    app.file_state.format = Some(format);
    app.root = Some(root);
    app.visualization = crate::app::visualization::VisualizationMode::Tree;
    let remove_path = app.root.as_ref().unwrap().children[1].path.clone();
    app.selected_paths.insert(remove_path);
    let context = egui::Context::default();
    context.enable_accesskit();

    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let edit_menu_position = label_center(&output, "Edit");
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(edit_menu_position))
        .drop_without_applying_deltas();

    let output = app_frame(&mut app, &context, 1000.0, Vec::new());
    let delete_position = label_center(
        &output,
        Locale::English.text(TextKey::DeleteSelectedStructures),
    );
    output.drop_without_applying_deltas();
    app_frame(&mut app, &context, 1000.0, click_at(delete_position)).drop_without_applying_deltas();

    assert!(
        struct_view_core::parser::node_to_value(app.root.as_ref().unwrap())
            .unwrap()
            .get("remove")
            .is_none()
    );
}

#[test]
fn delete_key_removes_selected_structure_from_tree_view() {
    let mut app = crate::app::StructViewApp::default();
    app.locale = Locale::English;
    app.mode = crate::app::state::AppMode::Edit;
    let (root, format) =
        struct_view_core::parser::parse_data(r#"{"keep":1,"remove":2}"#, None).unwrap();
    app.file_state.format = Some(format);
    app.root = Some(root);
    let remove_path = app.root.as_ref().unwrap().children[1].path.clone();
    app.selected_paths.insert(remove_path);
    let context = egui::Context::default();
    app_frame(&mut app, &context, 1000.0, Vec::new()).drop_without_applying_deltas();

    let delete_key = egui::Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app_frame(&mut app, &context, 1000.0, vec![delete_key]).drop_without_applying_deltas();

    assert!(
        struct_view_core::parser::node_to_value(app.root.as_ref().unwrap())
            .unwrap()
            .get("remove")
            .is_none()
    );
}

#[test]
fn multiple_selected_structures_can_be_deleted_together() {
    let mut view = view(r#"{"first":1,"second":2,"keep":3}"#);
    view.editing = true;
    view.selected_nodes = HashSet::from([1, 2]);

    view.delete_selected(Locale::English);

    let document = view.document.as_ref().unwrap();
    assert_eq!(document.nodes.len(), 2);
    assert_eq!(document.nodes[1].key, "keep");
    assert_eq!(view.notice.as_deref(), Some("Structures deleted: 2"));
}

#[test]
fn selected_structures_can_be_copied_and_pasted_into_a_container() {
    let mut view = view(r#"{"profile":{"name":"Ada"},"tags":["admin"]}"#);
    view.editing = true;
    view.selected_nodes = HashSet::from([1]);
    assert!(view.can_copy_selected());

    view.copy_selected(Locale::English);
    assert!(
        view.error
            .as_deref()
            .is_none_or(|error| error.starts_with("System clipboard copy error:")),
        "Unexpected copy error: {:?}",
        view.error
    );
    let entries = view.clipboard_payload.clone().expect("entries cached");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key.as_deref(), Some("profile"));

    // Target the array container "tags" to paste the copied object as a new element.
    let document = view.document.as_ref().unwrap();
    let tags_id = document
        .nodes
        .iter()
        .position(|node| node.key == "tags")
        .unwrap();
    view.selected_nodes = HashSet::from([tags_id]);
    assert!(view.can_paste_into_selected());

    view.paste_into_selected(Locale::English);

    assert_eq!(view.error, None);
    assert_eq!(view.notice.as_deref(), Some("Structures pasted: 1"));
    assert!(view.unsaved);
    let document = view.document.as_ref().unwrap();
    let tags_id = document
        .nodes
        .iter()
        .position(|node| node.key == "tags")
        .unwrap();
    assert_eq!(document.nodes[tags_id].children.len(), 2);
}

#[test]
fn pasting_requires_editing_mode_and_a_single_container_selected() {
    let mut view = view(r#"{"keep":1,"other":2}"#);
    view.selected_nodes = HashSet::from([1]);
    view.copy_selected(Locale::English);
    assert!(view.clipboard_payload.is_some());

    // View mode: paste is disabled even though a container is selected.
    view.selected_nodes = HashSet::from([0]);
    assert!(!view.can_paste_into_selected());

    view.editing = true;
    assert!(view.can_paste_into_selected());

    // Selecting a scalar field disables paste.
    view.selected_nodes = HashSet::from([1]);
    assert!(!view.can_paste_into_selected());

    // Selecting more than one node disables paste.
    view.selected_nodes = HashSet::from([0, 1]);
    assert!(!view.can_paste_into_selected());
}

#[test]
fn source_text_editor_highlights_syntax_for_supported_data_formats() {
    let colors = super::super::super::theme::SyntaxColors::new(&egui::Visuals::dark());
    let cases = [
        (
            DataFormat::Json,
            r#"{"name":"Ada","age":42,"enabled":true}"#,
            vec![
                ("\"name\"", colors.key),
                ("\"Ada\"", colors.string),
                ("42", colors.number),
                ("true", colors.boolean),
            ],
        ),
        (
            DataFormat::Yaml,
            "name: Ada\ncount: 3 # note",
            vec![
                ("name", colors.key),
                ("Ada", colors.string),
                ("3", colors.number),
                ("# note", colors.comment),
            ],
        ),
        (
            DataFormat::Toml,
            "name = 'Ada'\nactive = true",
            vec![
                ("name", colors.key),
                ("'Ada'", colors.string),
                ("true", colors.boolean),
            ],
        ),
    ];

    for (format, text, expected) in cases {
        let job = super::StructureView::source_syntax_job(text, format, colors, 14.0);
        for (token, color) in expected {
            let start = text.find(token).unwrap();
            let section = job
                .sections
                .iter()
                .find(|section| {
                    section.byte_range.start <= egui::text::ByteIndex(start)
                        && section.byte_range.end >= egui::text::ByteIndex(start + token.len())
                })
                .unwrap_or_else(|| panic!("No syntax section for {token:?} in {text:?}"));
            assert_eq!(section.format.color, color, "wrong color for {token:?}");
        }
    }
}

#[test]
fn source_window_preview_tracks_selected_values_and_includes_descendants() {
    let mut view =
        view(r#"{"keep":0,"resource":{"id":7,"items":[{"name":"one"},{"name":"two"}]}}"#);
    let resource = view
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .position(|node| node.key == "resource")
        .unwrap();
    view.select(resource, false);

    let preview = view
        .selected_source_preview(resource, DataFormat::Json)
        .unwrap();
    assert!(preview.contains("\"id\": 7"));
    assert!(preview.contains("\"items\""));
    assert!(preview.contains("\"name\": \"one\""));
    assert!(preview.contains("\"name\": \"two\""));
    assert!(!preview.contains("\"keep\""));

    let id = view
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .position(|node| node.key == "id")
        .unwrap();
    view.select(id, false);
    assert_eq!(
        view.selected_source_preview(id, DataFormat::Json).unwrap(),
        "{\n  \"id\": 7\n}"
    );
}

#[test]
fn structure_canvas_selection_matches_graph_toggle_and_additive_behavior() {
    let mut view = view(r#"{"first":{"nested":1},"second":2}"#);
    let first = 1;
    let second = 3;

    view.select_canvas_node(first, false);
    assert_eq!(view.selected_nodes, HashSet::from([first]));
    assert!(!view.collapsed.contains(&first));
    view.select_canvas_node(first, false);
    assert!(view.selected_nodes.is_empty());

    view.select_canvas_node(first, false);
    view.select_canvas_node(second, true);
    assert_eq!(view.selected_nodes, HashSet::from([first, second]));
    view.select_canvas_node(first, true);
    assert_eq!(view.selected_nodes, HashSet::from([second]));
    view.select_canvas_node(first, false);
    assert_eq!(view.selected_nodes, HashSet::from([first]));
}

#[test]
fn dedicated_expand_control_toggles_children_without_changing_selection() {
    let mut view = view(r#"{"first":{"nested":1},"second":2}"#);
    let first = 1;
    let second = 3;
    view.select_canvas_node(second, false);
    view.collapsed.insert(first);

    view.toggle_expansion(first);

    assert!(!view.collapsed.contains(&first));
    assert_eq!(view.selected_nodes, HashSet::from([second]));
    assert_eq!(view.selected, second);

    view.toggle_expansion(first);
    assert!(view.collapsed.contains(&first));
    assert_eq!(view.selected_nodes, HashSet::from([second]));
}
#[test]
fn plus_constructor_adds_typed_fields_and_array_items() {
    let mut view = view(r#"{"items":[]}"#);
    view.open_add_dialog();
    assert!(view.edit_dialog.is_some(), "{:?}", view.error);
    let mut dialog = view.edit_dialog.take().unwrap();
    dialog.key = "count".into();
    dialog.value_type = JsonValueType::Number;
    dialog.value = "42".into();
    view.apply_edit_dialog(&dialog).unwrap();
    assert!(view.source.contains("\"count\": 42"));

    view.selected = view
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .position(|node| node.key == "items")
        .unwrap();
    view.open_add_dialog();
    assert!(view.edit_dialog.is_some(), "{:?}", view.error);
    let mut dialog = view.edit_dialog.take().unwrap();
    dialog.value_type = JsonValueType::Bool;
    dialog.value = "true".into();
    view.apply_edit_dialog(&dialog).unwrap();

    let document = view.document.as_ref().unwrap();
    let added = document.nodes.iter().find(|node| node.key == "0").unwrap();
    assert_eq!(added.value, "true");
}

#[test]
fn edits_reject_ambiguous_duplicate_keys() {
    let mut view = view(r#"{"same":1,"same":2}"#);
    view.selected = 2;
    view.open_edit_dialog();
    assert!(view.edit_dialog.is_none());
    assert!(
        view.error
            .as_deref()
            .is_some_and(|error| error.to_lowercase().contains("повторяется"))
    );
}

#[test]
fn saving_structure_edits_writes_the_source_and_clears_unsaved_state() {
    let mut view = view(r#"{"name":"old"}"#);
    view.source = r#"{"name":"new"}"#.into();
    view.unsaved = true;
    let path = std::env::temp_dir().join(format!(
        "structview-structure-save-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    view.write_source(&path, Locale::English);

    assert_eq!(std::fs::read_to_string(&path).unwrap(), view.source);
    assert_eq!(view.origin.as_deref(), Some(path.as_path()));
    assert!(!view.unsaved);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn structure_constructor_edits_yaml_and_toml_documents() {
    for (source, format) in [
        ("name: old\n", DataFormat::Yaml),
        ("name = \"old\"\n", DataFormat::Toml),
    ] {
        let document = struct_view_core::structure::parse(source, Some(format)).unwrap();
        let mut view = StructureView::default();
        view.reset(&document);
        view.document = Some(document);
        view.source = source.to_string();
        view.format = Some(format);
        view.open_add_dialog();
        let mut dialog = view.edit_dialog.take().unwrap();
        dialog.key = "count".into();
        dialog.value_type = JsonValueType::Number;
        dialog.value = "7".into();

        view.apply_edit_dialog(&dialog).unwrap();

        let edited = struct_view_core::structure::parse(&view.source, Some(format)).unwrap();
        assert!(
            edited
                .nodes
                .iter()
                .any(|node| node.key == "count" && node.value == "7")
        );
    }
}

#[test]
fn closing_modified_structure_source_requires_confirmation_and_saves_it() {
    let path = std::env::temp_dir().join(format!(
        "structview-structure-close-save-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, r#"{"value":1}"#).unwrap();

    let mut app = crate::app::StructViewApp::default();
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    let mut source_view = view(r#"{"value":2}"#);
    source_view.origin = Some(path.clone());
    source_view.unsaved = true;
    app.structure_view = source_view;

    app.request_close_file();

    assert!(app.close_file_confirmation_open);
    assert!(app.save_changes_and_close_file());
    assert!(app.root.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"value":2}"#);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn replacing_modified_structure_source_can_be_cancelled_or_saved() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.json");
    let next = directory.path().join("next.yaml");
    std::fs::write(&original, r#"{"value":1}"#).unwrap();
    std::fs::write(&next, "value: 3\n").unwrap();
    let mut app = crate::app::StructViewApp::default();
    app.visualization = crate::app::visualization::VisualizationMode::Structure;
    app.structure_view = view(r#"{"value":2}"#);
    app.structure_view.origin = Some(original.clone());
    app.structure_view.unsaved = true;

    app.request_document_replacement(crate::app::state::DocumentReplacement::OpenStructure(
        next.clone(),
    ));
    assert!(app.close_file_confirmation_open);
    assert!(app.structure_view.pending.is_none());
    app.cancel_close_file_confirmation();
    assert!(app.structure_view.unsaved);
    assert_eq!(app.structure_view.origin, Some(original.clone()));

    app.request_document_replacement(crate::app::state::DocumentReplacement::OpenStructure(
        next.clone(),
    ));
    assert!(app.save_changes_and_close_file());
    assert_eq!(
        std::fs::read_to_string(&original).unwrap(),
        r#"{"value":2}"#
    );
    let context = egui::Context::default();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.structure_view.pending.is_some() && std::time::Instant::now() < deadline {
        frame(&mut app.structure_view, &context, Vec::new()).drop_without_applying_deltas();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.structure_view.pending.is_none());
    assert_eq!(app.structure_view.source, "value: 3\n");
    assert_eq!(app.structure_view.origin, Some(next));
    assert_eq!(app.structure_view.format, Some(DataFormat::Yaml));
    assert!(!app.structure_view.unsaved);
}

#[test]
fn failed_structure_file_load_preserves_source_format_and_unsaved_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut view = view(r#"{"value":2}"#);
    view.unsaved = true;
    view.origin = Some(directory.path().join("original.json"));
    let origin = view.origin.clone();
    view.open(directory.path().join("missing.toml"));
    let context = egui::Context::default();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while view.pending.is_some() && std::time::Instant::now() < deadline {
        frame(&mut view, &context, Vec::new()).drop_without_applying_deltas();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(view.pending.is_none());
    assert!(view.error.is_some());
    assert!(view.unsaved);
    assert_eq!(view.source, r#"{"value":2}"#);
    assert_eq!(view.origin, origin);
    assert_eq!(view.format, Some(DataFormat::Json));
    assert!(view.document.is_some());
}

#[test]
fn dropped_structure_file_waits_for_unsaved_changes_confirmation() {
    let mut view = view(r#"{"value":2}"#);
    view.unsaved = true;
    let path = PathBuf::from("next.json");
    view.request_open(path.clone());
    assert_eq!(view.requested_open, Some(path));
    assert!(view.pending.is_none());
    assert!(view.unsaved);
    assert_eq!(view.source, r#"{"value":2}"#);
}

#[test]
fn exit_confirmation_cancels_pending_structure_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let next = directory.path().join("next.json");
    std::fs::write(&next, r#"{"value":3}"#).unwrap();
    let mut app = crate::app::StructViewApp::default();
    app.structure_view = view(r#"{"value":2}"#);
    app.structure_view.unsaved = true;
    app.structure_view.open(next);
    assert!(app.structure_view.pending.is_some());
    app.request_exit(&egui::Context::default());
    assert!(app.close_file_confirmation_open);
    assert!(app.exit_after_close_confirmation);
    assert!(app.structure_view.pending.is_none());
    assert!(app.structure_view.pending_origin.is_none());
    assert!(app.structure_view.unsaved);
    assert_eq!(app.structure_view.source, r#"{"value":2}"#);
}
