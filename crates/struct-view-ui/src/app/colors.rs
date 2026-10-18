use super::i18n::{Locale, TextKey};
use egui::{Color32, Sense, StrokeKind, Ui, Vec2};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{ScreenPickerState, show_eyedropper, start_screen_capture};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ColorFormat {
    #[default]
    Original,
    Hex,
    Rgb,
    Hsl,
    Hwb,
    Lab,
    Lch,
    Oklab,
    Oklch,
    ColorSrgb,
    Hsv,
    Name,
}

#[derive(Clone, Copy)]
pub(super) struct ColorFormatSelection {
    pub(super) format: ColorFormat,
    pub(super) changed: bool,
}

impl ColorFormat {
    fn label(self, locale: Locale) -> &'static str {
        match self {
            Self::Original => locale.text(TextKey::ColorFormatOriginal),
            Self::Hex => "HEX",
            Self::Rgb => "RGB(A)",
            Self::Hsl => "HSL(A)",
            Self::Hwb => "HWB",
            Self::Lab => "Lab",
            Self::Lch => "LCH",
            Self::Oklab => "OKLab",
            Self::Oklch => "OKLCH",
            Self::ColorSrgb => "CSS color(sRGB)",
            Self::Hsv => "HSV(A)",
            Self::Name => "CSS name",
        }
    }
}

pub(super) fn show_color_format_picker(
    ui: &mut Ui,
    locale: Locale,
    field_id: impl std::hash::Hash + std::fmt::Debug,
) -> ColorFormatSelection {
    let id = ui.make_persistent_id(("color-edit-output-format", field_id));
    let mut format = ui
        .ctx()
        .data(|data| data.get_temp::<ColorFormat>(id))
        .unwrap_or_default();
    let previous_format = format;
    ui.horizontal(|ui| {
        ui.label(locale.text(TextKey::ColorFormat));
        egui::ComboBox::from_id_salt(id)
            .selected_text(format.label(locale))
            .show_ui(ui, |ui| {
                for option in [
                    ColorFormat::Original,
                    ColorFormat::Hex,
                    ColorFormat::Rgb,
                    ColorFormat::Hsl,
                    ColorFormat::Hwb,
                    ColorFormat::Lab,
                    ColorFormat::Lch,
                    ColorFormat::Oklab,
                    ColorFormat::Oklch,
                    ColorFormat::ColorSrgb,
                    ColorFormat::Hsv,
                    ColorFormat::Name,
                ] {
                    ui.selectable_value(&mut format, option, option.label(locale));
                }
            });
    });
    ui.ctx().data_mut(|data| data.insert_temp(id, format));
    ColorFormatSelection {
        format,
        changed: format != previous_format,
    }
}

pub(super) fn color_edit_button_srgba(ui: &mut Ui, color: &mut Color32, locale: Locale) -> bool {
    let (rect, mut button_response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter().circle_filled(rect.center(), 6.0, *color);
        ui.painter().circle_stroke(
            rect.center(),
            6.0,
            egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color),
        );
    }
    let popup_id = button_response.id.with("color-picker");
    if ui.style().explanation_tooltips {
        button_response = button_response.on_hover_text(locale.text(TextKey::ChooseColor));
    }
    let picker_id = button_response.id.with("screen-color-picker");
    let mut changed = false;
    let mut start_eyedropper = false;
    egui::Popup::menu(&button_response)
        .id(popup_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().slider_width = 180.0;
            changed = compact_color_picker(ui, color, button_response.id);
            ui.separator();
            if ui
                .button(locale.text(TextKey::PickColorFromScreen))
                .clicked()
            {
                start_eyedropper = true;
            }
            if let Some(error) = ui
                .ctx()
                .data(|data| data.get_temp::<ScreenPickerState>(picker_id))
                .and_then(|state| state.error)
            {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });

    if start_eyedropper {
        let state = start_screen_capture(ui.ctx(), picker_id);
        ui.ctx().data_mut(|data| data.insert_temp(picker_id, state));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(50));
    }
    if let Some(selected) = show_eyedropper(ui.ctx(), picker_id) {
        ui.ctx()
            .data_mut(|data| data.remove::<ScreenPickerState>(picker_id));
        if let Some(selected) = selected {
            *color = selected;
            changed = true;
        }
    }
    changed
}

