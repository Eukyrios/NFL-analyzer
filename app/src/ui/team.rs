//! Teams tab: a US map of all 32 teams, and a team page with lineup,
//! roster and schedule.

use egui::{Align2, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind};
use nfl_core::geometry::WID;
use nfl_core::roster::{position_group, LineupSlot};
use nfl_core::style as s;
use nfl_core::{labels, teams, usmap};

use super::widgets::{row_rect, stat};
use crate::logos::Logos;
use crate::state::{AppState, TeamTab};
use crate::theme::{self, c};

/// The US map with every team's logo at its stadium. Tapping the map or a
/// logo selects that state; its teams are listed beside the map with
/// Roster / Games shortcuts. A grid of all teams sits below.
pub fn map(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    let mut open: Option<(&'static str, Option<TeamTab>)> = None;
    let mut pick_state: Option<&'static str> = None;
    let selected = st.map_state.clone();
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("Teams").font(theme::heading()).strong());
            ui.label(RichText::new("Tap a state to see its teams, then open a roster or the games.").color(theme::muted()));
            ui.add_space(8.0);

            // Wide screens: map and state panel side by side; phones: stacked.
            let full = ui.available_width();
            let side = full >= 900.0;
            let w = if side { full - 312.0 } else { full };
            let h = (w * 0.58).min(ui.available_height().max(360.0));
            ui.horizontal_top(|ui| {
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
                    .min_by(|a, b| (a.1 .0 - q.x).hypot(a.1 .1 - q.y).total_cmp(&(b.1 .0 - q.x).hypot(b.1 .1 - q.y)))
                    .and_then(|(i, _)| usmap::state_of(usmap::STADIUMS[i].0));
            }
            if let Some(code) = selected.as_deref() {
                let label = format!("{} · {}", usmap::state_name(code), code);
                p.text(rect.left_top() + egui::vec2(14.0, 12.0), Align2::LEFT_TOP, label, theme::heading(), Color32::WHITE);
            }

            if side {
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    state_panel(ui, selected.as_deref(), logos, &mut open, &mut pick_state);
                });
            }
            });
            if !side {
                ui.add_space(8.0);
                state_panel(ui, selected.as_deref(), logos, &mut open, &mut pick_state);
            }

            ui.add_space(12.0);
            ui.label(RichText::new("All teams").font(theme::heading()).strong());
            ui.add_space(4.0);
            let mut all: Vec<&teams::TeamInfo> = usmap::STADIUMS.iter().filter_map(|t| teams::team(t.0)).collect();
            all.sort_by_key(|t| t.name);
            let cols = ((ui.available_width() / 220.0).floor() as usize).max(1);
            let bw = ui.available_width() / cols as f32 - 8.0;
            egui::Grid::new("team-grid").spacing([8.0, 8.0]).show(ui, |ui| {
                for (i, t) in all.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(bw, 44.0f32.max(theme::sizes().touch_target)), Sense::click());
                    let p = ui.painter();
                    let fill = if resp.hovered() { c(s::SURFACE_RAISED) } else { c(s::SURFACE) };
                    p.rect_filled(rect, CornerRadius::same(6), fill);
                    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, c(s::BORDER)), StrokeKind::Inside);
                    let lr = egui::Rect::from_center_size(egui::pos2(rect.left() + 24.0, rect.center().y), egui::vec2(30.0, 30.0));
                    logos.paint(p, lr, t.abbr, 1.0);
                    p.text(egui::pos2(rect.left() + 46.0, rect.center().y), Align2::LEFT_CENTER, t.name, theme::body(), Color32::WHITE);
                    if resp.clicked() {
                        open = Some((t.abbr, None));
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    if let Some(code) = pick_state {
        // Tapping the selected state again clears it.
        st.map_state = if st.map_state.as_deref() == Some(code) { None } else { Some(code.to_string()) };
    }
    match open {
        Some((abbr, Some(tab))) => st.open_team_tab(abbr, tab),
        Some((abbr, None)) => st.open_team(abbr),
        None => {}
    }
}

