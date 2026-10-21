//! Shared widgets: faceted dropdowns, chips, cards.

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Ui};
use nfl_core::style::{self as s, Rgb};

use crate::theme::{self, c};

}

/// Fully rounded chip; `fill` None = surface with border.
pub fn chip(ui: &mut Ui, text: &str, fill: Option<Rgb>) {
    let (bg, fg) = match fill {
        Some(f) => (c(f), theme::on(f)),
        None => (c(s::SURFACE_RAISED), Color32::WHITE),
    };
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, c(s::BORDER)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| ui.label(RichText::new(text).color(fg).font(theme::small())));
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(c(s::SURFACE))
        .corner_radius(CornerRadius::same(s::RADIUS_PANEL as u8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, add)
        .inner
}

/// Small stat card: big value over a muted label.
pub fn stat(ui: &mut Ui, label: &str, value: &str) {
    egui::Frame::new()
        .fill(c(s::SURFACE))
        .corner_radius(CornerRadius::same(s::RADIUS_PANEL as u8))
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(value).font(theme::heading()).strong());
                ui.label(RichText::new(label).color(theme::muted()).font(theme::small()));
            });
        });
}

/// A full-width clickable row with hover / navy selection styling.
pub fn row_rect(ui: &mut Ui, h: f32, selected: bool) -> (egui::Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    let fill = if selected { c(s::NAVY) } else if resp.hovered() { c(s::SURFACE_RAISED) } else { c(s::SURFACE) };
    p.rect_filled(rect, CornerRadius::same(6), fill);
    if selected {
        p.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, h)), 0.0, c(s::RED));
    }
    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, c(s::BORDER)), StrokeKind::Inside);
    (rect, resp)
}

/// Result label and color for a play, e.g. "Complete +49".
pub fn result_text(p: &nfl_core::dal::Play) -> (String, Rgb) {
    let code = p.pass_result.as_deref().unwrap_or("");
    let yards = p.play_result.map(|y| format!(" {y:+}")).unwrap_or_default();
    (format!("{}{yards}", nfl_core::labels::pass_result(code)), s::result_color(code))
}
