//! Animated play viewer: header, field, timeline, details.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind};
use nfl_core::blob::Frame;
use nfl_core::geometry::{angle_vec, frame_at, Playback, Viewport, HASHES, LEN, WID};
use nfl_core::labels;
use nfl_core::style::{self as s, Rgb};
use nfl_core::teams::primary;

use super::widgets::{chip, result_text};
use crate::logos::Logos;
use crate::state::{AppState, ViewerData};
use crate::theme::{self, c, ca};

pub fn show(ctx: &egui::Context, st: &mut AppState, logos: &Logos) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if st.viewer.is_none() {
            ui.label("Loading…");
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        header(ui, st, logos);
        let avail = ui.available_size();
        let field_h = (avail.y - 430.0).max(300.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail.x, field_h), Sense::click());
        let portrait = rect.height() > rect.width() * 1.2;
        let vp = Viewport::fit(rect.left(), rect.top(), rect.width(), rect.height(), portrait);
        let r = (0.95 * vp.scale()).max(6.0);
        let hit_r = r + if theme::IS_PHONE { 16.0 } else { 6.0 };
        let hover = resp.hover_pos().and_then(|p| st.player_at(&vp, (p.x, p.y), hit_r));
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if st.tap(&vp, (p.x, p.y), hit_r).is_some() {
                    return;
                }
            }
        }
        let Some(v) = &st.viewer else { return };
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 8.0, c(s::FIELD_OUTSIDE));
        draw_field(&p, &vp, v, logos);
        draw_entities(&p, &vp, v, st.trails, st.color_by_role, hover, r);
        if let Some(id) = hover {
            if let Some(info) = v.info.get(&id) {
                let role = v.tracking.as_ref().and_then(|t| t.entities.iter().find(|e| e.nfl_id == Some(id))).and_then(|e| e.role.clone());
                resp.clone().on_hover_text(format!("{} · {}\n{}\nTap for stats", info.name, info.position, role.as_deref().map(labels::role).unwrap_or("")));
            }
        }
        timeline(ui, st);
        details(ui, st);
        game_timeline(ui, st, logos);
        });
    });
}

fn header(ui: &mut egui::Ui, st: &AppState, logos: &Logos) {
    let Some(v) = &st.viewer else { return };
    let (g, pl) = (&v.game, &v.play);
    let (first, second, sep) = if g.is_neutral() { (&g.home, &g.away, "vs") } else { (&g.away, &g.home, "@") };
    let score = |t: &str| if t == g.home { pl.home_score } else { pl.away_score };
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            ui.horizontal(|ui| {
                logos.widget(ui, first, 36.0);
                ui.label(RichText::new(format!("{first} {sep} {second}")).font(theme::heading()).strong());
                logos.widget(ui, second, 36.0);
                if g.is_neutral() {
                    chip(ui, &format!("Neutral · {}", g.stadium.clone().unwrap_or_default()), None);
                }
            });
            ui.label(
                RichText::new(format!("Week {} · {} · {} dropback plays", g.week, g.kickoff_label(), st.plays.len()))
                    .color(theme::muted())
                    .font(theme::small()),
            );
        });
        cols[1].with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            // Right to left: second team, situation, first team.
            score_card(ui, logos, second, score(second), pl.offense == *second);
            situation_card(ui, pl);
            score_card(ui, logos, first, score(first), pl.offense == *first);
        });
    });
    ui.add_space(8.0);
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(c(s::SURFACE))
        .stroke(Stroke::new(1.0, c(s::BORDER)))
        .corner_radius(egui::CornerRadius::same(s::RADIUS_PANEL as u8))
        .inner_margin(egui::Margin::symmetric(12, 8))
}

fn score_card(ui: &mut egui::Ui, logos: &Logos, team: &str, score: i64, has_ball: bool) {
    card_frame().show(ui, |ui| {
        ui.allocate_ui_with_layout(egui::vec2(140.0, 34.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            logos.widget(ui, team, 28.0);
            ui.label(RichText::new(team).strong());
            if has_ball {
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, c(s::LOS));
            }
            ui.label(RichText::new(score.to_string()).font(egui::FontId::proportional(theme::sizes().heading + 4.0)).strong());
        });
    });
}

