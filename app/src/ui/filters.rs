//! Filters + virtualized play list.

use egui::{Align2, Color32, RichText};
use nfl_core::dal::Play;
use nfl_core::filter::{Dim, Side};
use nfl_core::labels;
use nfl_core::style as s;

use super::widgets::{facet_combo, option_row, row_rect, truncate};
use crate::logos::Logos;
use crate::state::AppState;
use crate::theme::{self, c};

pub fn show(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    egui::SidePanel::left("filters")
        .resizable(false)
        .exact_width(320.0)
        .frame(egui::Frame::new().fill(c(s::SURFACE)).inner_margin(egui::Margin::same(12)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| filters(ui, st, logos));
        });
    egui::CentralPanel::default().show(ctx, |ui| list(ui, st, logos));
}

fn filters(ui: &mut egui::Ui, st: &mut AppState, logos: &Logos) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Filters").font(theme::heading()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear").clicked() {
                st.clear_filters();
            }
        });
    });
    let mut changes: Vec<(Dim, Option<String>)> = Vec::new();
    let mut combo = |ui: &mut egui::Ui, st: &AppState, d: Dim| {
        if let Some(v) = facet_combo(ui, &format!("f{d:?}"), d.title(), st.facets.get(&d), st.filter.get(d), logos) {
            changes.push((d, v));
        }
    };
    section(ui, "Game");
    combo(ui, st, Dim::Week);
    combo(ui, st, Dim::Game);
    section(ui, "Team");
    combo(ui, st, Dim::Team);
    let mut side = None;
    ui.label(RichText::new("Side").color(theme::muted()).font(theme::small()));
    ui.horizontal(|ui| {
        let counts = st.facets.get(&Dim::Side);
        for sd in Side::ALL {
            let n = counts.and_then(|f| f.options.iter().find(|o| o.value == sd.label())).map_or(0, |o| o.count);
            let sel = st.filter.side == sd;
            let b = egui::Button::new(RichText::new(format!("{} {}", sd.label(), labels::thousands(n))).color(Color32::WHITE))
                .fill(if sel { c(s::NAVY) } else { c(s::BG) });
            if ui.add(b).clicked() {
                side = Some(sd);
            }
        }
    });
    section(ui, "Play state");
    for d in Dim::PLAY_STATE {
        combo(ui, st, d);
    }
    section(ui, "Player");
    // Player picker: team → position → player (no text input; Android keyboard bug).
    let teams = st.facets.get(&Dim::Team).cloned();
    let mut picker_team = None;
    if let Some(v) = facet_combo(ui, "pteam", "Team", teams.as_ref(), st.picker_team.as_deref(), logos) {
        picker_team = Some(v);
    }
    let mut picker_pos = None;
    let mut pick_player = None;
    if st.picker_team.is_some() {
        ui.label(RichText::new("Position").color(theme::muted()).font(theme::small()));
        egui::ComboBox::from_id_salt("ppos")
            .width(ui.available_width() - 8.0)
            .selected_text(st.picker_position.clone().unwrap_or_else(|| "Any".into()))
            .show_ui(ui, |ui| {
                ui.set_min_width(320.0);
                if ui.selectable_label(st.picker_position.is_none(), "Any").clicked() {
                    picker_pos = Some(None);
                }
                for o in &st.positions {
                    if option_row(ui, o, st.picker_position.as_deref() == Some(&o.value), logos) {
                        picker_pos = Some(Some(o.value.clone()));
                    }
                }
            });
        let cur = st.filter.get(Dim::Player).map(str::to_string);
        ui.label(RichText::new("Player").color(theme::muted()).font(theme::small()));
        let sel = cur.as_deref().and_then(|v| st.players.iter().find(|o| o.value == v)).map_or("Any".to_string(), |o| o.label.clone());
        egui::ComboBox::from_id_salt("pplayer").width(ui.available_width() - 8.0).height(460.0).selected_text(sel).show_ui(ui, |ui| {
            ui.set_min_width(360.0);
            if ui.selectable_label(cur.is_none(), "Any").clicked() {
                pick_player = Some(None);
            }
            for o in &st.players {
                if option_row(ui, o, cur.as_deref() == Some(&o.value), logos) {
                    pick_player = Some(Some(o.value.clone()));
                }
            }
        });
        if let Some(id) = cur.and_then(|v| v.parse::<i64>().ok()) {
            if ui.add(egui::Button::new(RichText::new("View player stats").color(Color32::WHITE)).fill(c(s::RED))).clicked() {
                st.open_player(id);
            }
        }
    }
    for (d, v) in changes {
        st.set_filter(d, v);
    }
    if let Some(sd) = side {
        st.set_side(sd);
    }
    if let Some(t) = picker_team {
        st.set_picker_team(t);
    }
    if let Some(p) = picker_pos {
        st.set_picker_position(p);
    }
    if let Some(p) = pick_player {
        st.set_filter(Dim::Player, p);
    }
}

