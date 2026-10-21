//! Reads games and plays from the SQLite database and shapes them for the viewer.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;

use rusqlite::{types::ValueRef, Connection, OptionalExtension, Row};
use serde::Serialize;

pub type Res<T> = Result<T, Box<dyn Error>>;

/// Values stored per entity per frame: x*10, y*10, orientation (deg), speed*10.
const STRIDE: usize = 4;

// SQLite columns are dynamically typed, so read them leniently.
fn int(r: &Row, i: usize) -> rusqlite::Result<Option<i64>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Integer(v) => Some(v),
        ValueRef::Real(v) => Some(v as i64),
        ValueRef::Text(t) => std::str::from_utf8(t).ok().and_then(|s| s.parse::<f64>().ok()).map(|f| f as i64),
        _ => None,
    })
}

fn real(r: &Row, i: usize) -> rusqlite::Result<Option<f64>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Integer(v) => Some(v as f64),
        ValueRef::Real(v) => Some(v),
        ValueRef::Text(t) => std::str::from_utf8(t).ok().and_then(|s| s.parse().ok()),
        _ => None,
    })
}

fn text(r: &Row, i: usize) -> rusqlite::Result<Option<String>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Integer(v) => Some(v.to_string()),
        ValueRef::Real(v) => Some(v.to_string()),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        _ => None,
    })
}

fn tenths(v: f64) -> i64 {
    (v * 10.0).round_ties_even() as i64
}

#[derive(Serialize)]
pub struct GameInfo {
    pub id: i64,
    pub week: i64,
    pub date: String,
    pub time: String,
    pub home: String,
    pub away: String,
    /// Stadium of a neutral-site game (nflverse `location` = Neutral); neither team is home.
    pub site: Option<String>,
}

impl GameInfo {
    /// "AWAY @ HOME", or "AWAY vs HOME" at a neutral site.
    pub fn matchup(&self) -> String {
        let sep = if self.site.is_some() { "vs" } else { "@" };
        format!("{} {sep} {}", self.away, self.home)
    }
}

/// A play's situation and outcome; tracking is fetched separately via [`play_tracking`].
#[derive(Serialize)]
pub struct PlaySummary {
    id: i64,
    q: i64,
    clock: String,
    down: Option<i64>,
    ytg: Option<i64>,
    pos: String,
    def: String,
    yl: String,
    desc: String,
    res: Option<String>,
    yds: Option<i64>,
    fouls: Vec<String>,
    hs: i64,
    vs: i64,
    form: Option<String>,
    #[serde(rename = "persO")]
    pers_o: Option<String>,
    #[serde(rename = "persD")]
    pers_d: Option<String>,
    #[serde(rename = "box")]
    in_box: Option<i64>,
    cov: Option<String>,
    #[serde(rename = "covType")]
    cov_type: Option<String>,
    pa: bool,
    drop: Option<String>,
}

#[derive(Serialize)]
pub struct GameSummary {
    game: GameInfo,
    plays: Vec<PlaySummary>,
}

/// [nflId, side (0 home, 1 away, 2 ball), jersey, PFF role, PFF position lined up]
type Ent = (Option<i64>, u8, Option<i64>, Option<String>, Option<String>);

#[derive(Serialize)]
pub struct PlayTracking {
    dir: &'static str,
    los: Option<f64>,
    snap: usize,
    ev: Vec<(usize, String)>,
    ents: Vec<Ent>,
    f: Vec<Vec<Option<i64>>>,
    players: BTreeMap<String, (String, String)>,
}

const GAME_COLUMNS: &str = "gameId, week, gameDate, gameTimeEastern, homeTeamAbbr, visitorTeamAbbr, location, stadium";

fn game_row(r: &Row) -> rusqlite::Result<GameInfo> {
    let neutral = text(r, 6)?.as_deref() == Some("Neutral");
    Ok(GameInfo {
        id: int(r, 0)?.unwrap_or(0),
        site: if neutral { Some(text(r, 7)?.unwrap_or_else(|| "Neutral site".into())) } else { None },
        week: int(r, 1)?.unwrap_or(0),
        date: text(r, 2)?.unwrap_or_default(),
        time: text(r, 3)?.unwrap_or_default(),
        home: text(r, 4)?.unwrap_or_default(),
        away: text(r, 5)?.unwrap_or_default(),
    })
}

