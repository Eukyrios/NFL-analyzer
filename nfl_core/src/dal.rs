//! Typed models and the read-only data access layer.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row};

use crate::blob::{self, Frame};
use crate::schema::{self, SCHEMA_VERSION};
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub game_id: i64,
    pub week: i64,
    pub game_date: String,
    pub game_time: String,
    pub home: String,
    pub away: String,
    pub location: Option<String>,
    pub stadium: Option<String>,
    pub home_score: Option<i64>,
    pub away_score: Option<i64>,
    pub overtime: bool,
}

impl Game {
    pub fn is_neutral(&self) -> bool {
        self.location.as_deref() == Some("Neutral")
    }
    /// `BAL @ LV`, or `NYJ vs ATL` at a neutral site (home team listed first).
    pub fn matchup(&self) -> String {
        if self.is_neutral() { format!("{} vs {}", self.home, self.away) } else { format!("{} @ {}", self.away, self.home) }
    }
    pub fn final_score(&self) -> Option<(i64, i64)> {
        Some((self.home_score?, self.away_score?))
    }
    /// `LV 33–27 BAL (OT)`: winner first.
    pub fn final_label(&self) -> Option<String> {
        let (h, a) = self.final_score()?;
        let ot = if self.overtime { " (OT)" } else { "" };
        Some(if h >= a {
            format!("{} {h}–{a} {}{ot}", self.home, self.away)
        } else {
            format!("{} {a}–{h} {}{ot}", self.away, self.home)
        })
    }
    /// `Mon 09/13 · 20:15 ET`
    pub fn kickoff_label(&self) -> String {
        let day = weekday(&self.game_date).unwrap_or("");
        let md = self.game_date.get(0..5).unwrap_or(&self.game_date);
        let hm = self.game_time.get(0..5).unwrap_or(&self.game_time);
        format!("{day} {md} · {hm} ET").trim().to_string()
    }
}

