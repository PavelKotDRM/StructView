use std::fmt::Write;
use std::io;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum GraphExportFormat {
    Svg,
    Png,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum GraphExportStyle {
    LightTransparent,
    DarkOpaque,
}

impl GraphExportStyle {
    pub(in crate::app) fn text_key(self) -> TextKey {
        match self {
            Self::LightTransparent => TextKey::GraphExportLight,
            Self::DarkOpaque => TextKey::GraphExportDark,
        }
    }
}

impl GraphExportFormat {
    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            Self::Svg => "SVG",
            Self::Png => "PNG",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }
}

pub(in crate::app) fn export_graph_image(
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    format: GraphExportFormat,
    style: GraphExportStyle,
) -> io::Result<bool> {
    let extension = format.extension();
    let Some(mut path) = rfd::FileDialog::new()
        .add_filter(format.label(), &[extension])
        .set_file_name(format!("graph.{extension}"))
        .save_file()
    else {
        return Ok(false);
    };
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case(extension))
    {
        path.set_extension(extension);
    }
    let visuals = match style {
        GraphExportStyle::LightTransparent => egui::Visuals::light(),
        GraphExportStyle::DarkOpaque => egui::Visuals::dark(),
    };
    let svg = graph_svg(graph, routing, &visuals);
    let content = match format {
        GraphExportFormat::Svg => svg.into_bytes(),
        GraphExportFormat::Png => graph_png(&svg)?,
    };
    struct_view_core::files::write_bytes_atomic(&path, &content)?;
    Ok(true)
}

fn xml_text(value: &str) -> String {
    value
        .chars()
        .filter(|&ch| {
            matches!(ch, '\t' | '\n' | '\r') || ch >= ' ' && ch != '\u{fffe}' && ch != '\u{ffff}'
        })
        .fold(String::new(), |mut output, ch| {
            output.push_str(match ch {
                '&' => "&amp;",
                '<' => "&lt;",
                '>' => "&gt;",
                '"' => "&quot;",
                '\'' => "&apos;",
                _ => {
                    output.push(ch);
                    return output;
                }
            });
            output
        })
}

fn color(value: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", value.r(), value.g(), value.b())
}

fn opaque_color(value: Color32, background: Color32) -> String {
    let [red, green, blue, alpha] = value.to_srgba_unmultiplied();
    let opacity = f32::from(alpha) / 255.0;
    let channel = |foreground: u8, behind: u8| {
        (f32::from(foreground) * opacity + f32::from(behind) * (1.0 - opacity)).round() as u8
    };
    color(Color32::from_rgb(
        channel(red, background.r()),
        channel(green, background.g()),
        channel(blue, background.b()),
    ))
}

fn text(
    svg: &mut String,
    position: Pos2,
    value: &str,
    size: f32,
    fill: Color32,
    anchor: &str,
    family: &str,
) {
    writeln!(svg,
        r#"<text x="{}" y="{}" font-size="{size}" fill="{}" text-anchor="{anchor}" font-family="{family}">{}</text>"#,
        position.x, position.y + size * 0.35, color(fill), xml_text(value),
    ).expect("writing SVG to a string cannot fail");
}

fn shortened_label(
    painter: &egui::Painter,
    value: &str,
    max_chars: usize,
    size: f32,
    family: egui::FontFamily,
    fill: Color32,
) -> String {
    shorten_to_width(
        painter,
        value,
        max_chars,
        &FontId::new(size, family),
        GRAPH_NODE_SIZE.x - 16.0,
        fill,
    )
}