pub fn list_games(conn: &Connection) -> Res<Vec<GameInfo>> {
    let mut stmt = conn.prepare(&format!("SELECT {GAME_COLUMNS} FROM games ORDER BY week, gameId"))?;
    let games = stmt.query_map([], game_row)?.collect::<Result<_, _>>()?;
    Ok(games)
}

pub fn game_info(conn: &Connection, gid: i64) -> Res<Option<GameInfo>> {
    let sql = format!("SELECT {GAME_COLUMNS} FROM games WHERE gameId = ?1");
    Ok(conn.query_row(&sql, [gid], game_row).optional()?)
}

pub fn teams(conn: &Connection) -> Res<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT homeTeamAbbr FROM games UNION SELECT visitorTeamAbbr FROM games")?;
    let teams = stmt.query_map([], |r| Ok(text(r, 0)?.unwrap_or_default()))?.collect::<Result<_, _>>()?;
    Ok(teams)
}

const PLAY_COLUMNS: &str = "playId, quarter, gameClock, down, yardsToGo, possessionTeam, defensiveTeam, \
    yardlineSide, yardlineNumber, playDescription, passResult, playResult, foulName1, foulName2, foulName3, \
    preSnapHomeScore, preSnapVisitorScore, offenseFormation, personnelO, personnelD, defendersInBox, \
    pff_passCoverage, pff_passCoverageType, pff_playAction, dropBackType";

fn play_row(r: &Row) -> rusqlite::Result<PlaySummary> {
    let yl = match (text(r, 7)?, int(r, 8)?) {
        (Some(side), Some(n)) if n != 50 => format!("{side} {n}"),
        _ => "50".to_string(),
    };
    Ok(PlaySummary {
        id: int(r, 0)?.unwrap_or(0),
        q: int(r, 1)?.unwrap_or(0),
        clock: text(r, 2)?.unwrap_or_default(),
        down: int(r, 3)?,
        ytg: int(r, 4)?,
        pos: text(r, 5)?.unwrap_or_default(),
        def: text(r, 6)?.unwrap_or_default(),
        yl,
        desc: text(r, 9)?.unwrap_or_default(),
        res: text(r, 10)?,
        yds: int(r, 11)?,
        fouls: [text(r, 12)?, text(r, 13)?, text(r, 14)?].into_iter().flatten().collect(),
        hs: int(r, 15)?.unwrap_or(0),
        vs: int(r, 16)?.unwrap_or(0),
        form: text(r, 17)?,
        pers_o: text(r, 18)?,
        pers_d: text(r, 19)?,
        in_box: int(r, 20)?,
        cov: text(r, 21)?,
        cov_type: text(r, 22)?,
        pa: int(r, 23)? == Some(1),
        drop: text(r, 24)?,
    })
}

/// The game plus every play that has tracking data, in game order.
pub fn game_summary(conn: &Connection, gid: i64) -> Res<Option<GameSummary>> {
    let Some(game) = game_info(conn, gid)? else { return Ok(None) };
    let mut stmt = conn.prepare(&format!(
        "SELECT {PLAY_COLUMNS} FROM plays p WHERE gameId = ?1 \
         AND EXISTS (SELECT 1 FROM tracking t WHERE t.gameId = p.gameId AND t.playId = p.playId) \
         ORDER BY playId"
    ))?;
    let plays = stmt.query_map([gid], play_row)?.collect::<Result<_, _>>()?;
    Ok(Some(GameSummary { game, plays }))
}