fn situation_card(ui: &mut egui::Ui, pl: &nfl_core::dal::Play) {
    card_frame().show(ui, |ui| {
        ui.allocate_ui_with_layout(egui::vec2(190.0, 34.0), egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{} {}", labels::quarter(pl.quarter), pl.game_clock)).font(theme::mono()).strong());
            ui.label(RichText::new(format!("{} · {}", pl.offense, pl.situation())).color(theme::muted()).font(theme::small()));
        });
    });
}

fn line(p: &Painter, vp: &Viewport, a: (f32, f32), b: (f32, f32), st: Stroke) {
    let (ax, ay) = vp.to_screen(a.0, a.1);
    let (bx, by) = vp.to_screen(b.0, b.1);
    p.line_segment([Pos2::new(ax, ay), Pos2::new(bx, by)], st);
}

fn field_rect(vp: &Viewport, x0: f32, x1: f32, y0: f32, y1: f32) -> Rect {
    let (a, b) = (vp.to_screen(x0, y0), vp.to_screen(x1, y1));
    Rect::from_two_pos(Pos2::new(a.0, a.1), Pos2::new(b.0, b.1))
}

/// Text centered at `pos`, rotated by `angle` radians.
fn rotated_text(p: &Painter, pos: Pos2, text: &str, font: FontId, color: Color32, angle: f32) {
    let g = p.layout_no_wrap(text.to_string(), font, color);
    let half = g.size() / 2.0;
    let (sn, cs) = angle.sin_cos();
    let off = egui::vec2(half.x * cs - half.y * sn, half.x * sn + half.y * cs);
    p.add(egui::epaint::TextShape::new(pos - off, g, color).with_angle(angle));
}

fn draw_field(p: &Painter, vp: &Viewport, v: &ViewerData, logos: &Logos) {
    let sc = vp.scale();
    let rot = if vp.portrait { -std::f32::consts::FRAC_PI_2 } else { 0.0 };
    for i in 0..20 {
        let x = 10.0 + i as f32 * 5.0;
        p.rect_filled(field_rect(vp, x, x + 5.0, 0.0, WID), 0.0, c(if i % 2 == 0 { s::GRASS_A } else { s::GRASS_B }));
    }
    // End zones: normalized left → right, so left = offense, right = defense.
    let pl = &v.play;
    for (x0, team, a) in [(0.0, &pl.offense, -std::f32::consts::FRAC_PI_2), (110.0, &pl.defense, std::f32::consts::FRAC_PI_2)] {
        let col = primary(team);
        p.rect_filled(field_rect(vp, x0, x0 + 10.0, 0.0, WID), 0.0, c(col));
        let (cx, cy) = vp.to_screen(x0 + 5.0, WID / 2.0);
        let center = Pos2::new(cx, cy);
        let has = logos.has(team);
        if has {
            p.circle_filled(center, 4.65 * sc, ca(s::ENDZONE_BADGE));
            logos.paint(p, Rect::from_center_size(center, egui::vec2(7.5 * sc, 7.5 * sc)), team, 1.0);
        }
        let txt = if nfl_core::geometry::is_light(col) { Color32::from_black_alpha(180) } else { Color32::from_white_alpha(230) };
        let ys: &[f32] = if has { &[WID / 2.0 - 15.0, WID / 2.0 + 15.0] } else { &[WID / 2.0] };
        for &y in ys {
            let (tx, ty) = vp.to_screen(x0 + 5.0, y);
            rotated_text(p, Pos2::new(tx, ty), team, FontId::proportional((if has { 3.0 } else { 4.2 }) * sc), txt, a + rot);
        }
    }
    if !v.game.is_neutral() {
        let (cx, cy) = vp.to_screen(60.0, WID / 2.0);
        logos.paint(p, Rect::from_center_size(Pos2::new(cx, cy), egui::vec2(11.0 * sc, 11.0 * sc)), v.home(), 0.9);
    }
    let thin = Stroke::new(1.0, ca(s::LINE));
    for i in 0..=18 {
        let x = 15.0 + i as f32 * 5.0;
        line(p, vp, (x, 0.0), (x, WID), thin);
    }
    for x in 11..110 {
        if x % 5 == 0 {
            continue;
        }
        for (k, y) in HASHES.iter().enumerate() {
            let y0 = if k == 3 { y - 0.67 } else if k == 0 { *y } else { y - 0.33 };
            line(p, vp, (x as f32, y0), (x as f32, y0 + 0.67), thin);
        }
    }
    let thick = Stroke::new(2.0, ca(s::LINE));
    line(p, vp, (10.0, 0.0), (10.0, WID), thick);
    line(p, vp, (110.0, 0.0), (110.0, WID), thick);
    p.rect_stroke(field_rect(vp, 0.0, LEN, 0.0, WID), 0.0, thick, StrokeKind::Middle);
    for k in 1..=9 {
        let x = 10.0 + 10.0 * k as f32;
        let n = if k <= 5 { 10 * k } else { 100 - 10 * k };
        let f = FontId::proportional(2.2 * sc);
        let (ax, ay) = vp.to_screen(x, 12.0);
        rotated_text(p, Pos2::new(ax, ay), &n.to_string(), f.clone(), ca(s::YARD_NUMBER), rot);
        let (bx, by) = vp.to_screen(x, WID - 12.0);
        rotated_text(p, Pos2::new(bx, by), &n.to_string(), f, ca(s::YARD_NUMBER), rot + std::f32::consts::PI);
    }
    let w = (0.3 * sc).max(2.0);
    if let Some(los) = pl.los_x() {
        line(p, vp, (los, 0.0), (los, WID), Stroke::new(w, c(s::LOS)));
    }
    if let Some((fd, _)) = pl.first_down() {
        line(p, vp, (fd, 0.0), (fd, WID), Stroke::new(w, c(s::FIRST_DOWN)));
    }
}

