//! Home screen (US map of all 32 teams) and the team page (roster and
//! season matches).

use egui::{Align2, Color32, CornerRadius, RichText, Sense, Stroke};
use nfl_core::roster::position_group;
use nfl_core::style as s;
use nfl_core::{labels, teams, usmap};

use super::widgets::{row_rect, stat};
use crate::logos::Logos;
use crate::state::{AppState, TeamTab};
use crate::theme::{self, c};

/// Home screen: the US map with every team's logo at its stadium. Tapping a
/// logo (or the map near one) opens a pop-up listing that state's teams;
/// picking a team opens its page.
pub fn map(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    let mut open: Option<&'static str> = None;
    let mut pick_state: Option<&'static str> = None;
    let selected = st.map_state.clone();
    egui::CentralPanel::default().show(ctx, |ui| {
        {
            let w = ui.available_width();
            let h = ui.available_height();
            {
            let (rect, map_resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, CornerRadius::same(s::RADIUS_PANEL as u8), c(s::SURFACE));
            let fit = usmap::MapFit::new(rect.left() + 16.0, rect.top() + 16.0, rect.width() - 32.0, rect.height() - 32.0);

            // The painter only fills convex shapes, so fill the outline row by row.
            let poly: Vec<(f32, f32)> = usmap::OUTLINE.iter().map(|&(lon, lat)| fit.to_screen(lon, lat)).collect();
            let mut y = rect.top();
            while y < rect.bottom() {
                for (x0, x1) in usmap::scanline_spans(&poly, y + 1.0) {
                    p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1, y + 2.0)), 0.0, c(s::SURFACE_RAISED));
                }
                y += 2.0;
            }
            let mut outline: Vec<egui::Pos2> = poly.iter().map(|&(x, y)| egui::pos2(x, y)).collect();
            outline.push(outline[0]);
            p.add(egui::Shape::line(outline, Stroke::new(1.5, c(s::BORDER))));

            let logo = (fit.scale * 0.035).clamp(26.0, 52.0).max(theme::sizes().touch_target);
            let at: Vec<(f32, f32)> = usmap::STADIUMS.iter().map(|t| fit.to_screen(t.3, t.2)).collect();
            let spread = usmap::spread(&at, logo * 0.95, 60);
            let pointer = ui.ctx().pointer_hover_pos();
            let in_sel = |abbr: &str| selected.is_some() && usmap::state_of(abbr) == selected.as_deref();
            for (i, t) in usmap::STADIUMS.iter().enumerate() {
                let (sx, sy) = at[i];
                let (x, y) = spread[i];
                if (x - sx).hypot(y - sy) > 2.0 {
                    p.line_segment([egui::pos2(sx, sy), egui::pos2(x, y)], Stroke::new(1.0, theme::muted()));
                    p.circle_filled(egui::pos2(sx, sy), 2.5, theme::muted());
                }
                let r = egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(logo, logo));
                let resp = ui.interact(r, ui.id().with(("team", t.0)), Sense::click());
                if in_sel(t.0) {
                    p.circle_stroke(r.center(), logo * 0.62, Stroke::new(3.0, c(s::RED)));
                } else if resp.hovered() || pointer.is_some_and(|q| r.contains(q)) {
                    p.circle_filled(r.center(), logo * 0.62, theme::alpha(s::RED, 0.35));
                }
                let dim = if selected.is_some() && !in_sel(t.0) { 0.45 } else { 1.0 };
                logos.paint(&p, r, t.0, dim);
                let name = teams::team(t.0).map_or(t.0.to_string(), |x| x.name.to_string());
                if resp.on_hover_text(format!("{name}\n{}", t.1)).clicked() {
                    pick_state = usmap::state_of(t.0);
                }
            }
            // A tap on the map itself picks the state of the nearest stadium.
            if let Some(q) = map_resp.clicked().then(|| map_resp.interact_pointer_pos()).flatten() {
                pick_state = at
                    .iter()
                    .enumerate()
                    .map(|(i, a)| (i, (a.0 - q.x).hypot(a.1 - q.y)))
                    .filter(|(_, d)| *d < rect.width() * 0.09)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .and_then(|(i, _)| usmap::state_of(usmap::STADIUMS[i].0));
            }
            if let Some(code) = selected.as_deref() {
                let label = format!("{} · {}", usmap::state_name(code), code);
                p.text(rect.left_top() + egui::vec2(14.0, 12.0), Align2::LEFT_TOP, label, theme::heading(), Color32::WHITE);
            }

            }
        }
    });
    if let Some(code) = selected.as_deref() {
        state_modal(ctx, code, logos, &mut open, &mut pick_state);
    }
    if let Some(code) = pick_state {
        // Tapping the selected state again clears it.
        st.map_state = if st.map_state.as_deref() == Some(code) { None } else { Some(code.to_string()) };
    }
    match open {
        Some(abbr) => st.open_team(abbr),
        None => {}
    }
}