fn section(ui: &mut egui::Ui, t: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(t.to_uppercase()).color(theme::muted()).font(theme::small()).strong());
    ui.separator();
}

fn list(ui: &mut egui::Ui, st: &mut AppState, logos: &Logos) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} plays", labels::thousands(st.plays.len() as u64))).font(theme::heading()).strong());
        ui.label(RichText::new("tap a play to animate it").color(theme::muted()));
    });
    ui.add_space(4.0);
    let h = 58.0;
    let mut open = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, h + 4.0, st.plays.len(), |ui, range| {
        for p in &st.plays[range] {
            let sel = st.selected_play == Some((p.game_id, p.play_id));
            if play_row(ui, p, sel, h, logos) {
                open = Some((p.game_id, p.play_id));
            }
            ui.add_space(4.0);
        }
    });
    if let Some((g, p)) = open {
        st.open_play(g, p);
    }
}

pub fn result_text(p: &Play) -> (String, nfl_core::style::Rgb) {
    let code = p.pass_result.as_deref().unwrap_or("");
    let yards = p.play_result.map(|y| format!(" {y:+}")).unwrap_or_default();
    (format!("{}{yards}", labels::pass_result(code)), s::result_color(code))
}

fn play_row(ui: &mut egui::Ui, p: &Play, sel: bool, h: f32, logos: &Logos) -> bool {
    let (rect, resp) = row_rect(ui, h, sel);
    let pa = ui.painter();
    let (x0, y1, y2) = (rect.left() + 12.0, rect.top() + 16.0, rect.top() + 40.0);
    pa.text(egui::pos2(x0, y1), Align2::LEFT_CENTER, format!("{} · {}", labels::quarter(p.quarter), p.game_clock), theme::mono(), Color32::WHITE);
    pa.text(egui::pos2(x0, y2), Align2::LEFT_CENTER, p.situation(), theme::small(), theme::muted());
    let lx = x0 + 150.0;
    logos.paint(pa, egui::Rect::from_center_size(egui::pos2(lx + 12.0, rect.center().y), egui::vec2(26.0, 26.0)), &p.offense, 1.0);
    pa.text(egui::pos2(lx + 30.0, rect.center().y), Align2::LEFT_CENTER, format!("{} vs {}", p.offense, p.defense), theme::body(), Color32::WHITE);
    let (res, col) = result_text(p);
    let cx = lx + 120.0;
    let chip = egui::Rect::from_min_size(egui::pos2(cx, rect.center().y - 11.0), egui::vec2(124.0, 22.0));
    pa.rect_filled(chip, 11.0, theme::c(col));
    pa.text(chip.center(), Align2::CENTER_CENTER, res, theme::small(), theme::on(col));
    let dx = cx + 136.0;
    let maxc = ((rect.right() - dx - 220.0) / 7.0).max(10.0) as usize;
    pa.text(egui::pos2(dx, rect.center().y), Align2::LEFT_CENTER, truncate(&p.description, maxc), theme::small(), theme::muted());
    let mut bx = rect.right() - 10.0;
    let mut badges = Vec::new();
    if let Some(t) = p.time_to_throw {
        badges.push(format!("{t:.1} s"));
    }
    if let Some(cv) = &p.coverage {
        badges.push(cv.clone());
    }
    if p.play_action {
        badges.push("PA".into());
    }
    for b in badges {
        let g = pa.layout_no_wrap(b, theme::small(), Color32::WHITE);
        let w = g.size().x + 14.0;
        let r = egui::Rect::from_min_size(egui::pos2(bx - w, rect.center().y - 10.0), egui::vec2(w, 20.0));
        pa.rect_filled(r, 10.0, c(s::SURFACE_RAISED));
        pa.galley(egui::pos2(r.left() + 7.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
        bx -= w + 6.0;
    }
    resp.clicked()
}
