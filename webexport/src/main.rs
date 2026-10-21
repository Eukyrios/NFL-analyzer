//! `cargo run -p webexport --release -- [data/nfl.sqlite] [out_dir]`
//!
//! Writes JS data files (`meta.js`, `team_<ABBR>.js`, `game_<ID>.js`) that the
//! web app loads with <script> tags. Everything is computed by `nfl_core`, so
//! the web app shows exactly the numbers the native app does.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nfl_core::dal::{Db, Play};
use nfl_core::roster::position_group;
use nfl_core::style::{self, Rgb};
use nfl_core::{labels, teams, usmap};
use serde_json::{json, Value};

type R<T> = nfl_core::Result<T>;

fn hex((r, g, b): Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn b64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn game_json(g: &nfl_core::dal::Game) -> Value {
    json!({
        "id": g.game_id, "week": g.week, "home": g.home, "away": g.away,
        "when": g.kickoff_label(), "neutral": g.is_neutral(), "stadium": g.stadium,
        "hs": g.home_score, "as": g.away_score, "ot": g.overtime,
    })
}

fn play_json(p: &Play) -> Value {
    let code = p.pass_result.clone().unwrap_or_default();
    json!({
        "id": p.play_id, "q": p.quarter, "qL": labels::quarter(p.quarter), "clock": p.game_clock,
        "off": p.offense, "def": p.defense, "hs": p.home_score, "as": p.away_score,
        "code": code, "res": labels::pass_result(&code), "col": hex(style::result_color(&code)),
        "yds": p.play_result, "form": p.formation.as_deref().map(labels::title_case),
        "pers": p.personnel_o, "box": p.defenders_in_box,
        "cov": p.coverage, "covType": p.coverage_type,
        "drop": p.dropback_type.as_deref().map(labels::title_case), "pa": p.play_action,
        "ttt": p.time_to_throw, "desc": p.description, "sit": p.situation(),
        "los": p.los_x(), "fd": p.first_down().map(|(x, _)| x),
    })
}

fn export_game(db: &Db, g: &nfl_core::dal::Game, out: &PathBuf) -> R<u64> {
    let plays = db.plays_for_game(g.game_id)?;
    let mut trk = serde_json::Map::new();
    let mut info = serde_json::Map::new();
    for p in &plays {
        let Some(t) = db.tracking(g.game_id, p.play_id)? else { continue };
        let slots = t.entities.len();
        let mut raw: Vec<u8> = Vec::with_capacity(t.frames.len() * slots * 6);
        let q = |v: f32, s: f32| -> u16 { (v * s).round().clamp(0.0, 65534.0) as u16 };
        for f in &t.frames {
            for s in 0..slots {
                let v = match f.get(s).copied().flatten() {
                    Some(e) => [q(e.x, 100.0), q(e.y, 100.0), e.o.map_or(0xFFFF, |o| q(o.rem_euclid(360.0), 10.0) % 3600)],
                    None => [0xFFFF; 3],
                };
                for x in v {
                    raw.extend_from_slice(&x.to_le_bytes());
                }
            }
        }
        let mut ents = Vec::new();
        for e in &t.entities {
            if let Some(id) = e.nfl_id {
                if !info.contains_key(&id.to_string()) {
                    if let Some(pl) = db.player(id)? {
                        info.insert(id.to_string(), json!([pl.name, pl.position.unwrap_or_default()]));
                    }
                }
            }
            ents.push(json!([e.nfl_id, e.jersey, e.team, e.role]));
        }
        trk.insert(
            p.play_id.to_string(),
            json!({
                "ents": ents, "n": t.frames.len(), "snap": t.summary.snap_frame, "throw": t.summary.throw_frame,
                "sack": t.summary.sack_frame, "events": t.events, "b64": b64(&raw),
            }),
        );
    }
    let body = json!({ "plays": plays.iter().map(play_json).collect::<Vec<_>>(), "trk": trk, "info": info });
    let s = format!("(window.GAMES=window.GAMES||{{}})[{}]={};", g.game_id, body);
    fs::write(out.join(format!("game_{}.js", g.game_id)), &s)?;
    Ok(s.len() as u64)
}

fn player_json(db: &Db, id: i64) -> R<Option<Value>> {
    let Some(ps) = db.player_stats(id)? else { return Ok(None) };
    let games = db.player_games(id)?;
    let p = &ps.player;
    let pos = p.position.clone().unwrap_or_default();
    Ok(Some(json!({
        "id": id, "name": p.name, "pos": pos, "posL": labels::position(&pos), "role": ps.role.pff(), "roleL": ps.role.label(),
        "roleCol": hex(style::role_color(ps.role.pff())),
        "height": p.height, "weight": p.weight, "college": p.college, "age": p.age(), "team": ps.team,
        "snaps": ps.snaps, "numbers": ps.numbers,
        "breakdowns": ps.breakdowns.iter().map(|b| json!({
            "t": b.title, "bars": b.bars.iter().map(|x| json!([x.label, x.value, hex(x.color)])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "trendTitle": ps.trend_title, "trend": ps.trend,
        "games": games.iter().map(|g| json!({
            "gid": g.game.game_id, "week": g.game.week, "opp": g.opponent, "team": g.team,
            "at": if g.game.is_neutral() || g.game.home == g.team { "vs" } else { "@" },
            "snaps": g.snaps, "headline": g.headline,
        })).collect::<Vec<_>>(),
    })))
}

fn main() -> R<()> {
    let mut args = std::env::args().skip(1);
    let db_path = PathBuf::from(args.next().unwrap_or_else(|| "data/nfl.sqlite".into()));
    let out = PathBuf::from(args.next().unwrap_or_else(|| "web/data".into()));
    fs::create_dir_all(&out)?;
    let db = Db::open_ro(&db_path)?;
    let games = db.games()?;

    // meta.js
    let team_list: Vec<Value> = teams::TEAMS
        .iter()
        .map(|t| {
            let (city, lat, lon) = usmap::stadium(t.abbr).unwrap_or(("", 0.0, 0.0));
            json!({ "a": t.abbr, "n": t.name, "c": hex(t.primary), "s": hex(t.secondary), "city": city, "lat": lat, "lon": lon,
                    "state": usmap::state_of(t.abbr) })
        })
        .collect();
    let states: serde_json::Map<String, Value> = teams::TEAMS
        .iter()
        .filter_map(|t| usmap::state_of(t.abbr))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|c| (c.to_string(), json!(usmap::state_name(c))))
        .collect();
    let meta = json!({
        "teams": team_list, "states": states,
        "outline": usmap::OUTLINE.iter().map(|&(a, b)| json!([a, b])).collect::<Vec<_>>(),
        "games": games.iter().map(game_json).collect::<Vec<_>>(),
    });
    fs::write(out.join("meta.js"), format!("window.META={meta};"))?;

    // team_<ABBR>.js
    for t in teams::TEAMS.iter() {
        let roster = db.team_roster(t.abbr)?;
        let tg = db.team_games(t.abbr)?;
        let mut stats = serde_json::Map::new();
        for r in &roster {
            if let Some(v) = player_json(&db, r.nfl_id)? {
                stats.insert(r.nfl_id.to_string(), v);
            }
        }
        let body = json!({
            "roster": roster.iter().map(|r| {
                let (unit, group, order) = position_group(&r.position);
                json!({ "id": r.nfl_id, "name": r.name, "pos": r.position, "posL": labels::position(&r.position), "jersey": r.jersey,
                        "snaps": r.snaps, "role": r.role, "roleL": labels::role(&r.role), "roleCol": hex(style::role_color(&r.role)),
                        "unit": unit, "group": group, "order": order })
            }).collect::<Vec<_>>(),
            "games": tg.iter().map(|g| json!({
                "id": g.game.game_id, "week": g.game.week, "opp": g.opponent, "label": g.opponent_label(), "when": g.game.kickoff_label(),
                "neutral": g.game.is_neutral(), "stadium": g.game.stadium, "res": g.result_label(), "po": g.plays_off, "pd": g.plays_def,
                "score": g.score,
            })).collect::<Vec<_>>(),
            "stats": stats,
        });
        fs::write(out.join(format!("team_{}.js", t.abbr)), format!("(window.TEAMDATA=window.TEAMDATA||{{}})[\"{}\"]={body};", t.abbr))?;
        println!("team {}  ({} players)", t.abbr, roster.len());
    }

    // game_<ID>.js
    let mut total = 0;
    for g in &games {
        total += export_game(&db, g, &out)?;
    }
    println!("games: {} files, {:.1} MB of JS", games.len(), total as f64 / 1e6);
    Ok(())
}
