//! Tracking CSVs → `play_entities`, packed `play_frames`, deduped
//! `play_events`, `play_summary` and `player_play_stats`. Plays are normalized
//! so the offense always moves left → right.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use nfl_core::blob::{self, EntityPos, Frame};
use nfl_core::geometry::normalize;
use rusqlite::{params, Connection};

use crate::Result;

#[derive(Debug, Clone)]
pub struct Row {
    pub game_id: i64,
    pub play_id: i64,
    pub nfl_id: Option<i64>,
    pub frame: i64,
    pub jersey: Option<i64>,
    pub team: String,
    pub left: bool,
    pub pos: EntityPos,
    pub dis: f32,
    pub event: Option<String>,
}

fn opt<T: std::str::FromStr>(v: &str) -> Option<T> {
    if matches!(v, "NA" | "" | "None") { None } else { v.parse().ok() }
}

pub fn read_file(path: &Path) -> Result<Vec<Row>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let h = rdr.headers()?.clone();
    let col = |n: &str| h.iter().position(|c| c == n).ok_or_else(|| format!("{}: no {n} column", path.display()));
    let c = [
        col("gameId")?, col("playId")?, col("nflId")?, col("frameId")?, col("jerseyNumber")?, col("team")?,
        col("playDirection")?, col("x")?, col("y")?, col("s")?, col("dis")?, col("o")?, col("dir")?, col("event")?,
    ];
    let mut out = Vec::new();
    let mut rec = csv::StringRecord::new();
    while rdr.read_record(&mut rec)? {
        let num = |i: usize| -> Result<f32> {
            opt(&rec[c[i]]).ok_or_else(|| format!("{}: bad number {:?}", path.display(), &rec[c[i]]).into())
        };
        out.push(Row {
            game_id: opt(&rec[c[0]]).ok_or("bad gameId")?,
            play_id: opt(&rec[c[1]]).ok_or("bad playId")?,
            nfl_id: opt(&rec[c[2]]),
            frame: opt(&rec[c[3]]).ok_or("bad frameId")?,
            jersey: opt::<f64>(&rec[c[4]]).map(|j| j as i64),
            team: rec[c[5]].to_string(),
            left: &rec[c[6]] == "left",
            pos: EntityPos { x: num(7)?, y: num(8)?, s: num(9)?, o: opt(&rec[c[11]]), dir: opt(&rec[c[12]]) },
            dis: opt(&rec[c[10]]).unwrap_or(0.0),
            event: opt::<String>(&rec[c[13]]),
        });
    }
    Ok(out)
}

/// Manual tags win over auto tags; auto tags are renamed to the manual name.
pub fn dedup_events(raw: &BTreeSet<(i64, String)>) -> Vec<(i64, String)> {
    const AUTO: [(&str, &str); 2] = [("autoevent_ballsnap", "ball_snap"), ("autoevent_passforward", "pass_forward")];
    let has = |name: &str| raw.iter().any(|(_, e)| e == name);
    let mut out = BTreeSet::new();
    for (f, e) in raw {
        match AUTO.iter().find(|(a, _)| a == e) {
            Some((_, manual)) if has(manual) => {}
            Some((_, manual)) => {
                out.insert((*f, manual.to_string()));
            }
            None => {
                out.insert((*f, e.clone()));
            }
        }
    }
    out.into_iter().collect()
}

pub struct PlayOut {
    pub game_id: i64,
    pub play_id: i64,
    /// (nflId, jersey, team) per slot.
    pub entities: Vec<(Option<i64>, Option<i64>, String)>,
    pub frames: Vec<Frame>,
    pub events: Vec<(i64, String)>,
    pub snap: i64,
    pub throw: Option<i64>,
    pub sack: Option<i64>,
    pub end: i64,
    /// (nflId, max speed, avg speed, distance) from the snap on.
    pub stats: Vec<(i64, f32, f32, f32)>,
}