/// Weekday for `MM/DD/YYYY` (Sakamoto's method).
fn weekday(date: &str) -> Option<&'static str> {
    let mut it = date.split('/').map(|p| p.parse::<i64>().ok());
    let (m, d, mut y) = (it.next()??, it.next()??, it.next()??);
    const T: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    let w = (y + y / 4 - y / 100 + y / 400 + T.get((m - 1) as usize)? + d).rem_euclid(7);
    Some(["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][w as usize])
}

#[derive(Debug, Clone, PartialEq)]
pub struct Team {
    pub abbr: String,
    pub name: String,
    pub logo_png: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub nfl_id: i64,
    pub name: String,
    pub position: Option<String>,
    pub height: Option<String>,
    pub weight: Option<i64>,
    pub birth_date: Option<String>,
    pub college: Option<String>,
}

impl Player {
    /// Age at the 2021 season opener (2021-09-09).
    pub fn age(&self) -> Option<i64> {
        let b = self.birth_date.as_deref()?;
        let nums: Vec<i64> = b.split(['-', '/']).filter_map(|p| p.parse().ok()).collect();
        let (y, m, d) = match nums.as_slice() {
            [y, m, d] if *y > 1000 => (*y, *m, *d),
            [m, d, y] => (*y, *m, *d),
            _ => return None,
        };
        Some(2021 - y - i64::from((m, d) > (9, 9)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    pub game_id: i64,
    pub play_id: i64,
    pub description: String,
    pub quarter: i64,
    pub down: i64,
    pub yards_to_go: i64,
    pub offense: String,
    pub defense: String,
    pub yardline_side: Option<String>,
    pub yardline_number: Option<i64>,
    pub game_clock: String,
    pub home_score: i64,
    pub away_score: i64,
    pub pass_result: Option<String>,
    pub play_result: Option<i64>,
    pub absolute_yardline: Option<f32>,
    pub formation: Option<String>,
    pub personnel_o: Option<String>,
    pub personnel_d: Option<String>,
    pub defenders_in_box: Option<i64>,
    pub dropback_type: Option<String>,
    pub play_action: bool,
    pub coverage: Option<String>,
    pub coverage_type: Option<String>,
    pub time_to_throw: Option<f32>,
}

impl Play {
    /// Line of scrimmage on the normalized field (offense left → right).
    pub fn los_x(&self) -> Option<f32> {
        Some(crate::geometry::los_from_yardline(&self.offense, self.yardline_side.as_deref(), self.yardline_number?))
    }

    /// First-down line, clamped to the goal line; `true` when it's "& Goal".
    pub fn first_down(&self) -> Option<(f32, bool)> {
        (self.down > 0).then_some(())?;
        Some(crate::geometry::first_down_x(self.los_x()?, self.yards_to_go))
    }

    /// `2nd & 7 · LV 39` (or `& Goal`).
    pub fn situation(&self) -> String {
        let goal = self.first_down().is_some_and(|(_, g)| g);
        let dist = if goal { "Goal".to_string() } else { self.yards_to_go.to_string() };
        let spot = match (&self.yardline_side, self.yardline_number) {
            (_, Some(50)) => " · 50".to_string(),
            (Some(s), Some(n)) => format!(" · {s} {n}"),
            _ => String::new(),
        };
        format!("{} & {dist}{spot}", crate::labels::down(self.down))
    }
}

pub const PLAY_COLUMNS: &str = "p.gameId, p.playId, p.playDescription, p.quarter, p.down, p.yardsToGo, \
    p.possessionTeam, p.defensiveTeam, p.yardlineSide, p.yardlineNumber, p.gameClock, p.preSnapHomeScore, \
    p.preSnapVisitorScore, p.passResult, p.playResult, p.absoluteYardlineNumber, p.offenseFormation, \
    p.personnelO, p.personnelD, p.defendersInBox, p.dropBackType, p.pff_playAction, p.pff_passCoverage, \
    p.pff_passCoverageType, s.time_to_throw";

pub fn play_from_row(r: &Row) -> rusqlite::Result<Play> {
    Ok(Play {
        game_id: r.get(0)?,
        play_id: r.get(1)?,
        description: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        quarter: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
        down: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
        yards_to_go: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
        offense: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        defense: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
        yardline_side: r.get(8)?,
        yardline_number: r.get(9)?,
        game_clock: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
        home_score: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
        away_score: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
        pass_result: r.get(13)?,
        play_result: r.get(14)?,
        absolute_yardline: r.get::<_, Option<f64>>(15)?.map(|v| v as f32),
        formation: r.get(16)?,
        personnel_o: r.get(17)?,
        personnel_d: r.get(18)?,
        defenders_in_box: r.get(19)?,
        dropback_type: r.get(20)?,
        play_action: r.get::<_, Option<i64>>(21)? == Some(1),
        coverage: r.get(22)?,
        coverage_type: r.get(23)?,
        time_to_throw: r.get::<_, Option<f64>>(24)?.map(|v| v as f32),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayEntity {
    pub slot: usize,
    pub nfl_id: Option<i64>,
    pub jersey: Option<i64>,
    /// Team abbreviation, or `football` for the ball.
    pub team: String,
    /// The player's PFF role on this play, when scouted.
    pub role: Option<String>,
}

impl PlayEntity {
    pub fn is_ball(&self) -> bool {
        self.nfl_id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaySummary {
    pub snap_frame: i64,
    pub throw_frame: Option<i64>,
    pub sack_frame: Option<i64>,
    pub end_frame: i64,
    pub time_to_throw: Option<f32>,
}

/// Everything the viewer needs for one play.
#[derive(Debug, Clone)]
pub struct PlayTracking {
    pub entities: Vec<PlayEntity>,
    pub frames: Vec<Frame>,
    pub events: Vec<(i64, String)>,
    pub summary: PlaySummary,
}

pub struct Db {
    pub(crate) conn: Connection,
}

impl Db {
    /// Opens the compact DB read-only and checks the schema version.
    pub fn open_ro(path: &Path) -> Result<Db> {
        if !path.exists() {
            return Err(format!("database not found at {}", path.display()).into());
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        Self::from_conn(conn)
    }

    /// Read-only with `immutable=1` (no locking, no journal): for a DB inside a
    /// read-only app bundle (iOS).
    pub fn open_immutable(path: &Path) -> Result<Db> {
        if !path.exists() {
            return Err(format!("database not found at {}", path.display()).into());
        }
        let uri = format!("file:{}?immutable=1", path.display());
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
        Self::from_conn(Connection::open_with_flags(uri, flags)?)
    }

    /// Wraps an existing connection (tests use in-memory DBs) after the version check.
    pub fn from_conn(conn: Connection) -> Result<Db> {
        let v = schema::version(&conn)?;
        if v != SCHEMA_VERSION {
            return Err(format!("database schema version {v}, expected {SCHEMA_VERSION}; rebuild it with the etl").into());
        }
        Ok(Db { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        let mut s = self.conn.prepare(
            "SELECT gameId, week, gameDate, gameTimeEastern, homeTeamAbbr, visitorTeamAbbr, location, stadium, \
             home_score, away_score, overtime FROM games ORDER BY gameDate, gameTimeEastern, gameId",
        )?;
        let rows = s.query_map([], game_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn game(&self, game_id: i64) -> Result<Option<Game>> {
        Ok(self
            .conn
            .query_row(
                "SELECT gameId, week, gameDate, gameTimeEastern, homeTeamAbbr, visitorTeamAbbr, location, stadium, \
                 home_score, away_score, overtime FROM games WHERE gameId = ?1",
                [game_id],
                game_from_row,
            )
            .optional()?)
    }

    pub fn teams(&self) -> Result<Vec<Team>> {
        let mut s = self.conn.prepare("SELECT abbr, name, logo_png FROM teams ORDER BY abbr")?;
        let rows = s.query_map([], |r| Ok(Team { abbr: r.get(0)?, name: r.get(1)?, logo_png: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn plays_for_game(&self, game_id: i64) -> Result<Vec<Play>> {
        let sql = format!(
            "SELECT {PLAY_COLUMNS} FROM plays p LEFT JOIN play_summary s USING (gameId, playId) \
             WHERE p.gameId = ?1 ORDER BY p.playId"
        );
        let mut s = self.conn.prepare(&sql)?;
        let rows = s.query_map([game_id], play_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn play(&self, game_id: i64, play_id: i64) -> Result<Option<Play>> {
        let sql = format!(
            "SELECT {PLAY_COLUMNS} FROM plays p LEFT JOIN play_summary s USING (gameId, playId) \
             WHERE p.gameId = ?1 AND p.playId = ?2"
        );
        Ok(self.conn.query_row(&sql, [game_id, play_id], play_from_row).optional()?)
    }

    pub fn player(&self, nfl_id: i64) -> Result<Option<Player>> {
        Ok(self
            .conn
            .query_row(
                "SELECT nflId, displayName, officialPosition, height, weight, birthDate, collegeName \
                 FROM players WHERE nflId = ?1",
                [nfl_id],
                |r| {
                    Ok(Player {
                        nfl_id: r.get(0)?,
                        name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        position: r.get(2)?,
                        height: r.get(3)?,
                        weight: r.get(4)?,
                        birth_date: r.get(5)?,
                        college: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    /// The team a player appeared for most often (from tracking entities).
    pub fn player_team(&self, nfl_id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT team FROM play_entities WHERE nflId = ?1 GROUP BY team ORDER BY COUNT(*) DESC LIMIT 1",
                [nfl_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn tracking(&self, game_id: i64, play_id: i64) -> Result<Option<PlayTracking>> {
        let mut s = self.conn.prepare(
            "SELECT e.slot, e.nflId, e.jerseyNumber, e.team, f.pff_role FROM play_entities e \
             LEFT JOIN pff_scouting f ON f.gameId = e.gameId AND f.playId = e.playId AND f.nflId = e.nflId \
             WHERE e.gameId = ?1 AND e.playId = ?2 ORDER BY e.slot",
        )?;
        let entities: Vec<PlayEntity> = s
            .query_map([game_id, play_id], |r| {
                Ok(PlayEntity {
                    slot: r.get::<_, i64>(0)? as usize,
                    nfl_id: r.get(1)?,
                    jersey: r.get(2)?,
                    team: r.get(3)?,
                    role: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT blob FROM play_frames WHERE gameId = ?1 AND playId = ?2", [game_id, play_id], |r| r.get(0))
            .optional()?;
        let Some(blob) = blob else { return Ok(None) };
        let frames = blob::decode(&blob, entities.len())?;
        let mut s = self.conn.prepare("SELECT frameId, event FROM play_events WHERE gameId = ?1 AND playId = ?2 ORDER BY frameId")?;
        let events = s.query_map([game_id, play_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let summary = self
            .conn
            .query_row(
                "SELECT snap_frame, throw_frame, sack_frame, end_frame, time_to_throw FROM play_summary \
                 WHERE gameId = ?1 AND playId = ?2",
                [game_id, play_id],
                |r| {
                    Ok(PlaySummary {
                        snap_frame: r.get(0)?,
                        throw_frame: r.get(1)?,
                        sack_frame: r.get(2)?,
                        end_frame: r.get(3)?,
                        time_to_throw: r.get::<_, Option<f64>>(4)?.map(|v| v as f32),
                    })
                },
            )
            .optional()?
            .unwrap_or(PlaySummary { snap_frame: 1, end_frame: frames.len() as i64, ..Default::default() });
        Ok(Some(PlayTracking { entities, frames, events, summary }))
    }
}

/// Columns read by [`game_from_row`] (table alias `g`).
pub const GAME_COLUMNS: &str = "g.gameId, g.week, g.gameDate, g.gameTimeEastern, g.homeTeamAbbr, g.visitorTeamAbbr, \
    g.location, g.stadium, g.home_score, g.away_score, g.overtime";

pub(crate) fn game_from_row(r: &Row) -> rusqlite::Result<Game> {
    Ok(Game {
        game_id: r.get(0)?,
        week: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
        game_date: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        game_time: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        home: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        away: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
        location: r.get(6)?,
        stadium: r.get(7)?,
        home_score: r.get(8)?,
        away_score: r.get(9)?,
        overtime: r.get::<_, Option<i64>>(10)?.unwrap_or(0) != 0,
    })
}

/// A small seeded database used by unit tests here and in the app crate.
pub mod testdata {
    use super::*;
    use crate::blob::EntityPos;

    pub fn seeded() -> Result<Db> {
        let c = Connection::open_in_memory()?;
        schema::create_all(&c)?;
        c.execute_batch(
            r#"
            INSERT INTO games VALUES
              (1, 2021, 1, '09/13/2021', '20:15:00', 'LV', 'BAL', 'Home', 'Allegiant Stadium', 33, 27, 1),
              (2, 2021, 5, '10/10/2021', '09:30:00', 'ATL', 'NYJ', 'Neutral', 'Tottenham Stadium', 27, 20, 0);
            INSERT INTO teams VALUES ('BAL','Baltimore Ravens','#241773','#9E7C0C',NULL),
              ('LV','Las Vegas Raiders','#000000','#A5ACAF',NULL),
              ('ATL','Atlanta Falcons','#A71930','#000000',NULL),
              ('NYJ','New York Jets','#125740','#FFFFFF',NULL);
            INSERT INTO players VALUES
              (10, '6-2', 212, '1997-01-07', 'Louisville', 'QB', 'Lamar Jackson'),
              (20, '6-6', 320, '1990-05-01', 'Alabama', 'T', 'Kolton Miller'),
              (30, '6-5', 255, '1997-06-12', 'Michigan', 'DE', 'Maxx Crosby'),
              (40, '6-0', 200, '1996-03-01', 'Iowa', 'CB', 'Some Corner'),
              (50, '6-4', 220, '1997-11-14', 'Wyoming', 'QB', 'Derek Carr');
            INSERT INTO plays (gameId, playId, playDescription, quarter, down, yardsToGo, possessionTeam, defensiveTeam,
                yardlineSide, yardlineNumber, gameClock, preSnapHomeScore, preSnapVisitorScore, passResult, playResult,
                absoluteYardlineNumber, offenseFormation, personnelO, personnelD, defendersInBox, dropBackType,
                pff_playAction, pff_passCoverage, pff_passCoverageType) VALUES
              (1, 1, 'L.Jackson pass short right complete', 1, 1, 10, 'BAL', 'LV', 'BAL', 25, '15:00', 0, 0, 'C', 8, 35, 'SHOTGUN', '1 RB, 1 TE, 3 WR', '4 DL, 2 LB, 5 DB', 6, 'TRADITIONAL', 0, 'Cover-3', 'Zone'),
              (1, 2, 'L.Jackson sacked', 1, 2, 2, 'BAL', 'LV', 'BAL', 33, '14:20', 0, 0, 'S', -7, 43, 'SHOTGUN', '1 RB, 1 TE, 3 WR', '4 DL, 2 LB, 5 DB', 6, 'TRADITIONAL', 1, 'Cover-1', 'Man'),
              (1, 3, 'D.Carr pass incomplete', 2, 3, 7, 'LV', 'BAL', 'LV', 39, '11:30', 7, 3, 'I', 0, 49, 'EMPTY', '1 RB, 1 TE, 3 WR', '4 DL, 2 LB, 5 DB', 6, 'DESIGNED_ROLLOUT_RIGHT', 0, 'Cover-1', 'Man'),
              (2, 1, 'Jets pass complete', 1, 1, 10, 'NYJ', 'ATL', 'NYJ', 25, '15:00', 0, 0, 'C', 5, 35, 'SHOTGUN', '1 RB, 1 TE, 3 WR', '4 DL, 2 LB, 5 DB', 6, 'TRADITIONAL', 0, NULL, NULL);
            INSERT INTO pff_scouting (gameId, playId, nflId, pff_role, pff_positionLinedUp, pff_hit, pff_hurry, pff_sack,
                pff_beatenByDefender, pff_hitAllowed, pff_hurryAllowed, pff_sackAllowed, pff_blockType) VALUES
              (1, 1, 10, 'Pass', 'QB', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
              (1, 1, 30, 'Pass Rush', 'LEO', 0, 1, 0, NULL, NULL, NULL, NULL, NULL),
              (1, 2, 10, 'Pass', 'QB', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
              (1, 2, 30, 'Pass Rush', 'LEO', 0, 0, 1, NULL, NULL, NULL, NULL, NULL),
              (1, 3, 50, 'Pass', 'QB', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
              (1, 3, 20, 'Pass Block', 'LT', NULL, NULL, NULL, 1, 0, 1, 0, 'PP'),
              (1, 3, 40, 'Coverage', 'RCB', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO play_summary VALUES (1, 1, 6, 30, NULL, 33, 2.4), (1, 2, 6, NULL, 40, 41, NULL), (1, 3, 6, 32, NULL, 35, 2.6);
            INSERT INTO play_events VALUES (1, 1, 6, 'ball_snap'), (1, 1, 30, 'pass_forward');
            INSERT INTO player_play_stats VALUES (1, 1, 10, 6.0, 3.0, 9.0), (1, 2, 10, 8.0, 4.0, 11.0),
              (1, 3, 40, 7.5, 3.5, 12.0), (1, 1, 30, 9.0, 5.0, 14.0);
            INSERT INTO play_entities VALUES (1, 1, 0, 10, 8, 'BAL'), (1, 1, 1, 30, 98, 'LV'),
              (1, 1, 2, NULL, NULL, 'football');
            "#,
        )?;
        let frames: Vec<Frame> = (0..3)
            .map(|f| {
                let f = f as f32;
                vec![
                    Some(EntityPos { x: 30.0 - f, y: 26.0, s: 1.0, o: Some(90.0), dir: Some(270.0) }),
                    Some(EntityPos { x: 36.0 - f, y: 20.0, s: 2.0, o: Some(270.0), dir: Some(270.0) }),
                    Some(EntityPos { x: 35.0, y: 26.6, s: 0.0, o: None, dir: None }),
                ]
            })
            .collect();
        c.execute(
            "INSERT INTO play_frames VALUES (1, 1, 3, ?1)",
            [crate::blob::encode(&frames, 3)],
        )?;
        Db::from_conn(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries() {
        let db = testdata::seeded().unwrap();
        let games = db.games().unwrap();
        assert_eq!(games.len(), 2);
        let lv = &games[0];
        assert_eq!(lv.matchup(), "BAL @ LV");
        assert_eq!(lv.final_label().unwrap(), "LV 33–27 BAL (OT)");
        assert_eq!(lv.kickoff_label(), "Mon 09/13 · 20:15 ET");
        assert!(!lv.is_neutral());
        let london = db.game(2).unwrap().unwrap();
        assert!(london.is_neutral());
        assert_eq!(london.matchup(), "ATL vs NYJ");
        assert_eq!(db.teams().unwrap().len(), 4);
        let plays = db.plays_for_game(1).unwrap();
        assert_eq!(plays.len(), 3);
        assert_eq!(plays[2].situation(), "3rd & 7 · LV 39");
        assert_eq!(plays[0].time_to_throw, Some(2.4));
        assert_eq!(db.player(10).unwrap().unwrap().age(), Some(24));
        let t = db.tracking(1, 1).unwrap().unwrap();
        assert_eq!(t.entities.len(), 3);
        assert_eq!(t.frames.len(), 3);
        assert!(t.entities[2].is_ball());
        assert_eq!(t.entities[0].role.as_deref(), Some("Pass"));
        assert_eq!(t.summary.throw_frame, Some(30));
        assert!(db.tracking(1, 3).unwrap().is_none());
        assert_eq!(db.player_team(30).unwrap().as_deref(), Some("LV"));
    }

    #[test]
    fn rejects_wrong_version() {
        let c = Connection::open_in_memory().unwrap();
        assert!(Db::from_conn(c).is_err());
        assert!(Db::open_ro(Path::new("/nonexistent/nfl.sqlite")).is_err());
    }
}