fn graph_svg(
    graph: &RelationshipGraph,
    routing: &GraphRoutingLayout,
    visuals: &egui::Visuals,
) -> String {
    let colors = SyntaxColors::new(visuals);
    let context = egui::Context::default();
    let mut svg = String::new();
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        let painter = ui.painter();
        let canvas = egui::Rect::from_min_size(Pos2::ZERO, routing.content_size);
        let node_rects = routing.node_positions.iter()
            .map(|&position| egui::Rect::from_center_size(position, GRAPH_NODE_SIZE)).collect::<Vec<_>>();
        let mut occupied = Vec::new();
        let mut content_size = routing.content_size;
        let mut labels = graph.edges.iter().enumerate().map(|(index, edge)| {
            if edge.label.trim().is_empty() { return None; }
            let full_text = single_line_text(&edge.label).into_owned();
            let route = &routing.edge_paths[index];
            let local = routing.edge_labels[index].as_ref().and_then(|label| {
                place_edge_label(&full_text, label.position, label.alignment, canvas, &node_rects, &occupied, &routing.edge_paths)
                    .map(|background| graph_edge_label_layout(full_text.clone(), label.position, label.alignment, background, route))
            });
            let label = local.unwrap_or_else(|| graph_edge_label_callout_text(full_text, route, canvas, &occupied));
            occupied.push(label.background);
            content_size.x = content_size.x.max(label.background.right() + 24.0);
            content_size.y = content_size.y.max(label.background.bottom() + 24.0);
            Some(label)
        }).collect::<Vec<_>>();
        resolve_graph_label_leaders(&mut labels, &node_rects, &routing.edge_paths);
        writeln!(svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            content_size.x.ceil(), content_size.y.ceil(),
            content_size.x.ceil(), content_size.y.ceil(),
        ).unwrap();
        if visuals.dark_mode {
            writeln!(svg, r#"<rect width="100%" height="100%" fill="{}"/>"#, color(visuals.panel_fill)).unwrap();
        }
        if let Some(names) = &routing.partition_labels {
            for (index, name) in names.iter().enumerate() {
                text(&mut svg, Pos2::new(24.0 + index as f32 * GRAPH_STEP.x + GRAPH_NODE_SIZE.x / 2.0, 10.0),
                    name, 13.0, colors.key, "middle", "sans-serif");
            }
        }
        for (index, edge) in graph.edges.iter().enumerate() {
            let fill = graph_edge_color(&edge.label, colors);
            let points = &routing.edge_paths[index];
            let coordinates = points.iter().map(|point| format!("{},{}", point.x, point.y)).collect::<Vec<_>>().join(" ");
            writeln!(svg, r#"<polyline points="{coordinates}" fill="none" stroke="{}" stroke-width="1.5"/>"#, color(fill)).unwrap();
            if graph.directed {
                let tip = *points.last().expect("graph route must have an endpoint");
                let direction = (tip - points[points.len() - 2]).normalized();
                for angle in [2.55, -2.55] {
                    let wing = tip + Vec2::angled(direction.angle() + angle) * 9.0;
                    writeln!(svg, r#"<path d="M {} {} L {} {}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
                        tip.x, tip.y, wing.x, wing.y, color(fill)).unwrap();
                }
            }
        }
        for (index, label) in labels.iter().enumerate() {
            let Some(label) = label else { continue };
            let fill = graph_edge_color(&graph.edges[index].label, colors);
            if let Some([start, end]) = label.leader {
                writeln!(svg, r#"<path d="M {} {} L {} {}" fill="none" stroke="{}" stroke-width="1" stroke-dasharray="3 3"/>"#,
                    start.x, start.y, end.x, end.y, color(fill)).unwrap();
            }
        }
        // Draw labels after all routes and leaders so subsequent lines cannot obscure text.
        for (index, label) in labels.iter().enumerate() {
            let Some(label) = label else { continue };
            let fill = graph_edge_color(&graph.edges[index].label, colors);
            let rect = label.background;
            writeln!(svg, r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"#,
                rect.left(), rect.top(), rect.width(), rect.height(), color(visuals.panel_fill)).unwrap();
            let anchor = if label.alignment == Align2::LEFT_CENTER { "start" }
                else if label.alignment == Align2::RIGHT_CENTER { "end" } else { "middle" };
            text(&mut svg, label.position, &label.text, 12.0, fill, anchor, "sans-serif");
            if let Some((number, route_anchor)) = label.reference {
                for center in [route_anchor, Pos2::new(label.background.right() - 11.0, label.background.center().y)] {
                    writeln!(svg, r#"<circle cx="{}" cy="{}" r="8" fill="{}" stroke="{}"/>"#,
                        center.x, center.y, color(visuals.panel_fill), color(fill)).unwrap();
                    text(&mut svg, center, &number.to_string(), 10.0, fill, "middle", "sans-serif");
                }
            }
        }
        for (node, &center) in graph.nodes.iter().zip(&routing.node_positions) {
            let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE);
            writeln!(svg, r#"<g><title>{}</title><rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                xml_text(&format!("{}\n{}\n{}", node.label, node.id, node.path)),
                rect.left(), rect.top(), rect.width(), rect.height(),
                opaque_color(visuals.faint_bg_color, visuals.panel_fill),
                opaque_color(visuals.widgets.noninteractive.bg_stroke.color, visuals.panel_fill),
                visuals.widgets.noninteractive.bg_stroke.width).unwrap();
            let label = shortened_label(painter, &node.label, 24, 15.0, egui::FontFamily::Proportional, colors.key);
            text(&mut svg, center - Vec2::new(0.0, 9.0), &label, 15.0, colors.key, "middle", "sans-serif");
            let id = shortened_label(painter, &node.id, 26, 11.0, egui::FontFamily::Monospace, visuals.weak_text_color());
            text(&mut svg, center + Vec2::new(0.0, 13.0), &id, 11.0, visuals.weak_text_color(), "middle", "monospace");
            svg.push_str("</g>\n");
        }
        svg.push_str("</svg>\n");
    });
    output.drop_without_applying_deltas();
    svg
}

