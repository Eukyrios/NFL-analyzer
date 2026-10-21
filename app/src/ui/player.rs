//! Player detail: header, role numbers, bar charts, weekly trend, game log.

use egui::{Color32, RichText};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoint, PlotPoints, Points, Text};
use nfl_core::labels;
use nfl_core::stats::Breakdown;
use nfl_core::style as s;

use super::widgets::{card, row_rect, stat};
use crate::logos::Logos;
use crate::state::AppState;
use crate::theme::{self, c};

pub fn show(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let Some(pd) = &st.player else {
            ui.label("No stats for this player.");
            return;
        };
        let ps = &pd.stats;
        let pl = &ps.player;
        let team = ps.team.clone().unwrap_or_default();
        let mut open_team = false;
        let mut open_game = None;
        ui.horizontal(|ui| {
            logos.widget(ui, &team, 64.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&pl.name).font(egui::FontId::proportional(theme::sizes().heading + 6.0)).strong());
                let pos = pl.position.clone().unwrap_or_default();
                let tname = nfl_core::teams::team(&team).map_or(team.clone(), |t| t.name.to_string());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{pos} · {} · {}", labels::position(&pos), ps.role.label())).color(theme::muted()));
                    if ui.link(tname).clicked() {
                        open_team = true;
                    }
                });
                let bits: Vec<String> = [
                    pl.height.clone(),
                    pl.weight.map(|w| format!("{w} lb")),
                    pl.college.clone(),
                    pl.age().map(|a| format!("age {a}")),
                ]
                .into_iter()
                .flatten()
                .collect();
                ui.label(RichText::new(bits.join(" · ")).color(theme::muted()).font(theme::small()));
            });
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for (k, v) in &ps.numbers {
                stat(ui, k, v);
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let w = ((ui.available_width() - 24.0) / 2.0).max(300.0);
            ui.horizontal_wrapped(|ui| {
                for (i, b) in ps.breakdowns.iter().enumerate() {
                    ui.allocate_ui(egui::vec2(w, 240.0), |ui| card(ui, |ui| bars(ui, i, b, w - 24.0)));
                }
                ui.allocate_ui(egui::vec2(w, 240.0), |ui| {
                    card(ui, |ui| {
                        ui.label(RichText::new(&ps.trend_title).strong());
                        trend(ui, &ps.trend, w - 24.0);
                    })
                });
            });
            ui.add_space(8.0);
            ui.label(RichText::new("Games").font(theme::heading()).strong());
            for g in &pd.games {
                let (rect, resp) = row_rect(ui, 44.0, false);
                let p = ui.painter();
                let y = rect.center().y;
                p.text(egui::pos2(rect.left() + 12.0, y), egui::Align2::LEFT_CENTER, format!("Week {}", g.game.week), theme::body(), Color32::WHITE);
                logos.paint(p, egui::Rect::from_center_size(egui::pos2(rect.left() + 110.0, y), egui::vec2(28.0, 28.0)), &g.opponent, 1.0);
                let at = if g.game.is_neutral() { "vs" } else if g.game.home == g.team { "vs" } else { "@" };
                p.text(egui::pos2(rect.left() + 130.0, y), egui::Align2::LEFT_CENTER, format!("{at} {}", g.opponent), theme::body(), Color32::WHITE);
                p.text(egui::pos2(rect.left() + 220.0, y), egui::Align2::LEFT_CENTER, format!("{} snaps", g.snaps), theme::mono(), theme::muted());
                p.text(egui::pos2(rect.left() + 340.0, y), egui::Align2::LEFT_CENTER, &g.headline, theme::body(), Color32::WHITE);
                p.text(egui::pos2(rect.right() - 12.0, y), egui::Align2::RIGHT_CENTER, "plays ▶", theme::small(), theme::muted());
                if resp.clicked() {
                    open_game = Some((g.game.game_id, g.team.clone()));
                }
                ui.add_space(4.0);
            }
        });
        let id = pl.nfl_id;
        if open_team && !team.is_empty() {
            st.open_team(&team);
        } else if let Some((gid, t)) = open_game {
            st.show_game_plays(gid, Some(&t), Some(id));
        }
    });
}

fn bars(ui: &mut egui::Ui, i: usize, b: &Breakdown, w: f32) {
    ui.label(RichText::new(&b.title).strong());
    let n = b.bars.len();
    let max = b.bars.iter().map(|x| x.value).fold(1.0, f64::max);
    let total: f64 = b.bars.iter().map(|x| x.value).sum();
    let items: Vec<Bar> = b
        .bars
        .iter()
        .enumerate()
        .map(|(k, x)| Bar::new(-(k as f64), x.value).width(0.7).fill(theme::c(x.color)).name(&x.label))
        .collect();
    let labels: Vec<(f64, f64, String)> = b
        .bars
        .iter()
        .enumerate()
        .map(|(k, x)| {
            let pct = if total > 0.0 { format!(" · {:.0}%", x.value / total * 100.0) } else { String::new() };
            (x.value, -(k as f64), format!("{}  {}{pct}", x.label, labels::thousands(x.value as u64)))
        })
        .collect();
    Plot::new(format!("bars{i}"))
        .width(w)
        .height((n as f32 * 26.0 + 20.0).clamp(80.0, 200.0))
        .show_axes([false, false])
        .show_grid([false, false])
        .show_background(false)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show_x(false)
        .show_y(false)
        .include_x(0.0)
        .include_x(max * 2.2)
        .show(ui, |pu| {
            pu.bar_chart(BarChart::new("bars", items).horizontal());
            for (x, y, t) in labels {
                pu.text(Text::new("label", PlotPoint::new(x + max * 0.03, y), RichText::new(t).color(Color32::WHITE)).anchor(egui::Align2::LEFT_CENTER));
            }
        });
}

fn trend(ui: &mut egui::Ui, pts: &[(f64, f64)], w: f32) {
    if pts.is_empty() {
        ui.label(RichText::new("No weekly data").color(theme::muted()));
        return;
    }
    let data: Vec<[f64; 2]> = pts.iter().map(|&(x, y)| [x, y]).collect();
    Plot::new("trend")
        .width(w)
        .height(180.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(0.5)
        .include_x(8.5)
        .include_y(0.0)
        .show_background(false)
        .x_axis_formatter(|m, _| if m.value.fract() == 0.0 && (1.0..=8.0).contains(&m.value) { format!("W{}", m.value) } else { String::new() })
        .show(ui, |pu| {
            pu.line(Line::new("trend", PlotPoints::from(data.clone())).color(c(s::RED)).width(2.0));
            pu.points(Points::new("pts", PlotPoints::from(data)).color(c(s::RED)).radius(4.0));
        });
}