fn compact_color_picker(ui: &mut Ui, color: &mut Color32, id: egui::Id) -> bool {
    let mut hsva = ui
        .ctx()
        .data(|data| data.get_temp::<PickerHsvaState>(id.with("hsva")))
        .filter(|state| state.color == *color)
        .map_or_else(|| egui::ecolor::Hsva::from(*color), |state| state.hsva);
    let size = ui.spacing().slider_width;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let resolution = 96;
    let pixels = (0..resolution)
        .flat_map(|y| {
            (0..resolution).map(move |x| {
                Color32::from(egui::ecolor::Hsva {
                    h: hsva.h,
                    s: x as f32 / (resolution - 1) as f32,
                    v: 1.0 - y as f32 / (resolution - 1) as f32,
                    a: 1.0,
                })
            })
        })
        .collect();
    let image = egui::ColorImage::new([resolution, resolution], pixels);
    let texture = ui.ctx().load_texture_cached(
        "color-picker-saturation-value",
        id.with("saturation-value"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    if let Some(pointer) = response.interact_pointer_pos() {
        hsva.s = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        hsva.v = (1.0 - (pointer.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
    }
    let marker = egui::pos2(
        egui::lerp(rect.left()..=rect.right(), hsva.s),
        egui::lerp(rect.bottom()..=rect.top(), hsva.v),
    );
    let marker_color = Color32::from(hsva);
    let marker_radius = if response.hovered() || response.is_pointer_button_down_on() {
        7.0
    } else {
        2.5
    };
    let [red, green, blue, _] = marker_color.to_srgba_unmultiplied();
    ui.painter()
        .circle_filled(marker, marker_radius, marker_color);
    ui.painter().circle_stroke(
        marker,
        marker_radius,
        egui::Stroke::new(
            if marker_radius > 3.0 { 2.0 } else { 1.0 },
            if u16::from(red) + u16::from(green) + u16::from(blue) < 384 {
                Color32::WHITE
            } else {
                Color32::BLACK
            },
        ),
    );

    let mut changed = response.changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut hsva.h, 0.0..=1.0)
                .show_value(false)
                .text("H"),
        )
        .changed();
    let mut additive = hsva.a < 0.0;
    if !additive {
        changed |= ui
            .add(
                egui::Slider::new(&mut hsva.a, 0.0..=1.0)
                    .show_value(false)
                    .text("A"),
            )
            .changed();
    }
    let previous_additive = additive;
    ui.horizontal(|ui| {
        ui.label("Blending:");
        ui.radio_value(&mut additive, false, "Normal");
        ui.radio_value(&mut additive, true, "Additive");
    });
    changed |= additive != previous_additive;
    hsva.a = if additive {
        -hsva.a.abs()
    } else {
        hsva.a.abs()
    };

    let mut rgba = Color32::from(hsva).to_srgba_unmultiplied();
    let mut rgba_changed = false;
    ui.horizontal(|ui| {
        for (label, channel) in ["R ", "G ", "B "].into_iter().zip(rgba[..3].iter_mut()) {
            rgba_changed |= ui
                .add(egui::DragValue::new(channel).range(0..=255).prefix(label))
                .changed();
        }
        if !additive {
            rgba_changed |= ui
                .add(
                    egui::DragValue::new(&mut rgba[3])
                        .range(0..=255)
                        .prefix("A "),
                )
                .changed();
        }
    });
    changed |= rgba_changed;
    if changed {
        *color = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
    }
    if rgba_changed {
        hsva = egui::ecolor::Hsva::from(*color);
        if additive {
            hsva.a = -hsva.a.abs();
        }
    }
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            id.with("hsva"),
            PickerHsvaState {
                color: *color,
                hsva,
            },
        )
    });
    changed
}

#[derive(Clone, Copy)]
struct PickerHsvaState {
    color: Color32,
    hsva: egui::ecolor::Hsva,
}

#[cfg(not(windows))]
#[derive(Clone)]
struct CapturedScreen {
    width: usize,
    height: usize,
    pixels: std::sync::Arc<[u8]>,
    texture: egui::TextureHandle,
    prepared: bool,
}

