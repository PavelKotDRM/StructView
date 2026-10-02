use super::{
    PairChange, comparison_column_width, comparison_value_color, document_label_for_path,
    pair_change,
};

use super::super::theme::SyntaxColors;
use egui::Color32;
use serde_json::json;
use struct_view_core::parser::DataFormat;

#[test]
fn diff_file_labels_include_format() {
    assert_eq!(
        document_label_for_path(std::path::Path::new("before.json"), DataFormat::Json),
        "before.json (JSON)"
    );
}

#[test]
fn pair_diff_classifies_missing_changed_and_unchanged_values() {
    assert_eq!(pair_change(None, Some(&json!(1))), Some(PairChange::Added));
    assert_eq!(
        pair_change(Some(&json!(1)), None),
        Some(PairChange::Removed)
    );
    assert_eq!(
        pair_change(Some(&json!(1)), Some(&json!(2))),
        Some(PairChange::Changed)
    );
    assert_eq!(pair_change(Some(&json!(null)), Some(&json!(null))), None);
    assert_eq!(pair_change(None, None), None);
    let decimal_one: serde_json::Value = serde_json::from_str("1.00").unwrap();
    let decimal_two: serde_json::Value = serde_json::from_str("1.0").unwrap();
    assert_eq!(pair_change(Some(&decimal_one), Some(&decimal_two)), None);
}

#[test]
fn comparison_colors_reflect_added_removed_and_changed_values() {
    let original = json!(1);
    let updated = json!(2);
    let unchanged_color = Color32::WHITE;
    let colors = SyntaxColors::new(&egui::Visuals::dark());

    assert_eq!(
        comparison_value_color(None, Some(&updated), unchanged_color, colors),
        colors.success
    );
    assert_eq!(
        comparison_value_color(Some(&original), None, unchanged_color, colors),
        colors.error
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&updated), unchanged_color, colors),
        colors.matched
    );
    assert_eq!(
        comparison_value_color(Some(&original), Some(&original), unchanged_color, colors),
        unchanged_color
    );
}

#[test]
fn comparison_column_width_tracks_the_available_window_width() {
    assert_eq!(comparison_column_width(612.0, 4), 144.0);
    assert_eq!(comparison_column_width(200.0, 4), 96.0);
}

fn window_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(320.0, 240.0),
        )),
        ..Default::default()
    }
}

fn render_graph(
    ui: &mut egui::Ui,
    graph: &super::super::visualization::RelationshipGraph,
    search: &struct_view_core::search::SearchState,
    locale: super::super::i18n::Locale,
) {
    let routing = super::graph::build_graph_routing_layout(graph);
    super::show_graph(ui, graph, &routing, search, locale);
}

fn graph_input(
    pointer_position: Option<egui::Pos2>,
    primary_pressed: Option<bool>,
) -> egui::RawInput {
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 450.0),
    ));
    if let Some(position) = pointer_position {
        input.events.push(egui::Event::PointerMoved(position));
        if let Some(pressed) = primary_pressed {
            input.events.push(egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    input
}

fn text_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::TextShape> {
    fn collect<'a>(shape: &'a egui::epaint::Shape, output: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::epaint::Shape::Text(text) => output.push(text),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, output);
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for shape in shapes {
        collect(&shape.shape, &mut texts);
    }
    texts
}

fn text_origin(output: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
    text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == text)
        .map(|shape| shape.pos)
}

fn text_color(output: &egui::FullOutput, text: &str) -> Option<egui::Color32> {
    text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == text)
        .map(|shape| shape.fallback_color)
}

#[test]
fn rendering_table_headers_scroll_with_their_columns() {
    let root = struct_view_core::parser::parse_json(r#"{"value":42}"#).unwrap();
    let table = super::super::visualization::build_table(&root);
    let context = egui::Context::default();
    let search = struct_view_core::search::SearchState::default();
    let mut scroll_id = egui::Id::NULL;
    let render = |ui: &mut egui::Ui| {
        super::show_table(ui, &table, &search, super::super::i18n::Locale::English);
    };
    let first = context.run_ui(window_input(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            scroll_id = ui.make_persistent_id(egui::IdSalt::new("table_horizontal"));
            let mut state = egui::scroll_area::State::default();
            state.offset.x = 250.0;
            state.store(ui.ctx(), scroll_id);
            render(ui);
        });
    });
    let mut state = egui::scroll_area::State::load(&context, scroll_id).unwrap_or_default();
    state.offset.x = 300.0;
    state.store(&context, scroll_id);
    let second = context.run_ui(window_input(), |ui| {
        egui::CentralPanel::default().show(ui, render);
    });
    let header_delta = text_origin(&second, "Value")
        .zip(text_origin(&first, "Value"))
        .map(|(second, first)| second.x - first.x);
    let row_delta = text_origin(&second, "42")
        .zip(text_origin(&first, "42"))
        .map(|(second, first)| second.x - first.x);
    first.drop_without_applying_deltas();
    second.drop_without_applying_deltas();
    let header_delta = header_delta.expect("Second column header must remain visible");
    let row_delta = row_delta.expect("Second column value must remain visible");
    assert!(
        (header_delta + 50.0).abs() < 1.0,
        "Header moved by {header_delta}"
    );
    assert!(
        (header_delta - row_delta).abs() < 1.0,
        "Header/body deltas: {header_delta}/{row_delta}"
    );
}