const GRAPH_PNG_MAX_PIXELS: u64 = 16_000_000;

fn graph_png_dimensions(width: u32, height: u32) -> (u32, u32) {
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= GRAPH_PNG_MAX_PIXELS {
        return (width, height);
    }
    let scale = (GRAPH_PNG_MAX_PIXELS as f64 / pixels as f64)
        .sqrt()
        .min(GRAPH_PNG_MAX_PIXELS as f64 / f64::from(width.max(height)));
    (
        (f64::from(width) * scale).floor().max(1.0) as u32,
        (f64::from(height) * scale).floor().max(1.0) as u32,
    )
}

fn graph_png(svg: &str) -> io::Result<Vec<u8>> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    for font in egui::FontDefinitions::default().font_data.values() {
        options.fontdb_mut().load_font_data(font.font.to_vec());
    }
    options.fontdb_mut().set_sans_serif_family("Ubuntu");
    options.fontdb_mut().set_monospace_family("Hack");
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(io::Error::other)?;
    let size = tree.size().to_int_size();
    let (width, height) = graph_png_dimensions(size.width(), size.height());
    let scale = if width == size.width() && height == size.height() {
        1.0
    } else {
        (width as f32 / tree.size().width()).min(height as f32 / tree.size().height())
    };
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| io::Error::other("Cannot allocate PNG image"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_dense_graph_contains_all_relationships_and_callouts() {
        let root = struct_view_core::parser::parse_json(
            r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#,
        )
        .unwrap();
        let mut graph = build_relationship_graph(&root);
        let edge = graph.edges[0].clone();
        graph.edges = (0..24)
            .map(|index| {
                let mut edge = edge.clone();
                edge.label = format!("relationship_{index:02}");
                edge
            })
            .collect();
        let routing = build_graph_routing_layout(&graph);
        let svg = graph_svg(&graph, &routing, &egui::Visuals::light());
        for edge in &graph.edges {
            assert!(svg.contains(&format!(">{}</text>", edge.label)));
        }
        let last_route = svg.rfind("<polyline").unwrap();
        let last_leader = svg.rfind(r#"stroke-dasharray="3 3""#);
        let first_label = svg.find(">relationship_").unwrap();
        assert!(last_route < first_label);
        if let Some(last_leader) = last_leader {
            assert!(last_leader < first_label);
        }
        assert!(
            svg.contains("<circle"),
            "Blocked leaders must use numbered references"
        );
        let png = graph_png(&svg).unwrap();
        assert!(
            u32::from_be_bytes(png[16..20].try_into().unwrap())
                >= routing.content_size.x.ceil() as u32
        );
        assert!(
            u32::from_be_bytes(png[20..24].try_into().unwrap())
                >= routing.content_size.y.ceil() as u32
        );
    }

    #[test]
    fn graph_export_menu_requests_the_selected_image_format() {
        let root = struct_view_core::parser::parse_json(r#"[{"id":"a"}]"#).unwrap();
        let graph = build_relationship_graph(&root);
        let routing = build_graph_routing_layout(&graph);
        for (format, style) in [
            (GraphExportFormat::Svg, GraphExportStyle::LightTransparent),
            (GraphExportFormat::Svg, GraphExportStyle::DarkOpaque),
            (GraphExportFormat::Png, GraphExportStyle::LightTransparent),
            (GraphExportFormat::Png, GraphExportStyle::DarkOpaque),
        ] {
            let context = egui::Context::default();
            let render = |events| {
                let mut requested = None;
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        requested = show_graph(
                            ui,
                            &graph,
                            &routing,
                            &SearchState::default(),
                            Locale::English,
                        );
                    },
                );
                (output, requested)
            };
            let position = |output: &egui::FullOutput, label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == label => {
                            Some(text.visual_bounding_rect().center())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Export control must be visible: {label}"))
            };
            let click = |pos| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            let (output, _) = render(Vec::new());
            let button = position(&output, "Export graph");
            output.drop_without_applying_deltas();
            let (output, _) = render(click(button));
            output.drop_without_applying_deltas();
            let (output, _) = render(Vec::new());
            let button = position(&output, format.label());
            output.drop_without_applying_deltas();
            let (output, _) = render(click(button));
            output.drop_without_applying_deltas();
            let (output, _) = render(Vec::new());
            let button = position(&output, Locale::English.text(style.text_key()));
            output.drop_without_applying_deltas();
            let (output, requested) = render(click(button));
            assert_eq!(requested, Some((format, style)));
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn svg_and_png_export_full_graph_and_escape_labels() {
        let root = struct_view_core::parser::parse_json(
            r#"[{"id":"a","name":"A < B & \"C\"","depends_on":"b"},{"id":"b","name":"Цель"}]"#,
        )
        .unwrap();
        let graph = build_relationship_graph(&root);
        let routing = build_graph_routing_layout(&graph);
        let svg = graph_svg(&graph, &routing, &egui::Visuals::light());
        assert!(svg.contains("A &lt; B &amp; &quot;C&quot;"));
        assert!(svg.contains("Цель"));
        assert_eq!(svg.matches("<polyline").count(), graph.edges.len());
        assert_eq!(svg.matches("<g>").count(), graph.nodes.len());
        assert!(svg.contains("depends_on"));
        let png = graph_png(&svg).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            routing.content_size.x.ceil() as u32
        );
        assert_eq!(
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
            routing.content_size.y.ceil() as u32
        );
    }

    #[test]
    fn png_export_downscales_large_images_without_cropping() {
        assert!(graph_png("<invalid>").is_err());
        let png = graph_png(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="5000" height="5000">
                <rect x="4900" y="4900" width="100" height="100" fill="#ff0000"/>
            </svg>"##,
        )
        .unwrap();
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (4000, 4000));
        assert_eq!(pixmap.pixel(0, 0).unwrap().alpha(), 0);
        let corner = pixmap.pixel(3999, 3999).unwrap();
        assert_eq!(corner.alpha(), 255);
        assert_eq!(corner.red(), 255);
        assert_eq!(xml_text("a\u{0}b\u{fffe}"), "ab");
    }

    #[test]
    fn png_dimensions_preserve_small_images_and_bound_large_allocations() {
        assert_eq!(graph_png_dimensions(800, 600), (800, 600));
        assert_eq!(graph_png_dimensions(4000, 4000), (4000, 4000));
        assert_eq!(graph_png_dimensions(8000, 4000), (5656, 2828));
        for (width, height) in [
            (5000, 5000),
            (100_000, 800),
            (u32::MAX, 1),
            (1, u32::MAX),
            (u32::MAX, u32::MAX),
        ] {
            let (actual_width, actual_height) = graph_png_dimensions(width, height);
            assert!(actual_width > 0 && actual_height > 0);
            assert!(u64::from(actual_width) * u64::from(actual_height) <= GRAPH_PNG_MAX_PIXELS);
            assert!(actual_width <= width && actual_height <= height);
        }
    }

    #[test]
    fn export_preserves_full_relationship_text_and_selected_background_alpha() {
        let full_label =
            "Очень длинное описание связи с подробностями & дополнительной информацией";
        let root = struct_view_core::parser::parse_json(
            r#"[{"id":"source","depends_on":"target"},{"id":"target"}]"#,
        )
        .unwrap();
        let mut graph = build_relationship_graph(&root);
        graph.edges[0].label = full_label.to_string();
        let routing = build_graph_routing_layout(&graph);
        assert_ne!(routing.edge_labels[0].as_ref().unwrap().text, full_label);
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            let svg = graph_svg(&graph, &routing, &visuals);
            assert!(svg.contains(&format!(">{}</text>", xml_text(full_label))));
            assert_eq!(
                svg.contains(r#"<rect width="100%" height="100%""#),
                visuals.dark_mode
            );
            let png = graph_png(&svg).unwrap();
            let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(
                pixmap.pixel(0, 0).unwrap().alpha(),
                if visuals.dark_mode { 255 } else { 0 }
            );
            assert!(pixmap.width() as f32 >= routing.content_size.x);
        }
    }

    #[test]
    fn exported_node_fills_preserve_light_and_dark_theme_colors() {
        let root = struct_view_core::parser::parse_json(r#"[{"id":"a","name":"Node"}]"#).unwrap();
        let graph = build_relationship_graph(&root);
        let routing = build_graph_routing_layout(&graph);
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            let svg = graph_svg(&graph, &routing, &visuals);
            let expected = opaque_color(visuals.faint_bg_color, visuals.panel_fill);
            assert!(svg.contains(&format!(r#"rx="6" fill="{expected}""#)));
            let png = graph_png(&svg).unwrap();
            let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            let center = routing.node_positions[0];
            let pixel = pixmap
                .pixel(center.x as u32, (center.y + 25.0) as u32)
                .unwrap();
            assert_eq!(
                color(Color32::from_rgb(pixel.red(), pixel.green(), pixel.blue())),
                expected,
            );
        }
    }
}