/// The selected state's teams, each with Roster / Games shortcuts.
fn state_panel(
    ui: &mut egui::Ui,
    state: Option<&str>,
    logos: &Logos,
    open: &mut Option<(&'static str, Option<TeamTab>)>,
    pick_state: &mut Option<&'static str>,
) {
    egui::Frame::new()
        .fill(c(s::SURFACE))
        .corner_radius(CornerRadius::same(s::RADIUS_PANEL as u8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let Some(code) = state else {
                ui.label(RichText::new("No state selected").strong());
                ui.label(RichText::new("Tap a state or a logo on the map.").color(theme::muted()));
                return;
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(usmap::state_name(code)).font(theme::heading()).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✖").on_hover_text("Clear").clicked() {
                        *pick_state = usmap::STADIUMS.iter().find_map(|t| usmap::state_of(t.0).filter(|s| *s == code));
                    }
                });
            });
            let list = usmap::teams_in_state(code);
            let n = list.len();
            ui.label(RichText::new(format!("{n} team{}", if n == 1 { "" } else { "s" })).color(theme::muted()));
            ui.add_space(6.0);
            for abbr in list {
                let info = teams::team(abbr);
                ui.horizontal(|ui| {
                    if logos.widget(ui, abbr, 40.0).interact(Sense::click()).clicked() {
                        *open = Some((abbr, None));
                    }
                    ui.vertical(|ui| {
                        let name = info.map_or(abbr.to_string(), |t| t.name.to_string());
                        if ui.link(RichText::new(name).strong()).clicked() {
                            *open = Some((abbr, None));
                        }
                        let city = usmap::stadium(abbr).map_or("", |s| s.0);
                        ui.label(RichText::new(city).color(theme::muted()).font(theme::small()));
                        ui.horizontal(|ui| {
                            let size = egui::vec2(84.0, theme::sizes().touch_target);
                            if ui.add(egui::Button::new("Roster").fill(c(s::NAVY)).min_size(size)).clicked() {
                                *open = Some((abbr, Some(TeamTab::Roster)));
                            }
                            if ui.add(egui::Button::new("Games").fill(c(s::NAVY)).min_size(size)).clicked() {
                                *open = Some((abbr, Some(TeamTab::Games)));
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }
        });
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
            for (label, tab) in [("Lineup", TeamTab::Lineup), ("Roster", TeamTab::Roster), ("Games", TeamTab::Games)] {
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
            TeamTab::Lineup => {
                ui.label(RichText::new("Most-used players at each position (offense left, defense right)").color(theme::muted()).font(theme::small()));
                ui.add_space(4.0);
                if let Some(id) = lineup(ui, &td.abbr, &td.lineup.offense, &td.lineup.defense) {
                    action = Some(Action::Player(id));
                }
            }
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
        (Some(Action::Game(gid)), Some(abbr)) => st.show_game_plays(gid, Some(&abbr), None),
        _ => {}
    }
}

/// Draws the lineup on a strip of field around the line of scrimmage.
/// Returns the clicked player's nflId.
fn lineup(ui: &mut egui::Ui, abbr: &str, offense: &[LineupSlot], defense: &[LineupSlot]) -> Option<i64> {
    const X0: f32 = -9.0;
    const X1: f32 = 16.0;
    let w = ui.available_width();
    let h = (w * WID / (X1 - X0) * 0.55).clamp(320.0, 620.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    let sx = rect.width() / (X1 - X0);
    let sy = rect.height() / WID;
    let to = |dx: f32, y: f32| egui::pos2(rect.left() + (dx - X0) * sx, rect.top() + (WID - y) * sy);

    p.rect_filled(rect, CornerRadius::same(s::RADIUS_PANEL as u8), c(s::FIELD_OUTSIDE));
    for k in 0..((X1 - X0) as i32 / 5 + 1) {
        let x = X0 + k as f32 * 5.0;
        let col = if k % 2 == 0 { s::GRASS_A } else { s::GRASS_B };
        let r = egui::Rect::from_min_max(to(x, WID), to((x + 5.0).min(X1), 0.0));
        p.rect_filled(r, 0.0, c(col));
    }
    p.line_segment([to(0.0, 0.0), to(0.0, WID)], Stroke::new(2.5, c(s::LOS)));
    p.text(to(-0.4, WID - 1.0), Align2::RIGHT_TOP, "OFFENSE", theme::small(), Color32::WHITE);
    p.text(to(0.4, WID - 1.0), Align2::LEFT_TOP, "DEFENSE", theme::small(), Color32::WHITE);

    let team_col = teams::primary(abbr);
    let r = (sx * 0.9).clamp(14.0, 22.0).max(theme::sizes().touch_target / 2.0);
    let mut clicked = None;
    for slot in offense.iter().chain(defense) {
        let at = to(slot.dx, slot.y);
        let hit = egui::Rect::from_center_size(at, egui::vec2(r * 2.4, r * 2.4));
        let resp = ui.interact(hit, ui.id().with(("slot", slot.label, slot.dx.to_bits(), slot.y.to_bits())), Sense::click());
        let Some(pl) = &slot.player else {
            p.circle_stroke(at, r, Stroke::new(1.5, theme::muted()));
            p.text(at, Align2::CENTER_CENTER, slot.label, theme::small(), theme::muted());
            continue;
        };
        if resp.hovered() {
            p.circle_stroke(at, r + 3.0, Stroke::new(3.0, c(s::RED)));
        }
        p.circle_filled(at, r, c(team_col));
        p.circle_stroke(at, r, Stroke::new(1.5, Color32::WHITE));
        let num = pl.jersey.map_or(slot.label.to_string(), |j| j.to_string());
        p.text(at, Align2::CENTER_CENTER, num, egui::FontId::proportional(r * 0.9), theme::on(team_col));
        let last = pl.name.rsplit(' ').next().unwrap_or(&pl.name);
        p.text(at + egui::vec2(0.0, r + 2.0), Align2::CENTER_TOP, format!("{} {last}", slot.label), theme::small(), Color32::WHITE);
        if resp.on_hover_text(format!("{} · {} · {} snaps here", pl.name, pl.position, slot.snaps)).clicked() {
            clicked = Some(pl.nfl_id);
        }
    }
    clicked
}