#[test]
fn rendering_schema_union_types_do_not_expand_virtualized_rows() {
    let root = struct_view_core::parser::parse_json(
        r#"{"type":["string","number","boolean","object","array","null"]}"#,
    )
    .unwrap();
    let diagram = super::super::visualization::build_schema_diagram(&root).unwrap();
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 240.0),
    ));
    let output = context.run_ui(input, |ui| {
        super::show_schema(
            ui,
            &diagram,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let lines = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text.contains(" | "))
        .map(|shape| shape.galley.rows.len());
    output.drop_without_applying_deltas();
    assert_eq!(lines, Some(1));
}

#[test]
fn rendering_graph_labels_fit_inside_their_cards() {
    let root = struct_view_core::parser::parse_json(
        &serde_json::json!({"id":"entity","name":"W".repeat(120)}).to_string(),
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let output = context.run_ui(window_input(), |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let measured = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text.starts_with("WWW"))
        .map(|shape| (shape.galley.rect.width(), shape.galley.rows.len()));
    output.drop_without_applying_deltas();
    let (width, lines) = measured.expect("Graph label must be visible");
    assert!(width <= super::GRAPH_NODE_SIZE.x - 16.0 + 1.0, "{width}");
    assert_eq!(lines, 1);
}

#[test]
fn rendering_graph_labels_use_the_light_theme_palette() {
    let root =
        struct_view_core::parser::parse_json(r#"{"id":"entity","name":"Visible entity"}"#).unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let visuals = egui::Visuals::light();
    context.set_visuals(visuals.clone());
    let output = context.run_ui(window_input(), |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let label_color = text_shapes(&output.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == "Visible entity")
        .map(|shape| shape.fallback_color);
    output.drop_without_applying_deltas();
    assert_eq!(
        label_color,
        Some(SyntaxColors::new(&visuals).key),
        "Graph labels should remain readable in the light theme"
    );
}

#[test]
fn graph_relationship_types_use_stable_distinct_colors() {
    for visuals in [egui::Visuals::dark(), egui::Visuals::light()] {
        let colors = SyntaxColors::new(&visuals);
        let depends_on = super::graph::graph_edge_color("depends_on", colors);
        let parent_id = super::graph::graph_edge_color("parent_id", colors);

        assert_eq!(
            depends_on,
            super::graph::graph_edge_color("depends_on", colors)
        );
        assert_ne!(depends_on, parent_id);
    }
}

#[test]
fn graph_reads_undirected_json_adjacency_and_deduplicates_symmetric_links() {
    let root = struct_view_core::parser::parse_json(
        r#"{
            "graph": {"name": "Дружеские связи", "type": "undirected"},
            "adjacency": {
                "Анна": ["Борис", "Вера"],
                "Борис": ["Анна", "Глеб"],
                "Вера": ["Анна", "Глеб"],
                "Глеб": ["Борис", "Вера"]
            }
        }"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let links = graph
        .edges
        .iter()
        .map(|edge| {
            let source = graph.nodes[edge.source].id.clone();
            let target = graph.nodes[edge.target].id.clone();
            if source <= target {
                (source, target)
            } else {
                (target, source)
            }
        })
        .collect::<std::collections::BTreeSet<_>>();

    assert!(!graph.directed);
    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(
        links,
        std::collections::BTreeSet::from([
            ("Анна".to_string(), "Борис".to_string()),
            ("Анна".to_string(), "Вера".to_string()),
            ("Борис".to_string(), "Глеб".to_string()),
            ("Вера".to_string(), "Глеб".to_string()),
        ])
    );
}

#[test]
fn multipartite_graph_renders_each_partition_in_a_separate_column() {
    let root = struct_view_core::parser::parse_data(
        r#"
            [graph]
            name = "Employees, projects, and regions"
            type = "multipartite"
            directed = false

            [partitions]
            employees = ["anna", "boris"]
            projects = ["shop", "analytics"]
            regions = ["europe"]

            [labels]
            anna = "Anna"
            boris = "Boris"
            shop = "Shop"
            analytics = "Analytics"
            europe = "Europe"

            [relations]
            pairs = [
                ["anna", "shop"],
                ["boris", "analytics"],
                ["shop", "europe"],
            ]
        "#,
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1400.0, 400.0),
    ));
    let output = context.run_ui(input, |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let text_center_x = |text: &str| {
        text_shapes(&output.shapes)
            .into_iter()
            .find(|shape| shape.galley.job.text == text)
            .map(|shape| shape.pos.x + shape.galley.rect.width() / 2.0)
            .unwrap_or_else(|| panic!("Expected graph text {text:?}"))
    };
    let anna_x = text_center_x("Anna");
    let shop_x = text_center_x("Shop");
    let europe_x = text_center_x("Europe");
    let visible_partitions = ["employees", "projects", "regions"]
        .into_iter()
        .all(|partition| {
            text_shapes(&output.shapes)
                .iter()
                .any(|shape| shape.galley.job.text == partition)
        });
    output.drop_without_applying_deltas();

    assert!(shop_x > anna_x + super::GRAPH_STEP.x / 2.0);
    assert!(europe_x > shop_x + super::GRAPH_STEP.x / 2.0);
    assert!(visible_partitions);
}

#[test]
fn hovering_graph_node_highlights_neighbors_and_fades_unrelated_items() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"source","name":"Source","depends_on":"target"},{"id":"target","name":"Target"},{"id":"unrelated","name":"Unrelated","parent_id":"other"},{"id":"other","name":"Other"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let search = struct_view_core::search::SearchState::default();
    let locale = super::super::i18n::Locale::English;
    let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 450.0));
    let mut input = window_input();
    input.screen_rect = Some(screen_rect);
    let initial = context.run_ui(input, |ui| {
        render_graph(ui, &graph, &search, locale);
    });
    let source_position = text_shapes(&initial.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == "Source")
        .map(|shape| shape.pos + egui::vec2(1.0, 1.0))
        .expect("Source label must be visible");
    initial.drop_without_applying_deltas();

    let mut input = window_input();
    input.screen_rect = Some(screen_rect);
    input
        .events
        .push(egui::Event::PointerMoved(source_position));
    let output = context.run_ui(input, |ui| {
        render_graph(ui, &graph, &search, locale);
    });
    let colors = SyntaxColors::new(&egui::Visuals::dark());
    assert_eq!(text_color(&output, "Source"), Some(colors.key));
    assert_eq!(text_color(&output, "Target"), Some(colors.key));
    assert_eq!(
        text_color(&output, "Unrelated"),
        Some(colors.key.gamma_multiply(super::graph::GRAPH_DIM_FACTOR))
    );
    assert_eq!(
        text_color(&output, "Other"),
        Some(colors.key.gamma_multiply(super::graph::GRAPH_DIM_FACTOR))
    );
    assert_eq!(
        text_color(&output, "depends_on"),
        Some(super::graph::graph_edge_color("depends_on", colors))
    );
    assert_eq!(
        text_color(&output, "parent_id"),
        Some(
            super::graph::graph_edge_color("parent_id", colors)
                .gamma_multiply(super::graph::GRAPH_DIM_FACTOR)
        )
    );
    output.drop_without_applying_deltas();
}

