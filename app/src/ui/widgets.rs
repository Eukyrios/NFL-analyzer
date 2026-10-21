//! Shared widgets: faceted dropdowns, chips, cards.

use egui::{Align2, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Ui};
use nfl_core::filter::{Facet, FacetOption};
use nfl_core::style::{self as s, Rgb};

use crate::logos::Logos;
use crate::theme::{self, c};

/// A dropdown whose rows show label, detail and count. Returns the new
/// selection (`Some(None)` = "Any") when the user picked something.
pub fn facet_combo(ui: &mut Ui, id: &str, title: &str, facet: Option<&Facet>, current: Option<&str>, logos: &Logos) -> Option<Option<String>> {
    let empty = Facet::default();
    let facet = facet.unwrap_or(&empty);
    let sel_label = current
        .and_then(|v| facet.options.iter().find(|o| o.value == v))
        .map(|o| o.label.clone())
        .unwrap_or_else(|| "Any".into());
    let mut out = None;
    ui.label(RichText::new(title).color(theme::muted()).font(theme::small()));
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width() - 8.0)
        .height(420.0)
        .selected_text(sel_label)
        .show_ui(ui, |ui| {
            ui.set_min_width(340.0);
            let any = FacetOption {
                value: String::new(),
                label: "Any".into(),
                detail: format!("{} plays", nfl_core::labels::thousands(facet.any)),
                count: facet.any,
                team: None,
                team2: None,
            };
            if option_row(ui, &any, current.is_none(), logos) {
                out = Some(None);
            }
            for o in &facet.options {
                if option_row(ui, o, current == Some(o.value.as_str()), logos) {
                    out = Some(Some(o.value.clone()));
                }
            }
        });
    out
}

/// One dropdown row; returns true when clicked.
pub fn option_row(ui: &mut Ui, o: &FacetOption, selected: bool, logos: &Logos) -> bool {
    let h = theme::sizes().touch_target.max(40.0);
    let w = ui.available_width().max(320.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    let p = ui.painter();
    let zero = o.count == 0;
    if selected {
        p.rect_filled(rect, CornerRadius::same(4), c(s::NAVY));
        p.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, h)), 0.0, c(s::RED));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(4), c(s::SURFACE_RAISED));
    }
    let text = if zero { theme::disabled() } else { Color32::WHITE };
    let sub = if zero { theme::disabled() } else { theme::muted() };
    let mut x = rect.left() + 8.0;
    let logo = h - 14.0;
    for t in [&o.team, &o.team2].into_iter().flatten() {
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - logo / 2.0), egui::vec2(logo, logo));
        logos.paint(p, r, t, if zero { 0.4 } else { 1.0 });
        x += logo + 4.0;
    }
    if let Some(col) = dot_color(o) {
        p.circle_filled(egui::pos2(x + 5.0, rect.center().y), 5.0, theme::c(col));
        x += 16.0;
    }
    p.text(egui::pos2(x + 2.0, rect.top() + 4.0), Align2::LEFT_TOP, &o.label, theme::body(), text);
    p.text(egui::pos2(x + 2.0, rect.bottom() - 4.0), Align2::LEFT_BOTTOM, truncate(&o.detail, 62), theme::small(), sub);
    p.text(egui::pos2(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, nfl_core::labels::thousands(o.count), theme::mono(), sub);
    resp.clicked()
}

fn dot_color(o: &FacetOption) -> Option<Rgb> {
    matches!(o.value.as_str(), "C" | "I" | "S" | "IN" | "R")
        .then(|| nfl_core::style::result_color(&o.value))
        .filter(|_| ["Complete", "Incomplete", "Sack", "Interception", "Scramble"].contains(&o.label.as_str()))
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
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