#[cfg(not(windows))]
impl CapturedScreen {
    fn viewport_builder(&self, index: usize) -> egui::ViewportBuilder {
        egui::ViewportBuilder::default()
            .with_title("Screen color picker")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_fullscreen(true)
            .with_monitor(index)
    }
}

#[cfg(not(windows))]
type ScreenCaptureReceiver = std::sync::Arc<
    std::sync::Mutex<std::sync::mpsc::Receiver<Result<Vec<CapturedScreen>, String>>>,
>;

#[cfg(not(windows))]
#[derive(Clone, Default)]
struct ScreenPickerState {
    screens: Vec<CapturedScreen>,
    capture: Option<ScreenCaptureReceiver>,
    error: Option<String>,
}

#[cfg(not(windows))]
fn capture_screens(ctx: &egui::Context, id: egui::Id) -> Result<Vec<CapturedScreen>, String> {
    let monitors = match xcap::Monitor::all() {
        Ok(monitors) => monitors,
        Err(error) => return Err(error.to_string()),
    };
    let mut screens = Vec::with_capacity(monitors.len());
    for (index, monitor) in monitors.into_iter().enumerate() {
        let image = monitor.capture_image().map_err(|error| error.to_string())?;
        let width = image.width() as usize;
        let height = image.height() as usize;
        let pixels: std::sync::Arc<[u8]> = image.into_raw().into();
        let texture = ctx.load_texture(
            format!("screen-color-picker-{id:?}-{index}"),
            egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels),
            egui::TextureOptions::NEAREST,
        );
        screens.push(CapturedScreen {
            width,
            height,
            pixels,
            texture,
            prepared: false,
        });
    }
    if screens.is_empty() {
        return Err("No screens are available".to_string());
    }
    Ok(screens)
}

#[cfg(not(windows))]
fn start_screen_capture(ctx: &egui::Context, id: egui::Id) -> ScreenPickerState {
    let (sender, receiver) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    let capture = std::thread::Builder::new()
        .name("screen-color-picker-capture".to_string())
        .spawn(move || {
            let _ = sender.send(capture_screens(&ctx, id));
        });
    match capture {
        Ok(_) => ScreenPickerState {
            capture: Some(std::sync::Arc::new(std::sync::Mutex::new(receiver))),
            ..Default::default()
        },
        Err(error) => ScreenPickerState {
            error: Some(error.to_string()),
            ..Default::default()
        },
    }
}

#[cfg(not(windows))]
fn show_eyedropper(ctx: &egui::Context, id: egui::Id) -> Option<Option<Color32>> {
    let mut state = ctx.data(|data| data.get_temp::<ScreenPickerState>(id))?;
    if state.screens.is_empty()
        && state.error.is_none()
        && let Some(capture) = state.capture.clone()
    {
        let result = match capture.lock() {
            Ok(receiver) => receiver.try_recv(),
            Err(_) => {
                state.error = Some("Screen capture worker failed".to_string());
                state.capture = None;
                ctx.data_mut(|data| data.insert_temp(id, state));
                ctx.request_repaint();
                return None;
            }
        };
        match result {
            Ok(Ok(screens)) => {
                state.screens = screens;
                state.capture = None;
                ctx.data_mut(|data| data.insert_temp(id, state.clone()));
            }
            Ok(Err(error)) => {
                state.error = Some(error);
                state.capture = None;
                ctx.data_mut(|data| data.insert_temp(id, state));
                ctx.request_repaint();
                return None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.error = Some("Screen capture worker stopped unexpectedly".to_string());
                state.capture = None;
                ctx.data_mut(|data| data.insert_temp(id, state));
                ctx.request_repaint();
                return None;
            }
        }
    }
    if state.screens.is_empty() {
        return None;
    }
    let mut outcome = None;
    for (index, screen) in state.screens.iter_mut().enumerate() {
        let viewport_id = egui::ViewportId::from_hash_of((id, "screen-eyedropper", index));
        let builder = screen.viewport_builder(index);
        ctx.show_viewport_immediate(viewport_id, builder, |viewport_ui, _class| {
            let viewport_ctx = viewport_ui.ctx().clone();
            viewport_ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            let escape_pressed = viewport_ctx.input(|input| {
                input.key_pressed(egui::Key::Escape) || input.viewport().close_requested()
            });
            let mut selection = None;
            {
                let draw_image = |ui: &mut Ui| {
                    let rect = ui.available_rect_before_wrap();
                    let size = rect.size();
                    let response = ui.put(
                        rect,
                        egui::Image::new((screen.texture.id(), size))
                            .fit_to_exact_size(size)
                            .sense(Sense::click()),
                    );
                    if response.clicked()
                        && let Some(pointer) = response.interact_pointer_pos()
                    {
                        let x = ((pointer.x - response.rect.left()) / response.rect.width()
                            * screen.width as f32)
                            .floor() as usize;
                        let y = ((pointer.y - response.rect.top()) / response.rect.height()
                            * screen.height as f32)
                            .floor() as usize;
                        let offset =
                            (y.min(screen.height - 1) * screen.width + x.min(screen.width - 1)) * 4;
                        selection = Some(Color32::from_rgba_unmultiplied(
                            screen.pixels[offset],
                            screen.pixels[offset + 1],
                            screen.pixels[offset + 2],
                            screen.pixels[offset + 3],
                        ));
                    }
                };
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(viewport_ui, draw_image);
            }
            if escape_pressed || selection.is_some() {
                viewport_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                outcome = Some(selection);
            } else if screen.prepared {
                // Immediate viewport commands are applied after the frame is presented.
                viewport_ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            } else {
                screen.prepared = true;
                ctx.request_repaint();
            }
        });
        if outcome.is_some() {
            break;
        }
    }
    ctx.data_mut(|data| data.insert_temp(id, state));
    outcome
}