struct Look {
    fill: Color32,
    stroke: Color32,
    text: Color32,
    trail: Color32,
}

fn look(team: &str, home: bool, role: Option<&str>, by_role: bool) -> Look {
    if by_role {
        let col = role.map_or(s::UNKNOWN_ROLE, s::role_color);
        return Look { fill: c(col), stroke: c(s::ROLE_STROKE), text: c(s::ROLE_STROKE), trail: c(col) };
    }
    let col: Rgb = primary(team);
    let light = nfl_core::geometry::is_light(col);
    if home {
        Look { fill: c(col), stroke: Color32::WHITE, text: theme::on(col), trail: c(col) }
    } else {
        let trim = if light { c(s::LIGHT_STROKE) } else { c(col) };
        Look { fill: Color32::WHITE, stroke: trim, text: if light { c(s::DARK_TEXT) } else { c(col) }, trail: trim }
    }
}

fn draw_entities(p: &Painter, vp: &Viewport, v: &ViewerData, trails: bool, by_role: bool, hover: Option<i64>, r: f32) {
    let Some(t) = &v.tracking else {
        p.text(p.clip_rect().center(), Align2::CENTER_CENTER, "No tracking for this play", theme::body(), Color32::WHITE);
        return;
    };
    let frame: Frame = frame_at(&t.frames, v.playback.frame);
    let scr = |x: f32, y: f32| {
        let (a, b) = vp.to_screen(x, y);
        Pos2::new(a, b)
    };
    if trails {
        let last = v.playback.frame.floor() as usize;
        for (i, e) in t.entities.iter().enumerate() {
            let col = if e.is_ball() { c(s::BALL_TRAIL) } else { look(&e.team, e.team == v.home(), e.role.as_deref(), by_role).trail };
            let mut pts: Vec<Pos2> = t.frames.iter().take(last + 1).filter_map(|f| f.get(i).copied().flatten()).map(|q| scr(q.x, q.y)).collect();
            if let Some(q) = frame.get(i).copied().flatten() {
                pts.push(scr(q.x, q.y));
            }
            if pts.len() > 1 {
                p.add(Shape::line(pts, Stroke::new(1.5, col.gamma_multiply(0.6))));
            }
        }
    }
    let font = FontId::proportional(r * 1.05);
    let mut ball = None;
    for (i, e) in t.entities.iter().enumerate() {
        let Some(q) = frame.get(i).copied().flatten() else { continue };
        if e.is_ball() {
            ball = Some((i, q));
            continue;
        }
        let pos = scr(q.x, q.y);
        let lk = look(&e.team, e.team == v.home(), e.role.as_deref(), by_role);
        if let Some(o) = q.o {
            let d = vp.dir_to_screen(angle_vec(o));
            p.line_segment([pos, pos + egui::vec2(d.0, d.1) * r * 1.7], Stroke::new(2.0, ca(s::ARROW)));
        }
        p.circle_filled(pos, r, lk.fill);
        p.circle_stroke(pos, r, Stroke::new(1.5, lk.stroke));
        if e.nfl_id.is_some() && e.nfl_id == hover {
            p.circle_stroke(pos, r + 3.0, Stroke::new(3.0, c(s::RED)));
        }
        if let (Some(j), true) = (e.jersey, r >= 6.0) {
            p.text(pos, Align2::CENTER_CENTER, j.to_string(), font.clone(), lk.text);
        }
    }
    if let Some((i, q)) = ball {
        let prev = frame_at(&t.frames, (v.playback.frame - 1.0).max(0.0)).get(i).copied().flatten();
        let pos = scr(q.x, q.y);
        let ang = prev
            .map(|pq| {
                let a = scr(pq.x, pq.y);
                let d = pos - a;
                if d.length_sq() > 0.01 { d.y.atan2(d.x) } else { 0.0 }
            })
            .unwrap_or(if vp.portrait { -std::f32::consts::FRAC_PI_2 } else { 0.0 });
        let (rx, ry) = ((0.6 * vp.scale()).max(4.0), (0.36 * vp.scale()).max(2.5));
        let (sn, cs) = ang.sin_cos();
        let pts: Vec<Pos2> = (0..20)
            .map(|k| {
                let t = k as f32 / 20.0 * std::f32::consts::TAU;
                let (ex, ey) = (rx * t.cos(), ry * t.sin());
                pos + egui::vec2(ex * cs - ey * sn, ex * sn + ey * cs)
            })
            .collect();
        p.add(Shape::convex_polygon(pts, c(s::BALL), Stroke::new(1.0, Color32::WHITE)));
    }
}

