use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::io;
use std::path::Path;

use struct_view_core::structure::Document;

use super::{kind_color, layout::Layout, node_label, shorten};

fn xml(value: &str) -> String {
    value
        .chars()
        .filter(|&c| {
            matches!(c, '\n' | '\t' | '\r') || c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}'
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
pub(super) fn svg(
    doc: &Document,
    layout: &Layout,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
) -> String {
    styled_svg(doc, layout, collapsed, limits, true)
}

pub(super) fn styled_svg(
    doc: &Document,
    layout: &Layout,
    collapsed: &HashSet<usize>,
    limits: &HashMap<usize, usize>,
    dark: bool,
) -> String {
    let width = layout.size.x.ceil().max(1.0);
    let height = layout.size.y.ceil().max(1.0);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    );
    if dark {
        svg.push_str(r##"<rect width="100%" height="100%" fill="#171d28"/>"##);
    }
    let edge_color = if dark { "#d2d7e1" } else { "#475569" };
    let node_fill = if dark { "#171d28" } else { "#ffffff" };
    let key_color = if dark { "#ffda73" } else { "#624200" };
    for &(_, _, points) in &layout.edges {
        let points = points
            .iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(
            svg,
            r#"<polyline points="{points}" fill="none" stroke="{edge_color}" stroke-width="1.5"/>"#
        );
    }
    for &(id, rect) in &layout.nodes {
        let node = &doc.nodes[id];
        let color = kind_color(node.kind);
        let color = if dark {
            color
        } else {
            color.gamma_multiply(0.45)
        };
        let color = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
        let title = xml(&format!("{}: {}\n{}", node.key, node.value, node.path));
        let key = xml(&shorten(&node.key, 24));
        let value = xml(&node_label(
            node,
            collapsed.contains(&id),
            limits.get(&id).copied().unwrap_or(100),
        ));
        let _ = write!(
            svg,
            r#"<g><title>{title}</title><rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{node_fill}" stroke="{color}" stroke-width="1.5"/><text x="{}" y="{}" text-anchor="middle" font-family="sans-serif" font-size="14" fill="{key_color}">{key}</text><text x="{}" y="{}" text-anchor="middle" font-family="sans-serif" font-size="12" fill="{color}">{value}</text></g>"#,
            rect.min.x,
            rect.min.y,
            rect.width(),
            rect.height(),
            rect.center().x,
            rect.min.y + 22.0,
            rect.center().x,
            rect.max.y - 12.0
        );
    }
    svg.push_str("</svg>");
    svg
}

pub(super) fn write(path: &Path, svg: &str, png: bool) -> io::Result<()> {
    let content = if png {
        render_png(svg)?
    } else {
        svg.as_bytes().to_vec()
    };
    struct_view_core::files::write_bytes_atomic(path, &content)
}

fn render_png(svg: &str) -> io::Result<Vec<u8>> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    for font in egui::FontDefinitions::default().font_data.values() {
        options.fontdb_mut().load_font_data(font.font.to_vec());
    }
    options.fontdb_mut().set_sans_serif_family("Ubuntu");
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(io::Error::other)?;
    let size = tree.size();
    let scale = (16_000_000.0 / (f64::from(size.width()) * f64::from(size.height())))
        .sqrt()
        .min(1.0) as f32;
    let width = (size.width() * scale).floor().max(1.0) as u32;
    let height = (size.height() * scale).floor().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| io::Error::other("Cannot allocate structure PNG"))?;
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
    use crate::app::views::structure::layout::{Direction, build};

    #[test]
    fn export_is_a_diagram_with_tree_edges_and_full_tooltips() {
        let doc =
            struct_view_core::structure::parse(r#"{"key<&":"full value & < > \" text"}"#, None)
                .unwrap();
        let layout = build(&doc, &HashSet::new(), &HashMap::new(), Direction::Vertical);
        let svg = svg(&doc, &layout, &HashSet::new(), &HashMap::new());
        assert_eq!(svg.matches("<polyline").count(), 1);
        assert_eq!(svg.matches("<g>").count(), 2);
        assert!(svg.contains("key&lt;&amp;"));
        assert!(svg.contains("full value &amp; &lt; &gt; &quot; text"));
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
        assert_eq!(tree.size().width(), layout.size.x.ceil());
        let png = render_png(&svg).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let light = styled_svg(&doc, &layout, &HashSet::new(), &HashMap::new(), false);
        assert!(!light.contains(r##"width="100%" height="100%" fill="#171d28""##));
        assert!(light.contains(r##"fill="#ffffff""##));
        assert!(resvg::usvg::Tree::from_str(&light, &resvg::usvg::Options::default()).is_ok());
        assert_eq!(&render_png(&light).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    }
}