pub fn build_play(rows: &[Row]) -> Option<PlayOut> {
    let first = rows.first()?;
    // Slots: players by team then jersey, ball last.
    let mut ents: Vec<(Option<i64>, Option<i64>, String)> = rows
        .iter()
        .map(|r| (r.nfl_id, r.jersey, r.team.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    ents.sort_by(|a, b| (a.0.is_none(), &a.2, a.1).cmp(&(b.0.is_none(), &b.2, b.1)));
    ents.dedup_by(|a, b| a.0 == b.0);
    let slot: HashMap<Option<i64>, usize> = ents.iter().enumerate().map(|(i, e)| (e.0, i)).collect();
    let n_frames = rows.iter().map(|r| r.frame).max().unwrap_or(0).max(1);
    let mut frames: Vec<Frame> = vec![vec![None; ents.len()]; n_frames as usize];
    let mut raw_events = BTreeSet::new();
    for r in rows {
        let p = if r.left { normalize(r.pos) } else { r.pos };
        if let (Some(&s), Some(f)) = (slot.get(&r.nfl_id), frames.get_mut((r.frame - 1).max(0) as usize)) {
            f[s] = Some(p);
        }
        if let Some(e) = &r.event {
            raw_events.insert((r.frame, e.clone()));
        }
    }
    let events = dedup_events(&raw_events);
    let find = |names: &[&str]| events.iter().find(|(_, e)| names.contains(&e.as_str())).map(|(f, _)| *f);
    let snap = find(&["ball_snap"]).unwrap_or(1);
    let mut acc: BTreeMap<i64, (f32, f32, u32, f32)> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.frame >= snap) {
        if let Some(id) = r.nfl_id {
            let e = acc.entry(id).or_insert((0.0, 0.0, 0, 0.0));
            e.0 = e.0.max(r.pos.s);
            e.1 += r.pos.s;
            e.2 += 1;
            e.3 += r.dis;
        }
    }
    Some(PlayOut {
        game_id: first.game_id,
        play_id: first.play_id,
        entities: ents,
        frames,
        snap,
        throw: find(&["pass_forward"]),
        sack: find(&["qb_sack", "qb_strip_sack"]),
        end: n_frames,
        events,
        stats: acc.into_iter().map(|(id, (mx, sum, n, d))| (id, mx, sum / n.max(1) as f32, d)).collect(),
    })
}

pub fn insert_play(conn: &Connection, p: &PlayOut) -> Result<()> {
    let (g, pl) = (p.game_id, p.play_id);
    let mut s = conn.prepare_cached("INSERT INTO play_entities VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (i, (id, j, t)) in p.entities.iter().enumerate() {
        s.execute(params![g, pl, i as i64, id, j, t])?;
    }
    conn.prepare_cached("INSERT INTO play_frames VALUES (?1, ?2, ?3, ?4)")?
        .execute(params![g, pl, p.frames.len() as i64, blob::encode(&p.frames, p.entities.len())])?;
    let mut s = conn.prepare_cached("INSERT INTO play_events VALUES (?1, ?2, ?3, ?4)")?;
    for (f, e) in &p.events {
        s.execute(params![g, pl, f, e])?;
    }
    let ttt = p.throw.map(|t| (t - p.snap) as f64 / 10.0);
    conn.prepare_cached("INSERT INTO play_summary VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
        .execute(params![g, pl, p.snap, p.throw, p.sack, p.end, ttt])?;
    let mut s = conn.prepare_cached("INSERT INTO player_play_stats VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (id, mx, avg, d) in &p.stats {
        s.execute(params![g, pl, id, *mx as f64, *avg as f64, *d as f64])?;
    }
    Ok(())
}

pub fn load_file(conn: &mut Connection, path: &Path) -> Result<usize> {
    let rows = read_file(path)?;
    let mut by_play: BTreeMap<(i64, i64), Vec<Row>> = BTreeMap::new();
    for r in rows {
        by_play.entry((r.game_id, r.play_id)).or_default().push(r);
    }
    let tx = conn.transaction()?;
    for rows in by_play.values() {
        if let Some(p) = build_play(rows) {
            insert_play(&tx, &p)?;
        }
    }
    tx.commit()?;
    Ok(by_play.len())
}

pub fn load_dir(conn: &mut Connection, dir: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    files.sort();
    let start = Instant::now();
    println!("  tracking: {} files", files.len());
    for (i, f) in files.iter().enumerate() {
        let t = Instant::now();
        let n = load_file(conn, f)?;
        println!(
            "    [{:>3}/{}] {} {n} plays  {:.2}s",
            i + 1,
            files.len(),
            f.file_name().map(|s| s.to_string_lossy()).unwrap_or_default(),
            t.elapsed().as_secs_f64()
        );
    }
    println!("  tracking      {:.1}s", start.elapsed().as_secs_f64());
    Ok(())
}