fn timeline(ui: &mut egui::Ui, st: &mut AppState) {
    let (mut st_trails, mut st_roles, mut st_auto) = (st.trails, st.color_by_role, st.auto_advance);
    let Some(v) = &mut st.viewer else { return };
    let pb: &mut Playback = &mut v.playback;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let big = egui::vec2(theme::sizes().touch_target.max(36.0), theme::sizes().touch_target.max(32.0));
        if ui.add(egui::Button::new(s::ICON_PREV).min_size(big)).on_hover_text("Previous play").clicked() {
            st_step(ui, -1);
        }
        let icon = if pb.playing { s::ICON_PAUSE } else { s::ICON_PLAY };
        if ui.add(egui::Button::new(RichText::new(icon).color(Color32::WHITE)).fill(c(s::RED)).min_size(big)).clicked() {
            pb.toggle();
        }
        if ui.add(egui::Button::new(s::ICON_NEXT).min_size(big)).on_hover_text("Next play").clicked() {
            st_step(ui, 1);
        }
        let t = pb.from_snap(v.snap);
        ui.label(RichText::new(format!("{:+.1}s from snap", t)).font(theme::mono()));
        egui::ComboBox::from_id_salt("speed").width(64.0).selected_text(format!("{}×", pb.speed)).show_ui(ui, |ui| {
            for sp in Playback::SPEEDS {
                ui.selectable_value(&mut pb.speed, sp, format!("{sp}×"));
            }
        });
        ui.checkbox(&mut st_trails, "Trails");
        ui.checkbox(&mut st_roles, "Color by role");
        ui.checkbox(&mut st_auto, "Auto-advance");
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            pb.toggle();
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            pb.step(1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            pb.step(-1);
        }
    });
    st.trails = st_trails;
    st.color_by_role = st_roles;
    st.auto_advance = st_auto;
    let Some(v) = &mut st.viewer else { return };
    let pb: &mut Playback = &mut v.playback;
    // Track
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 46.0), Sense::click_and_drag());
    let p = ui.painter();
    let track = Rect::from_min_max(egui::pos2(rect.left() + 10.0, rect.top() + 8.0), egui::pos2(rect.right() - 10.0, rect.top() + 14.0));
    let n = pb.frames.saturating_sub(1).max(1) as f32;
    let fx = |f: f32| track.left() + f / n * track.width();
    if let Some((a, b)) = v.span {
        p.rect_filled(Rect::from_min_max(egui::pos2(fx(a as f32), track.top() - 5.0), egui::pos2(fx(b as f32), track.bottom() + 5.0)), 3.0, c(s::NAVY));
    }
    p.rect_filled(track, 3.0, c(s::BORDER));
    p.rect_filled(Rect::from_min_max(track.min, egui::pos2(fx(pb.frame), track.max.y)), 3.0, c(s::RED));
    let mut last_x = f32::MIN;
    let mut row = 0;
    for m in &v.markers {
        let x = fx(m.frame as f32);
        row = if x - last_x < 44.0 { 1 - row } else { 0 };
        last_x = x;
        p.line_segment([egui::pos2(x, track.top() - 4.0), egui::pos2(x, track.bottom() + 6.0)], Stroke::new(1.5, theme::muted()));
        p.text(egui::pos2(x, track.bottom() + 8.0 + row as f32 * 13.0), Align2::CENTER_TOP, m.label, theme::small(), theme::muted());
    }
    p.circle_filled(egui::pos2(fx(pb.frame), track.center().y), 8.0, c(s::RED));
    p.circle_stroke(egui::pos2(fx(pb.frame), track.center().y), 8.0, Stroke::new(1.5, Color32::WHITE));
    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            pb.playing = false;
            pb.scrub((pos.x - track.left()) / track.width() * n);
        }
    }
}

