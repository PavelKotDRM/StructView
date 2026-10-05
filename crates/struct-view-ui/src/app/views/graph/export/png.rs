use super::*;

pub(super) fn graph_png_dimensions(width: u32, height: u32) -> (u32, u32) {
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

pub(super) fn graph_png(svg: &str) -> io::Result<Vec<u8>> {
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