#[test]
fn clicking_graph_node_keeps_selection_until_clicked_again() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"source","name":"Source","depends_on":"target"},{"id":"target","name":"Target"},{"id":"unrelated","name":"Unrelated","parent_id":"other"},{"id":"other","name":"Other"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let search = struct_view_core::search::SearchState::default();
    let locale = super::super::i18n::Locale::English;
    let render = |input: egui::RawInput| {
        context.run_ui(input, |ui| {
            render_graph(ui, &graph, &search, locale);
        })
    };

    let initial = render(graph_input(None, None));
    let source_position = text_shapes(&initial.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text == "Source")
        .map(|shape| shape.pos + egui::vec2(1.0, 1.0))
        .expect("Source label must be visible");
    initial.drop_without_applying_deltas();

    render(graph_input(Some(source_position), Some(true))).drop_without_applying_deltas();
    render(graph_input(Some(source_position), Some(false))).drop_without_applying_deltas();

    let colors = SyntaxColors::new(&egui::Visuals::dark());
    let outside = egui::pos2(900.0, 420.0);
    let selected = render(graph_input(Some(outside), None));
    assert_eq!(text_color(&selected, "Target"), Some(colors.key));
    assert_eq!(
        text_color(&selected, "Other"),
        Some(colors.key.gamma_multiply(super::graph::GRAPH_DIM_FACTOR))
    );
    selected.drop_without_applying_deltas();

    render(graph_input(Some(source_position), Some(true))).drop_without_applying_deltas();
    render(graph_input(Some(source_position), Some(false))).drop_without_applying_deltas();

    let cleared = render(graph_input(Some(outside), None));
    assert_eq!(text_color(&cleared, "Other"), Some(colors.key));
    assert_eq!(
        text_color(&cleared, "parent_id"),
        Some(super::graph::graph_edge_color("parent_id", colors))
    );
    cleared.drop_without_applying_deltas();
}