/// Defers prev/next-play to after the borrow of the viewer ends.
fn st_step(ui: &egui::Ui, by: i32) {
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("step_play"), by));
}

fn details(ui: &mut egui::Ui, st: &mut AppState) {
    if let Some(by) = ui.ctx().data_mut(|d| d.remove_temp::<i32>(egui::Id::new("step_play"))) {
        st.step_play(by);
    }
    if let Some((g, p)) = ui.ctx().data_mut(|d| d.remove_temp::<(i64, i64)>(egui::Id::new("jump_play"))) {
        st.jump_to_play(g, p);
    }
    let Some(v) = &st.viewer else { return };
    let pl = &v.play;
    ui.horizontal_wrapped(|ui| {
        let (res, col) = result_text(pl);
        chip(ui, &res, Some(col));
        for t in [
            pl.formation.clone().map(|f| labels::title_case(&f)),
            pl.personnel_o.clone(),
            pl.defenders_in_box.map(|b| format!("{b} in box")),
            pl.coverage.clone().map(|cv| format!("{cv} · {}", pl.coverage_type.clone().unwrap_or_default())),
            pl.dropback_type.clone().map(|d| labels::title_case(&d)),
            pl.play_action.then(|| "Play action".to_string()),
            pl.time_to_throw.map(|t| format!("{t:.1} s to throw")),
        ]
        .into_iter()
        .flatten()
        {
            chip(ui, &t, None);
        }
    });
    ui.label(RichText::new(&pl.description).color(theme::muted()));
    let Some(v) = &st.viewer else { return };
    let home = v.home().to_string();
    let away = if v.game.home == v.game.away { String::new() } else { v.game.away.clone() };
    let neutral = v.game.is_neutral();
    ui.horizontal_wrapped(|ui| {
        let by_role = st.color_by_role;
        if by_role {
            for (k, col) in [("Pass", s::ROLE_PASS), ("Pass Block", s::ROLE_BLOCK), ("Pass Route", s::ROLE_ROUTE), ("Pass Rush", s::ROLE_RUSH), ("Coverage", s::ROLE_COVERAGE)] {
                chip(ui, labels::role(k), Some(col));
            }
        } else {
            let suffix = |w: &str| if neutral { String::new() } else { format!(" ({w})") };
            dot(ui, c(primary(&home)), Color32::WHITE, &format!("{home}{}", suffix("home")));
            dot(ui, Color32::WHITE, c(primary(&away)), &format!("{away}{}", suffix("away")));
        }
        dot(ui, c(s::BALL), c(s::BALL), "Ball");
        swatch(ui, c(s::LOS));
        ui.label(RichText::new("Line of scrimmage").color(theme::muted()).font(theme::small()));
        swatch(ui, c(s::FIRST_DOWN));
        ui.label(RichText::new("Line to gain").color(theme::muted()).font(theme::small()));
    });
}

fn swatch(ui: &mut egui::Ui, col: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 12.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(3.0, col));
}

fn dot(ui: &mut egui::Ui, fill: Color32, stroke: Color32, label: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 5.0, fill);
    ui.painter().circle_stroke(r.center(), 5.0, Stroke::new(2.0, stroke));
    ui.label(RichText::new(label).color(theme::muted()).font(theme::small()));
}

