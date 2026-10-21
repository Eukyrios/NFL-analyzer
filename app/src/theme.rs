//! Converts `nfl_core::style` tokens into egui colors, visuals and text styles.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use nfl_core::style::{self as s, Rgb, Rgba, Sizes};

pub fn c((r, g, b): Rgb) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn ca((r, g, b, a): Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// `c` with an alpha in 0..=1.
pub fn alpha(rgb: Rgb, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(rgb.0, rgb.1, rgb.2, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub const IS_PHONE: bool = cfg!(any(target_os = "android", target_os = "ios"));

pub fn sizes() -> &'static Sizes {
    if IS_PHONE { &s::PHONE } else { &s::DESKTOP }
}

pub fn muted() -> Color32 {
    c(s::TEXT_MUTED)
}

pub fn disabled() -> Color32 {
    alpha(s::TEXT_MUTED, 0.5)
}

/// Text color for a fill: white, or `#111111` on light colors.
pub fn on(fill: Rgb) -> Color32 {
    if nfl_core::geometry::is_light(fill) { c(s::DARK_TEXT) } else { Color32::WHITE }
}

pub fn apply(ctx: &egui::Context) {
    let z = sizes();
    let mut style = (*ctx.style()).clone();
    let mut v = Visuals::dark();
    let r6 = CornerRadius::same(s::RADIUS_BUTTON as u8);
    v.panel_fill = c(s::BG);
    v.window_fill = c(s::SURFACE_RAISED);
    v.faint_bg_color = c(s::SURFACE);
    v.extreme_bg_color = c(s::BG);
    v.code_bg_color = c(s::SURFACE);
    v.window_stroke = Stroke::new(1.0, c(s::BORDER));
    v.window_corner_radius = CornerRadius::same(s::RADIUS_PANEL as u8);
    v.menu_corner_radius = CornerRadius::same(s::RADIUS_PANEL as u8);
    v.hyperlink_color = c(s::RESULT_R);
    v.selection.bg_fill = c(s::NAVY);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.override_text_color = None;
    let w = &mut v.widgets;
    for (wv, fill) in [
        (&mut w.noninteractive, s::BG),
        (&mut w.inactive, s::SURFACE),
        (&mut w.hovered, s::SURFACE_RAISED),
        (&mut w.active, s::NAVY),
        (&mut w.open, s::SURFACE_RAISED),
    ] {
        wv.bg_fill = c(fill);
        wv.weak_bg_fill = c(fill);
        wv.corner_radius = r6;
        wv.bg_stroke = Stroke::new(1.0, c(s::BORDER));
        wv.fg_stroke = Stroke::new(1.0, c(s::TEXT));
    }
    w.noninteractive.fg_stroke = Stroke::new(1.0, c(s::TEXT));
    w.noninteractive.bg_stroke = Stroke::new(1.0, c(s::BORDER));
    style.visuals = v;

    style.text_styles = [
        (TextStyle::Heading, FontId::new(z.heading, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(z.body, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(z.body, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(z.small, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(z.body, FontFamily::Monospace)),
    ]
    .into();

    let sp = &mut style.spacing;
    sp.item_spacing = egui::vec2(s::SPACE[1], s::SPACE[1]);
    sp.button_padding = egui::vec2(s::SPACE[2], s::SPACE[0]);
    sp.interact_size.y = z.touch_target;
    sp.combo_width = 260.0;
    sp.scroll.bar_width = if IS_PHONE { 8.0 } else { 6.0 };
    style.interaction.tooltip_delay = 0.2;
    ctx.set_style(style);
}

pub fn small() -> FontId {
    FontId::proportional(sizes().small)
}
pub fn body() -> FontId {
    FontId::proportional(sizes().body)
}
pub fn heading() -> FontId {
    FontId::proportional(sizes().heading)
}
pub fn mono() -> FontId {
    FontId::monospace(sizes().body)
}