pub(super) fn parse_color(value: &str) -> Option<Color32> {
    let value = value.trim();
    let color = parse_css_color(value).or_else(|| {
        serde_json::from_str::<String>(value)
            .ok()
            .and_then(|unquoted| parse_css_color(&unquoted))
    })?;
    let [red, green, blue, alpha] = color.clamp().to_rgba8();
    Some(Color32::from_rgba_unmultiplied(red, green, blue, alpha))
}

fn parse_css_color(value: &str) -> Option<csscolorparser::Color> {
    if !value.starts_with('#') && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    csscolorparser::Color::from_html(value).ok()
}

pub(super) fn attribute_color(attributes: &[(String, String)], names: &[&str]) -> Option<Color32> {
    names.iter().find_map(|name| {
        attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| parse_color(value))
    })
}

pub(super) fn format_color_like(original: &str, color: Color32) -> String {
    let original = original.trim();
    let original =
        serde_json::from_str::<String>(original).unwrap_or_else(|_| original.to_string());
    let notation = original.trim().to_ascii_lowercase();
    let css_color = {
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        csscolorparser::Color::from_rgba8(red, green, blue, alpha)
    };

    if notation.starts_with('#') || is_unprefixed_hex(&notation) {
        let mut output = format_hex_like(&notation, color);
        if !notation.starts_with('#') {
            output.remove(0);
        }
        return output;
    }
    if notation.starts_with("rgb(") || notation.starts_with("rgba(") {
        return format_rgb_like(&notation, color);
    }
    if notation.starts_with("hsl(") || notation.starts_with("hsla(") {
        return format_hsl_like(&notation, color);
    }
    if notation.starts_with("hwb(") {
        return css_color.to_css_hwb().to_string();
    }
    if notation.starts_with("lab(") {
        return css_color.to_css_lab().to_string();
    }
    if notation.starts_with("lch(") {
        return css_color.to_css_lch().to_string();
    }
    if notation.starts_with("oklab(") {
        return css_color.to_css_oklab().to_string();
    }
    if notation.starts_with("oklch(") {
        return css_color.to_css_oklch().to_string();
    }
    if notation.starts_with("color(") {
        return format_color_function(&notation, &css_color);
    }
    if notation.starts_with("hsv(") || notation.starts_with("hsva(") {
        return format_hsv_like(&notation, &css_color);
    }
    if css_color.a == 1.0
        && let Some(name) = css_color.name()
    {
        return name.to_string();
    }
    css_color.to_css_rgb().to_string()
}