#[test]
fn rendering_graph_keeps_relationship_labels_readable_between_nodes() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 240.0),
    ));
    let output = context.run_ui(input, |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let relationship_label = text_shapes(&output.shapes)
        .into_iter()
        .any(|shape| shape.galley.job.text == "depends_on");
    output.drop_without_applying_deltas();
    assert!(
        relationship_label,
        "Relationship label should not be truncated"
    );
}

#[test]
fn graph_routes_edges_around_intervening_cards() {
    let positions = (0..9)
        .map(|index| {
            egui::Pos2::new(
                128.0 + (index % 3) as f32 * 340.0,
                59.0 + (index / 3) as f32 * 150.0,
            )
        })
        .collect::<Vec<_>>();
    let routing_grid = super::graph::GraphRoutingGrid::new(&positions);
    let route = routing_grid.route_edge(0, 8);

    assert!(
        route.len() > 2,
        "The diagonal route should detour around a card"
    );
    assert!(
        route
            .windows(2)
            .all(|segment| { segment[0].x == segment[1].x || segment[0].y == segment[1].y })
    );
    for (index, position) in positions.iter().enumerate() {
        if index == 0 || index == 8 {
            continue;
        }
        let obstacle = egui::Rect::from_center_size(*position, super::GRAPH_NODE_SIZE)
            .expand(super::graph::GRAPH_ROUTE_CLEARANCE);
        assert!(
            route.windows(2).all(|segment| {
                !super::graph::segment_crosses_rect_interior(segment[0], segment[1], obstacle)
            }),
            "Route crosses card {index}: {route:?}"
        );
    }
}

#[test]
fn parallel_graph_edges_use_distinct_card_ports() {
    let positions = [egui::pos2(128.0, 59.0), egui::pos2(468.0, 59.0)];
    let endpoints = [(0, 1), (0, 1), (0, 1)];
    let ports = super::graph::graph_edge_ports(&positions, &endpoints);
    let routing_grid = super::graph::GraphRoutingGrid::new(&positions);
    let routes = ports
        .iter()
        .map(|ports| routing_grid.route_edge_with_ports(0, 1, *ports, &[]))
        .collect::<Vec<_>>();

    assert_eq!(routes.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 2]);
    assert!(
        routes
            .windows(2)
            .all(|routes| routes[0][0].y != routes[1][0].y)
    );
    assert!(
        routes
            .windows(2)
            .all(|routes| routes[0][1].y != routes[1][1].y)
    );
}

#[test]
fn crossing_graph_edges_are_routed_onto_separate_tracks() {
    let positions = [
        egui::pos2(128.0, 59.0),
        egui::pos2(468.0, 59.0),
        egui::pos2(128.0, 209.0),
        egui::pos2(468.0, 209.0),
    ];
    let endpoints = [(0, 3), (1, 2)];
    let ports = super::graph::graph_edge_ports(&positions, &endpoints);
    let routing_grid = super::graph::GraphRoutingGrid::new(&positions);
    let first = routing_grid.route_edge_with_ports(0, 3, ports[0], &[]);
    let second = routing_grid.route_edge_with_ports(1, 2, ports[1], std::slice::from_ref(&first));

    assert_eq!(first.len(), 2);
    assert!(
        second.len() > 2,
        "The crossing edge should take a detour: {second:?}"
    );
    assert!(
        second.windows(2).all(|second_segment| {
            first.windows(2).all(|first_segment| {
                !super::graph::segments_within_clearance(
                    second_segment[0],
                    second_segment[1],
                    first_segment[0],
                    first_segment[1],
                    super::graph::GRAPH_EDGE_CLEARANCE,
                )
            })
        }),
        "Routes overlap: first={first:?}, second={second:?}"
    );
}

