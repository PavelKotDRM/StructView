use super::*;
mod help;

fn view(text: &str) -> StructureView {
    let document = struct_view_core::structure::parse(text, Some(DataFormat::Json)).unwrap();
    let mut view = StructureView::default();
    view.reset(&document);
    view.document = Some(document);
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
        TextKey::StructureToggle,
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
            app.graph_calculation
                .ensure_started(app.root.as_ref().unwrap(), &context);
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