pub(super) fn format_color_as(original: &str, color: Color32, format: ColorFormat) -> String {
    let css_color = {
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        csscolorparser::Color::from_rgba8(red, green, blue, alpha)
    };
    match format {
        ColorFormat::Original => format_color_like(original, color),
        ColorFormat::Hex => format_hex(color),
        ColorFormat::Rgb => {
            let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
            if alpha == u8::MAX {
                format!("rgb({red} {green} {blue})")
            } else {
                format!(
                    "rgb({red} {green} {blue} / {:.4})",
                    f32::from(alpha) / 255.0
                )
            }
        }
        ColorFormat::Hsl => format_hsl_like("", color),
        ColorFormat::Hwb => css_color.to_css_hwb().to_string(),
        ColorFormat::Lab => css_color.to_css_lab().to_string(),
        ColorFormat::Lch => css_color.to_css_lch().to_string(),
        ColorFormat::Oklab => css_color.to_css_oklab().to_string(),
        ColorFormat::Oklch => css_color.to_css_oklch().to_string(),
        ColorFormat::ColorSrgb => css_color.to_css_color_srgb().to_string(),
        ColorFormat::Hsv => format_hsv_like("", &css_color),
        ColorFormat::Name => {
            if css_color.a == 1.0
                && let Some(name) = css_color.name()
            {
                name.to_string()
            } else {
                css_color.to_css_rgb().to_string()
            }
        }
    }
}