/// "Game timeline · dropback plays": one bar per play grouped by quarter.
/// Bars point up for the home team's possessions and down for the away
/// team's; color shows the result and height the yards gained.
fn game_timeline(ui: &mut egui::Ui, st: &AppState, logos: &Logos) {
    let Some(v) = &st.viewer else { return };
    let home = v.home().to_string();
    let away = v.game.away.clone();
    let plays = &st.plays;
    if plays.is_empty() {
        return;
    }
    let cur = (v.play.game_id, v.play.play_id);
    ui.add_space(8.0);
    let mut jump = None;
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Game timeline · dropback plays").strong());
            for (code, label) in [("C", "Complete"), ("I", "Incomplete"), ("S", "Sack"), ("IN", "Interception"), ("R", "Scramble")] {
                dot(ui, c(s::result_color(code)), c(s::result_color(code)), label);
            }
            ui.label(RichText::new("· bar height = yards gained").color(theme::muted()).font(theme::small()));
        });
        let h = 84.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
        let p = ui.painter_at(rect);
        let left = rect.left() + 34.0;
        let mid = rect.center().y + 6.0;
        logos.paint(&p, Rect::from_center_size(egui::pos2(rect.left() + 14.0, mid - 22.0), egui::vec2(20.0, 20.0)), &home, 1.0);
        logos.paint(&p, Rect::from_center_size(egui::pos2(rect.left() + 14.0, mid + 22.0), egui::vec2(20.0, 20.0)), &away, 1.0);
        p.line_segment([egui::pos2(left, mid), egui::pos2(rect.right(), mid)], Stroke::new(1.0, c(s::BORDER)));
        let gap = 8.0;
        let n = plays.len() as f32;
        let unit = (rect.right() - left - gap * 3.0) / n;
        let mut x = left;
        let mut hit: Option<usize> = None;
        let ptr = resp.hover_pos().or_else(|| resp.interact_pointer_pos());
        let mut q_start = 0;
        while q_start < plays.len() {
            let q = plays[q_start].quarter;
            let q_end = plays[q_start..].iter().position(|p| p.quarter != q).map_or(plays.len(), |k| q_start + k);
            p.text(egui::pos2(x, rect.top()), Align2::LEFT_TOP, labels::quarter(q), theme::small(), theme::muted());
            for (i, pl) in plays.iter().enumerate().take(q_end).skip(q_start) {
                let bar = Rect::from_min_size(egui::pos2(x, rect.top() + 16.0), egui::vec2((unit - 1.0).max(1.5), h - 16.0));
                let yds = pl.play_result.unwrap_or(0).unsigned_abs().min(27) as f32;
                let bh = 3.0 + yds;
                let col = pl.pass_result.as_deref().map_or(c(s::UNKNOWN_ROLE), |r| c(s::result_color(r)));
                let r = if pl.offense == home {
                    Rect::from_min_max(egui::pos2(bar.left(), mid - bh), egui::pos2(bar.right(), mid))
                } else {
                    Rect::from_min_max(egui::pos2(bar.left(), mid), egui::pos2(bar.right(), mid + bh))
                };
                if (pl.game_id, pl.play_id) == cur {
                    p.rect_filled(Rect::from_min_max(egui::pos2(bar.left() - 1.0, bar.top()), egui::pos2(bar.right() + 1.0, bar.bottom())), 2.0, theme::alpha(s::RED, 0.35));
                }
                p.rect_filled(r, 1.0, col);
                if ptr.is_some_and(|q| q.x >= x && q.x < x + unit) {
                    hit = Some(i);
                }
                x += unit;
            }
            x += gap;
            q_start = q_end;
        }
        if let Some(i) = hit {
            let pl = &plays[i];
            let (res, _) = result_text(pl);
            let tip = format!("{} {} · {} · {}\n{}", labels::quarter(pl.quarter), pl.game_clock, pl.offense, res, pl.description);
            if resp.hovered() {
                resp.clone().on_hover_text(tip);
            }
            if resp.clicked() {
                jump = Some((pl.game_id, pl.play_id));
            }
        }
    });
    if let Some(j) = jump {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("jump_play"), j));
        ui.ctx().request_repaint();
    }
}