/// Pop-up with the selected state's teams; tapping one opens its page.
fn state_modal(
    ctx: &egui::Context,
    code: &str,
    logos: &Logos,
    open: &mut Option<&'static str>,
    pick_state: &mut Option<&'static str>,
) {
    let close_code = usmap::STADIUMS.iter().find_map(|t| usmap::state_of(t.0).filter(|s| *s == code));
    let modal = egui::Modal::new(egui::Id::new("state-modal")).show(ctx, |ui| {
        ui.set_width(380.0f32.min(ctx.screen_rect().width() - 48.0));
        ui.horizontal(|ui| {
            ui.label(RichText::new(usmap::state_name(code)).font(theme::heading()).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✖").clicked() {
                    *pick_state = close_code;
                }
            });
        });
        let list = usmap::teams_in_state(code);
        let n = list.len();
        ui.label(RichText::new(format!("{n} team{}", if n == 1 { "" } else { "s" })).color(theme::muted()));
        ui.add_space(6.0);
        for abbr in list {
            let (rect, resp) = row_rect(ui, 64.0f32.max(theme::sizes().touch_target), false);
            let p = ui.painter();
            let y = rect.center().y;
            logos.paint(p, egui::Rect::from_center_size(egui::pos2(rect.left() + 34.0, y), egui::vec2(44.0, 44.0)), abbr, 1.0);
            let name = teams::team(abbr).map_or(abbr.to_string(), |t| t.name.to_string());
            p.text(egui::pos2(rect.left() + 70.0, y - 9.0), Align2::LEFT_CENTER, name, theme::body(), Color32::WHITE);
            let city = usmap::stadium(abbr).map_or("", |s| s.0);
            p.text(egui::pos2(rect.left() + 70.0, y + 10.0), Align2::LEFT_CENTER, city, theme::small(), theme::muted());
            p.text(egui::pos2(rect.right() - 12.0, y), Align2::RIGHT_CENTER, "▶", theme::body(), theme::muted());
            if resp.clicked() {
                *open = Some(abbr);
            }
            ui.add_space(4.0);
        }
    });
    if modal.should_close() {
        *pick_state = close_code;
    }
}

enum Action {
    Player(i64),
    Game(i64),
}