fn format_hex(color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    if alpha == u8::MAX {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

fn format_hex_like(original: &str, color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    let short_nibble = |channel: u8| (channel >> 4 == channel & 0x0f).then_some(channel >> 4);
    let short = [red, green, blue].map(short_nibble);
    let digits = original.strip_prefix('#').unwrap_or(original);
    let include_alpha = digits.len() == 4 || digits.len() == 8 || alpha != u8::MAX;
    if short.iter().all(Option::is_some) && (!include_alpha || short_nibble(alpha).is_some()) {
        let mut output = String::from("#");
        for value in short.into_iter().flatten() {
            output.push(char::from_digit(u32::from(value), 16).unwrap());
        }
        if include_alpha {
            output.push(char::from_digit(u32::from(short_nibble(alpha).unwrap()), 16).unwrap());
        }
        return output;
    }
    if include_alpha {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}")
    }
}

fn is_unprefixed_hex(value: &str) -> bool {
    matches!(value.len(), 3 | 4 | 6 | 8)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && parse_css_color(value).is_some()
}

fn format_rgb_like(original: &str, color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    let comma_syntax = original.contains(',');
    let rgba_function = original.starts_with("rgba(");
    if comma_syntax {
        if rgba_function || alpha != u8::MAX {
            format!(
                "rgba({red}, {green}, {blue}, {:.3})",
                f32::from(alpha) / 255.0
            )
        } else {
            format!("rgb({red}, {green}, {blue})")
        }
    } else if rgba_function {
        format!(
            "rgba({red} {green} {blue} / {:.3})",
            f32::from(alpha) / 255.0
        )
    } else if alpha == u8::MAX {
        format!("rgb({red} {green} {blue})")
    } else {
        format!(
            "rgb({red} {green} {blue} / {:.3})",
            f32::from(alpha) / 255.0
        )
    }
}

fn format_hsl_like(original: &str, color: Color32) -> String {
    let css_color = {
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        csscolorparser::Color::from_rgba8(red, green, blue, alpha)
    };
    let [hue, saturation, lightness, alpha] = css_color.to_hsla();
    let saturation = saturation * 100.0;
    let lightness = lightness * 100.0;
    if original.contains(',') {
        if original.starts_with("hsla(") || alpha < 1.0 {
            format!("hsla({hue:.3}, {saturation:.4}%, {lightness:.4}%, {alpha:.4})")
        } else {
            format!("hsl({hue:.3}, {saturation:.4}%, {lightness:.4}%)")
        }
    } else if alpha == 1.0 {
        format!("hsl({hue:.3} {saturation:.4}% {lightness:.4}%)")
    } else {
        format!("hsl({hue:.3} {saturation:.4}% {lightness:.4}% / {alpha:.4})")
    }
}

fn format_hsv_like(original: &str, color: &csscolorparser::Color) -> String {
    let [hue, saturation, value, alpha] = color.to_hsva();
    let saturation = saturation * 100.0;
    let value = value * 100.0;
    if original.starts_with("hsva(") || alpha < 1.0 {
        format!("hsva({hue:.3}, {saturation:.4}%, {value:.4}%, {alpha:.4})")
    } else {
        format!("hsv({hue:.3}, {saturation:.4}%, {value:.4}%)")
    }
}

fn format_color_function(original: &str, color: &csscolorparser::Color) -> String {
    let Some(space) = original
        .strip_prefix("color(")
        .and_then(|body| body.split_whitespace().next())
    else {
        return color.to_css_color_srgb().to_string();
    };
    let (space, components) = match space {
        "srgb-linear" => ("srgb-linear", color.to_linear_rgba()),
        "xyz" => ("xyz", color.to_xyz_d65()),
        "xyz-d65" => ("xyz-d65", color.to_xyz_d65()),
        "xyz-d50" => ("xyz-d50", color.to_xyz_d50()),
        _ => ("srgb", color.to_array()),
    };
    format!(
        "color({space} {} {} {}{})",
        components[0],
        components[1],
        components[2],
        if components[3] == 1.0 {
            String::new()
        } else {
            format!(" / {}", components[3])
        }
    )
}

pub(super) fn quoted_color_for_edit(
    original: &str,
    color: Color32,
    format: ColorFormat,
) -> Result<String, String> {
    serde_json::to_string(&format_color_as(original, color, format))
        .map_err(|error| format!("Не удалось сериализовать цвет: {error}"))
}

pub(super) fn color_swatch(ui: &mut Ui, color: Color32) {
    let size = ui.spacing().interact_size.y.min(14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
    let inset = rect.shrink(2.0);
    ui.painter().rect_filled(inset, 1.0, color);
    ui.painter().rect_stroke(
        rect,
        2.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    fn captured_screen(ctx: &egui::Context) -> CapturedScreen {
        CapturedScreen {
            width: 1920,
            height: 1080,
            pixels: vec![255; 1920 * 1080 * 4].into(),
            texture: ctx.load_texture(
                "test-screen",
                egui::ColorImage::filled([1, 1], Color32::WHITE),
                egui::TextureOptions::NEAREST,
            ),
            prepared: false,
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn eyedropper_window_starts_hidden() {
        let screen = captured_screen(&egui::Context::default());
        let builder = screen.viewport_builder(0);
        assert_eq!(builder.visible, Some(false));
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.resizable, Some(false));
        assert_eq!(builder.taskbar, Some(false));
    }

    #[cfg(not(windows))]
    #[test]
    fn cancelling_eyedropper_keeps_screen_image_in_last_frame() {
        let ctx = egui::Context::default();
        let id = egui::Id::unique("test-screen-picker");
        let screen = captured_screen(&ctx);
        let texture_id = screen.texture.id();
        ctx.data_mut(|data| {
            data.insert_temp(
                id,
                ScreenPickerState {
                    screens: vec![screen],
                    ..Default::default()
                },
            );
        });
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut initial = ctx.run_ui(input(), |ui| {
            show_eyedropper(ui.ctx(), id);
        });
        initial.textures_delta.clear();
        let mut cancel_input = input();
        cancel_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output = ctx.run_ui(cancel_input, |ui| {
            assert_eq!(show_eyedropper(ui.ctx(), id), Some(None));
        });
        output.textures_delta.clear();
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        assert!(primitives.iter().any(|primitive| {
            matches!(
                &primitive.primitive,
                egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture_id
            )
        }));
    }

    #[test]
    fn parses_css_and_extended_color_notations() {
        for input in [
            "rebeccapurple",
            "#abc",
            "abc",
            "#abcd",
            "#aabbcc",
            "#aabbccdd",
            "rgb(100% 0% 0% / 50%)",
            "rgba(255, 0, 0, 0.5)",
            "hsl(120 100% 50%)",
            "hsla(120, 100%, 50%, 0.5)",
            "hwb(240 0% 0%)",
            "lab(50% 20 30)",
            "lch(50% 40 30)",
            "oklab(50% 0.1 0.1)",
            "oklch(50% 0.1 30)",
            "color(srgb 1 0 0)",
            "color(srgb-linear 0.5 0 0 / 50%)",
            "color(xyz-d50 0.4 0.2 0.1)",
            "color(xyz 0.4 0.2 0.1)",
            "hsv(240 100% 100%)",
            "transparent",
            "\"red\"",
        ] {
            assert!(parse_color(input).is_some(), "failed to parse {input}");
        }
    }

    #[test]
    fn numeric_data_is_not_misidentified_as_unprefixed_hex() {
        for value in ["1", "12", "80", "123"] {
            assert_eq!(parse_color(value), None, "{value}");
        }
    }

    #[test]
    fn preserves_color_notation_when_formatting_picker_values() {
        let selected = Color32::from_rgb(20, 120, 220);
        for (input, prefix) in [
            ("#abc", "#"),
            ("abc", "1478dc"),
            ("rgb(1, 2, 3)", "rgb("),
            ("rgba(1, 2, 3, 0.5)", "rgba("),
            ("hsl(1, 2%, 3%)", "hsl("),
            ("hsla(1, 2%, 3%, 0.5)", "hsla("),
            ("hwb(1 2% 3%)", "hwb("),
            ("lab(1 2 3)", "lab("),
            ("lch(1 2 3)", "lch("),
            ("oklab(1 2 3)", "oklab("),
            ("oklch(1 2 3)", "oklch("),
            ("color(xyz-d50 0.4 0.2 0.1)", "color(xyz-d50 "),
            ("hsv(1, 2%, 3%)", "hsv("),
            ("red", "rgb("),
        ] {
            let formatted = format_color_like(input, selected);
            assert!(
                formatted.starts_with(prefix),
                "{input} changed notation to {formatted}"
            );
            assert!(
                csscolorparser::Color::from_html(&formatted).is_ok(),
                "invalid formatted value {formatted}"
            );
        }
    }

    #[test]
    fn picker_value_is_quoted_for_primitive_editing_and_keeps_format() {
        let color = Color32::from_rgb(20, 120, 220);
        for format in [
            struct_view_core::parser::DataFormat::Json,
            struct_view_core::parser::DataFormat::Json5,
            struct_view_core::parser::DataFormat::Yaml,
            struct_view_core::parser::DataFormat::Toml,
        ] {
            let source = match format {
                struct_view_core::parser::DataFormat::Json => r#"{"paint":"rgb(1, 2, 3)"}"#,
                struct_view_core::parser::DataFormat::Json5 => r#"{paint: "rgb(1, 2, 3)"}"#,
                struct_view_core::parser::DataFormat::Yaml => "paint: rgb(1, 2, 3)",
                struct_view_core::parser::DataFormat::Toml => r#"paint = "rgb(1, 2, 3)""#,
                _ => unreachable!(),
            };
            let mut root = struct_view_core::parser::parse_data(source, Some(format))
                .unwrap()
                .0;
            let node = root
                .children
                .iter_mut()
                .find(|node| node.key.as_deref() == Some("paint"))
                .unwrap();
            let edited =
                quoted_color_for_edit(&node.display_value, color, ColorFormat::Original).unwrap();
            struct_view_operations::app::edit::apply_primitive_edit(node, &edited, format).unwrap();
            assert_eq!(
                node.value_type,
                struct_view_core::parser::JsonValueType::String
            );
            assert!(
                node.display_value.contains("rgb("),
                "{}",
                node.display_value
            );
            assert_eq!(parse_color(&node.display_value), Some(color));
        }
    }

    #[test]
    fn explicit_output_formats_override_existing_notation() {
        let color = Color32::from_rgb(255, 0, 0);
        for (format, expected) in [
            (ColorFormat::Hex, "#ff0000"),
            (ColorFormat::Rgb, "rgb(255 0 0)"),
            (ColorFormat::Hsl, "hsl("),
            (ColorFormat::Hwb, "hwb("),
            (ColorFormat::Lab, "lab("),
            (ColorFormat::Lch, "lch("),
            (ColorFormat::Oklab, "oklab("),
            (ColorFormat::Oklch, "oklch("),
            (ColorFormat::ColorSrgb, "color(srgb "),
            (ColorFormat::Hsv, "hsv("),
            (ColorFormat::Name, "red"),
        ] {
            let formatted = format_color_as("#abc", color, format);
            assert!(
                formatted.starts_with(expected),
                "{format:?} produced {formatted}"
            );
            assert!(
                csscolorparser::Color::from_html(&formatted).is_ok(),
                "{format:?} produced invalid color {formatted}"
            );
        }
    }
}