#[test]
fn rendering_crossing_graph_edge_labels_does_not_overlap() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"tl","name":"Top left","depends_on":"br"},{"id":"tr","name":"Top right","parent_id":"bl"},{"id":"bl","name":"Bottom left"},{"id":"br","name":"Bottom right"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 450.0),
    ));
    let output = context.run_ui(input, |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let bounds = ["depends_on", "parent_id"].map(|label| {
        text_shapes(&output.shapes)
            .into_iter()
            .find(|shape| shape.galley.job.text == label)
            .map(|shape| shape.visual_bounding_rect())
            .unwrap_or_else(|| panic!("Relationship label must be visible: {label}"))
    });
    output.drop_without_applying_deltas();
    assert!(
        !bounds[0].intersects(bounds[1]),
        "Crossing relationship labels overlap: {bounds:?}"
    );
}

#[test]
fn rendering_parallel_graph_edge_labels_does_not_overlap() {
    let root = struct_view_core::parser::parse_json(
        r#"[{"id":"source","name":"Source","depends_on":"target","parent_id":"target","ref":"target"},{"id":"target","name":"Target"}]"#,
    )
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 300.0),
    ));
    let output = context.run_ui(input, |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let bounds = ["depends_on", "parent_id", "ref"].map(|label| {
        text_shapes(&output.shapes)
            .into_iter()
            .find(|shape| shape.galley.job.text == label)
            .map(|shape| shape.visual_bounding_rect())
            .unwrap_or_else(|| panic!("Relationship label must be visible: {label}"))
    });
    output.drop_without_applying_deltas();

    for (index, first) in bounds.iter().enumerate() {
        for second in &bounds[index + 1..] {
            assert!(
                !first.intersects(*second),
                "Relationship labels overlap: {first:?} and {second:?}"
            );
        }
    }
}

#[test]
fn hovering_truncated_graph_edge_label_shows_its_full_text() {
    let full_label = "very_long_relationship_name_that_needs_tooltip";
    let root = struct_view_core::parser::parse_json(&format!(
        r#"[{{"id":"source","{full_label}":"target"}},{{"id":"target"}}]"#
    ))
    .unwrap();
    let graph = super::super::visualization::build_relationship_graph(&root);
    let context = egui::Context::default();
    let mut input = window_input();
    input.time = Some(0.0);
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000.0, 300.0),
    ));
    let initial = context.run_ui(input, |ui| {
        render_graph(
            ui,
            &graph,
            &struct_view_core::search::SearchState::default(),
            super::super::i18n::Locale::English,
        );
    });
    let label_shape = text_shapes(&initial.shapes)
        .into_iter()
        .find(|shape| shape.galley.job.text.contains('…'))
        .expect("The relationship label should be shortened");
    let pointer = label_shape.pos + egui::vec2(1.0, 1.0);
    assert_ne!(label_shape.galley.job.text, full_label);
    initial.drop_without_applying_deltas();

    let mut tooltip_visible = false;
    for frame in 1..=10 {
        let mut input = window_input();
        input.time = Some(f64::from(frame) * 0.1);
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 300.0),
        ));
        if frame == 1 {
            input.events.push(egui::Event::PointerMoved(pointer));
        }
        let output = context.run_ui(input, |ui| {
            render_graph(
                ui,
                &graph,
                &struct_view_core::search::SearchState::default(),
                super::super::i18n::Locale::English,
            );
        });
        tooltip_visible = text_shapes(&output.shapes)
            .iter()
            .any(|shape| shape.galley.job.text == full_label);
        output.drop_without_applying_deltas();
        if tooltip_visible {
            break;
        }
    }
    assert!(
        tooltip_visible,
        "Hovering the shortened label should reveal its complete text"
    );
}

#[test]
fn graph_arrows_intersect_card_borders_for_diagonal_and_straight_edges() {
    for direction in [
        egui::vec2(340.0, 150.0).normalized(),
        egui::vec2(1.0, 0.0),
        egui::vec2(0.0, -1.0),
    ] {
        let offset = super::graph::box_border_offset(direction);
        let intersection = direction * offset;
        assert!(
            intersection.x.abs() <= super::GRAPH_NODE_SIZE.x / 2.0 + 0.001,
            "{intersection:?}"
        );
        assert!(
            intersection.y.abs() <= super::GRAPH_NODE_SIZE.y / 2.0 + 0.001,
            "{intersection:?}"
        );
        assert!(
            (intersection.x.abs() - super::GRAPH_NODE_SIZE.x / 2.0).abs() < 0.001
                || (intersection.y.abs() - super::GRAPH_NODE_SIZE.y / 2.0).abs() < 0.001,
            "{intersection:?}"
        );
    }
}