/// One team: header with record, then Lineup / Roster / Games tabs.
pub fn team(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    let mut action = None;
    egui::CentralPanel::default().show(ctx, |ui| {
        let Some(td) = st.team.as_mut() else {
            ui.label("Couldn't load this team.");
            return;
        };
        let info = teams::team(&td.abbr);
        ui.horizontal(|ui| {
            logos.widget(ui, &td.abbr, 72.0);
            ui.vertical(|ui| {
                let name = info.map_or(td.abbr.clone(), |t| t.name.to_string());
                ui.label(RichText::new(name).font(egui::FontId::proportional(theme::sizes().heading + 6.0)).strong());
                let city = usmap::stadium(&td.abbr).map_or("", |s| s.0);
                ui.label(RichText::new(format!("{} · {city}", td.abbr)).color(theme::muted()));
            });
            ui.add_space(16.0);
            let (w, l, t) = td.record();
            let record = if t > 0 { format!("{w}–{l}–{t}") } else { format!("{w}–{l}") };
            stat(ui, "Record, weeks 1–8", &record);
            let off: u64 = td.games.iter().map(|g| g.plays_off).sum();
            let def: u64 = td.games.iter().map(|g| g.plays_def).sum();
            stat(ui, "Dropbacks on offense", &labels::thousands(off));
            stat(ui, "Dropbacks faced", &labels::thousands(def));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for (label, tab) in [("Roster", TeamTab::Roster), ("Season matches", TeamTab::Games)] {
                let sel = td.tab == tab;
                let b = egui::Button::new(RichText::new(label).color(Color32::WHITE))
                    .fill(if sel { c(s::NAVY) } else { c(s::SURFACE) })
                    .min_size(egui::vec2(96.0, theme::sizes().touch_target));
                if ui.add(b).clicked() {
                    td.tab = tab;
                }
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match td.tab {
            TeamTab::Roster => {
                let mut roster: Vec<_> = td.roster.iter().collect();
                roster.sort_by_key(|r| (position_group(&r.position).2, std::cmp::Reverse(r.snaps)));
                let mut group = "";
                for r in roster {
                    let (_, g, _) = position_group(&r.position);
                    if g != group {
                        group = g;
                        ui.add_space(8.0);
                        ui.label(RichText::new(g).font(theme::heading()).strong());
                    }
                    let (rect, resp) = row_rect(ui, 40.0f32.max(theme::sizes().touch_target), false);
                    let p = ui.painter();
                    let y = rect.center().y;
                    let jersey = r.jersey.map_or("–".into(), |j| format!("#{j}"));
                    p.text(egui::pos2(rect.left() + 12.0, y), Align2::LEFT_CENTER, jersey, theme::mono(), theme::muted());
                    p.text(egui::pos2(rect.left() + 64.0, y), Align2::LEFT_CENTER, &r.name, theme::body(), Color32::WHITE);
                    p.text(egui::pos2(rect.left() + 280.0, y), Align2::LEFT_CENTER, format!("{} · {}", r.position, labels::position(&r.position)), theme::small(), theme::muted());
                    p.circle_filled(egui::pos2(rect.left() + 470.0, y), 5.0, c(s::role_color(&r.role)));
                    p.text(egui::pos2(rect.left() + 482.0, y), Align2::LEFT_CENTER, labels::role(&r.role), theme::small(), Color32::WHITE);
                    p.text(egui::pos2(rect.right() - 12.0, y), Align2::RIGHT_CENTER, format!("{} snaps", labels::thousands(r.snaps)), theme::mono(), theme::muted());
                    if resp.clicked() {
                        action = Some(Action::Player(r.nfl_id));
                    }
                    ui.add_space(4.0);
                }
            }
            TeamTab::Games => {
                for g in &td.games {
                    let (rect, resp) = row_rect(ui, 48.0f32.max(theme::sizes().touch_target), false);
                    let p = ui.painter();
                    let y = rect.center().y;
                    p.text(egui::pos2(rect.left() + 12.0, y), Align2::LEFT_CENTER, format!("Week {}", g.game.week), theme::body(), Color32::WHITE);
                    logos.paint(p, egui::Rect::from_center_size(egui::pos2(rect.left() + 110.0, y), egui::vec2(30.0, 30.0)), &g.opponent, 1.0);
                    p.text(egui::pos2(rect.left() + 132.0, y), Align2::LEFT_CENTER, g.opponent_label(), theme::body(), Color32::WHITE);
                    let mut when = g.game.kickoff_label();
                    if g.game.is_neutral() {
                        when += &format!(" · Neutral · {}", g.game.stadium.as_deref().unwrap_or("neutral site"));
                    }
                    p.text(egui::pos2(rect.left() + 230.0, y), Align2::LEFT_CENTER, when, theme::small(), theme::muted());
                    if let Some(res) = g.result_label() {
                        let col = match res.chars().next() {
                            Some('W') => s::RESULT_C,
                            Some('L') => s::RESULT_S,
                            _ => s::RESULT_I,
                        };
                        p.text(egui::pos2(rect.right() - 230.0, y), Align2::LEFT_CENTER, res, theme::mono(), c(col));
                    }
                    p.text(egui::pos2(rect.right() - 12.0, y), Align2::RIGHT_CENTER, format!("{} off · {} def ▶", g.plays_off, g.plays_def), theme::small(), theme::muted());
                    if resp.clicked() {
                        action = Some(Action::Game(g.game.game_id));
                    }
                    ui.add_space(4.0);
                }
            }
        });
    });
    let abbr = st.team.as_ref().map(|t| t.abbr.clone());
    match (action, abbr) {
        (Some(Action::Player(id)), _) => st.open_player(id),
        (Some(Action::Game(gid)), Some(_)) => st.show_game(gid),
        _ => {}
    }
}