/// All 22 players and the ball for every frame of one play.
pub fn play_tracking(conn: &Connection, gid: i64, pid: i64) -> Res<Option<PlayTracking>> {
    let teams = conn
        .query_row("SELECT homeTeamAbbr, visitorTeamAbbr FROM games WHERE gameId = ?1", [gid], |r| {
            Ok((text(r, 0)?.unwrap_or_default(), text(r, 1)?.unwrap_or_default()))
        })
        .optional()?;
    let Some((home, away)) = teams else { return Ok(None) };

    // Entity key is the nflId; the ball has none.
    let mut order: Vec<Option<i64>> = Vec::new();
    let mut info: HashMap<Option<i64>, (u8, Option<i64>)> = HashMap::new();
    let mut frames: BTreeMap<i64, HashMap<Option<i64>, [Option<i64>; STRIDE]>> = BTreeMap::new();
    let mut events: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    let mut dir = None;

    let mut stmt = conn.prepare(
        "SELECT nflId, frameId, jerseyNumber, team, playDirection, x, y, o, s, event \
         FROM tracking WHERE gameId = ?1 AND playId = ?2 ORDER BY frameId, rowid",
    )?;
    let mut rows = stmt.query([gid, pid])?;
    while let Some(r) = rows.next()? {
        let key = int(r, 0)?;
        let frame = int(r, 1)?.unwrap_or(0);
        if !info.contains_key(&key) {
            let team = text(r, 3)?.unwrap_or_default();
            let side = if team == home { 0 } else if team == away { 1 } else { 2 };
            order.push(key);
            info.insert(key, (side, int(r, 2)?));
        }
        if dir.is_none() {
            dir = text(r, 4)?;
        }
        let o = real(r, 7)?.map(|v| v.round_ties_even() as i64);
        let pos = [real(r, 5)?.map(tenths), real(r, 6)?.map(tenths), o, real(r, 8)?.map(tenths)];
        frames.entry(frame).or_default().insert(key, pos);
        if let Some(e) = text(r, 9)?.filter(|e| e != "None") {
            let list = events.entry(frame).or_default();
            if !list.contains(&e) {
                list.push(e);
            }
        }
    }
    if order.is_empty() {
        return Ok(None);
    }

    let mut roles = HashMap::new();
    let mut stmt =
        conn.prepare("SELECT nflId, pff_role, pff_positionLinedUp FROM pff_scouting WHERE gameId = ?1 AND playId = ?2")?;
    let mut rows = stmt.query([gid, pid])?;
    while let Some(r) = rows.next()? {
        roles.insert(int(r, 0)?, (text(r, 1)?, text(r, 2)?));
    }

    let mut keys = order;
    keys.sort_by_key(|k| (info[k].0, info[k].1.unwrap_or(0)));

    let frame_ids: Vec<i64> = frames.keys().copied().collect();
    let index: HashMap<i64, usize> = frame_ids.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let f: Vec<Vec<Option<i64>>> = frame_ids
        .iter()
        .map(|fid| keys.iter().flat_map(|k| frames[fid].get(k).copied().unwrap_or([None; STRIDE])).collect())
        .collect();

    let ents: Vec<Ent> = keys
        .iter()
        .map(|k| {
            let (side, jersey) = info[k];
            let (role, lined) = k.and_then(|id| roles.get(&Some(id)).cloned()).unwrap_or((None, None));
            (*k, side, jersey, role, lined)
        })
        .collect();

    let ev: Vec<(usize, String)> =
        events.iter().flat_map(|(fid, names)| names.iter().map(|n| (index[fid], n.clone()))).collect();
    let snap = ev.iter().find(|(_, e)| e.contains("snap")).map_or(0, |(i, _)| *i);

    // The ball sits on the line of scrimmage at the snap.
    let los = keys.iter().position(|k| k.is_none()).and_then(|b| f[snap][b * STRIDE]).map(|bx| bx as f64 / 10.0);

    let mut players = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT displayName, officialPosition FROM players WHERE nflId = ?1")?;
    for id in keys.iter().flatten() {
        let found = stmt
            .query_row([id], |r| Ok((text(r, 0)?.unwrap_or_default(), text(r, 1)?.unwrap_or_default())))
            .optional()?;
        players.insert(id.to_string(), found.unwrap_or(("Unknown".into(), String::new())));
    }

    Ok(Some(PlayTracking {
        dir: if dir.as_deref() == Some("right") { "R" } else { "L" },
        los,
        snap,
        ev,
        ents,
        f,
        players,
    }))
}
