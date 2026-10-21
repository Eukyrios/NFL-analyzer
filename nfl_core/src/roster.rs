//! Team screen data: schedule, roster, the most-used lineup, and a player's
//! game log. All from SQL aggregates over `play_entities`, `pff_scouting`,
//! `plays` and `player_play_stats`.

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use crate::dal::{game_from_row, Db, Game, GAME_COLUMNS};
use crate::geometry::WID;
use crate::stats::Role;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct RosterEntry {
    pub nfl_id: i64,
    pub name: String,
    pub position: String,
    pub jersey: Option<i64>,
    /// Plays the player was tracked on for this team.
    pub snaps: u64,
    /// Most frequent PFF role (raw value, e.g. `Pass Rush`).
    pub role: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamGame {
    pub game: Game,
    pub opponent: String,
    /// `None` at neutral sites.
    pub home: Option<bool>,
    pub plays_off: u64,
    pub plays_def: u64,
    /// (team score, opponent score) when known.
    pub score: Option<(i64, i64)>,
}

impl TeamGame {
    /// `W 33–27 (OT)`, `L 20–27`, `T 20–20`.
    pub fn result_label(&self) -> Option<String> {
        let (us, them) = self.score?;
        let wlt = match us.cmp(&them) {
            std::cmp::Ordering::Greater => "W",
            std::cmp::Ordering::Less => "L",
            std::cmp::Ordering::Equal => "T",
        };
        let ot = if self.game.overtime { " (OT)" } else { "" };
        Some(format!("{wlt} {us}–{them}{ot}"))
    }
    /// `@ LV`, `vs LV` (home) or `vs LV · London` style neutral label.
    pub fn opponent_label(&self) -> String {
        match self.home {
            Some(false) => format!("@ {}", self.opponent),
            _ => format!("vs {}", self.opponent),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGame {
    pub game: Game,
    pub team: String,
    pub opponent: String,
    pub snaps: u64,
    /// Role-specific one-liner, e.g. `C/A 21/30 · 2 sacks`.
    pub headline: String,
}

/// One position in the lineup diagram. `dx` is yards from the line of
/// scrimmage (offense negative, defense positive), `y` is across the field.
#[derive(Debug, Clone, PartialEq)]
pub struct LineupSlot {
    pub label: &'static str,
    pub dx: f32,
    pub y: f32,
    pub player: Option<RosterEntry>,
    /// Snaps at this lined-up position.
    pub snaps: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lineup {
    pub offense: Vec<LineupSlot>,
    pub defense: Vec<LineupSlot>,
}

struct SlotDef {
    label: &'static str,
    codes: &'static [&'static str],
    dx: f32,
    dy: f32,
}

const fn s(label: &'static str, codes: &'static [&'static str], dx: f32, dy: f32) -> SlotDef {
    SlotDef { label, codes, dx, dy }
}

/// 11 personnel: filled in order, each slot takes the unused player with the
/// most snaps at any of its PFF lined-up codes. +dy is the offense's left.
const OFFENSE: [SlotDef; 11] = [
    s("QB", &["QB"], -5.0, 0.0),
    s("C", &["C"], -1.0, 0.0),
    s("LG", &["LG"], -1.0, 2.0),
    s("RG", &["RG"], -1.0, -2.0),
    s("LT", &["LT"], -1.0, 4.0),
    s("RT", &["RT"], -1.0, -4.0),
    s("RB", &["HB", "HB-L", "HB-R", "FB", "FB-L", "FB-R"], -6.0, -2.0),
    s("TE", &["TE-R", "TE-L", "TE-iR", "TE-iL", "TE-oR", "TE-oL"], -1.5, -6.5),
    s("WR", &["LWR"], -1.0, 20.0),
    s("WR", &["RWR"], -1.0, -20.0),
    s("SLOT", &["SLWR", "SRWR", "SLiWR", "SRiWR", "SLoWR", "SRoWR"], -2.0, 13.0),
];

/// Nickel (4-2-5). The defense's left is the offense's right (−dy).
const DEFENSE: [SlotDef; 11] = [
    s("DT", &["DLT", "NLT", "NT"], 1.5, -1.8),
    s("DT", &["DRT", "NRT", "NT"], 1.5, 1.8),
    s("EDGE", &["LEO", "LE", "LOLB"], 1.5, -5.5),
    s("EDGE", &["REO", "RE", "ROLB"], 1.5, 5.5),
    s("LB", &["LILB", "LLB", "MLB"], 5.0, -3.0),
    s("LB", &["RILB", "RLB", "MLB"], 5.0, 3.0),
    s("CB", &["LCB"], 7.0, -20.0),
    s("CB", &["RCB"], 7.0, 20.0),
    s("NB", &["SCBL", "SCBR", "SCBiL", "SCBiR", "SCBoL", "SCBoR"], 5.5, 12.0),
    s("S", &["FS", "FSL", "SSL"], 13.0, -8.0),
    s("S", &["SS", "FSR", "SSR"], 13.0, 8.0),
];

/// Role-aware team of a scouted player on a play.
const PLAYER_TEAM: &str = "CASE WHEN x.pff_role IN ('Pass Rush', 'Coverage') THEN p.defensiveTeam ELSE p.possessionTeam END";

/// Unit and position group for the roster list, with a sort key.
pub fn position_group(pos: &str) -> (&'static str, &'static str, u8) {
    match pos {
        "QB" => ("Offense", "Quarterbacks", 0),
        "RB" | "FB" => ("Offense", "Running backs", 1),
        "WR" => ("Offense", "Wide receivers", 2),
        "TE" => ("Offense", "Tight ends", 3),
        "T" | "G" | "C" => ("Offense", "Offensive line", 4),
        "DE" | "DT" | "NT" => ("Defense", "Defensive line", 5),
        "OLB" | "ILB" | "MLB" | "LB" => ("Defense", "Linebackers", 6),
        "CB" => ("Defense", "Cornerbacks", 7),
        "SS" | "FS" | "DB" => ("Defense", "Safeties", 8),
        _ => ("Other", "Other", 9),
    }
}

impl Db {
    pub fn team_games(&self, abbr: &str) -> Result<Vec<TeamGame>> {
        let sql = format!(
            "SELECT {GAME_COLUMNS}, COALESCE(SUM(p.possessionTeam = ?1), 0), COALESCE(SUM(p.defensiveTeam = ?1), 0) \
             FROM games g LEFT JOIN plays p ON p.gameId = g.gameId \
             WHERE g.homeTeamAbbr = ?1 OR g.visitorTeamAbbr = ?1 GROUP BY g.gameId ORDER BY g.week, g.gameId"
        );
        let mut s = self.conn.prepare_cached(&sql)?;
        let rows = s.query_map([abbr], |r| Ok((game_from_row(r)?, r.get::<_, i64>(11)? as u64, r.get::<_, i64>(12)? as u64)))?;
        rows.map(|row| {
            let (game, off, def) = row?;
            let is_home = game.home == abbr;
            let opponent = if is_home { game.away.clone() } else { game.home.clone() };
            let score = game.final_score().map(|(h, a)| if is_home { (h, a) } else { (a, h) });
            let home = (!game.is_neutral()).then_some(is_home);
            Ok(TeamGame { game, opponent, home, plays_off: off, plays_def: def, score })
        })
        .collect()
    }

    /// Everyone tracked for the team, most snaps first.
    pub fn team_roster(&self, abbr: &str) -> Result<Vec<RosterEntry>> {
        let mut s = self.conn.prepare_cached(
            "SELECT e.nflId, COALESCE(pl.displayName, ''), COALESCE(pl.officialPosition, '?'), MAX(e.jerseyNumber), COUNT(*) \
             FROM play_entities e LEFT JOIN players pl ON pl.nflId = e.nflId \
             WHERE e.team = ?1 AND e.nflId IS NOT NULL GROUP BY e.nflId ORDER BY 5 DESC, 2",
        )?;
        let mut out: Vec<RosterEntry> = s
            .query_map([abbr], |r| {
                Ok(RosterEntry {
                    nfl_id: r.get(0)?,
                    name: r.get(1)?,
                    position: r.get(2)?,
                    jersey: r.get(3)?,
                    snaps: r.get::<_, i64>(4)? as u64,
                    role: String::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let roles = self.main_roles(abbr)?;
        for e in &mut out {
            e.role = roles.get(&e.nfl_id).cloned().unwrap_or_default();
        }
        Ok(out)
    }

    fn main_roles(&self, abbr: &str) -> Result<HashMap<i64, String>> {
        let sql = format!(
            "SELECT x.nflId, x.pff_role, COUNT(*) FROM pff_scouting x JOIN plays p ON p.gameId = x.gameId AND p.playId = x.playId \
             WHERE {PLAYER_TEAM} = ?1 GROUP BY 1, 2 ORDER BY 3 DESC"
        );
        let mut s = self.conn.prepare_cached(&sql)?;
        let mut out = HashMap::new();
        for row in s.query_map([abbr], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))? {
            let (id, role) = row?;
            out.entry(id).or_insert_with(|| role.unwrap_or_default());
        }
        Ok(out)
    }

    /// The most-used player at each lineup slot, from PFF lined-up positions.
    pub fn team_lineup(&self, abbr: &str) -> Result<Lineup> {
        let sql = format!(
            "SELECT x.pff_positionLinedUp, x.nflId, COUNT(*) FROM pff_scouting x \
             JOIN plays p ON p.gameId = x.gameId AND p.playId = x.playId \
             WHERE {PLAYER_TEAM} = ?1 AND x.pff_positionLinedUp IS NOT NULL GROUP BY 1, 2"
        );
        let mut s = self.conn.prepare_cached(&sql)?;
        let mut at: HashMap<String, Vec<(i64, u64)>> = HashMap::new();
        for row in s.query_map([abbr], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)? as u64)))? {
            let (code, id, n) = row?;
            at.entry(code).or_default().push((id, n));
        }
        let roster: HashMap<i64, RosterEntry> = self.team_roster(abbr)?.into_iter().map(|e| (e.nfl_id, e)).collect();
        let fill = |defs: &[SlotDef]| {
            let mut used = HashSet::new();
            defs.iter()
                .map(|d| {
                    let mut totals: HashMap<i64, u64> = HashMap::new();
                    for code in d.codes {
                        for &(id, n) in at.get(*code).map(Vec::as_slice).unwrap_or(&[]) {
                            *totals.entry(id).or_default() += n;
                        }
                    }
                    let best = totals
                        .into_iter()
                        .filter(|(id, _)| !used.contains(id))
                        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
                    if let Some((id, _)) = best {
                        used.insert(id);
                    }
                    LineupSlot {
                        label: d.label,
                        dx: d.dx,
                        y: WID / 2.0 + d.dy,
                        player: best.and_then(|(id, _)| roster.get(&id).cloned()),
                        snaps: best.map_or(0, |b| b.1),
                    }
                })
                .collect()
        };
        Ok(Lineup { offense: fill(&OFFENSE), defense: fill(&DEFENSE) })
    }

    /// Games a player was tracked in, with a role-specific headline.
    pub fn player_games(&self, nfl_id: i64) -> Result<Vec<PlayerGame>> {
        let role = self.player_role(nfl_id)?;
        let sql = format!(
            "SELECT {GAME_COLUMNS}, e.team, COUNT(*) FROM play_entities e JOIN games g ON g.gameId = e.gameId \
             WHERE e.nflId = ?1 GROUP BY e.gameId, e.team ORDER BY g.week, g.gameId"
        );
        let mut s = self.conn.prepare_cached(&sql)?;
        let rows: Vec<(Game, String, u64)> = s
            .query_map([nfl_id], |r| Ok((game_from_row(r)?, r.get(11)?, r.get::<_, i64>(12)? as u64)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(game, team, snaps)| {
                let opponent = if game.home == team { game.away.clone() } else { game.home.clone() };
                let headline = self.game_headline(nfl_id, game.game_id, role)?;
                Ok(PlayerGame { game, team, opponent, snaps, headline })
            })
            .collect()
    }

    fn game_headline(&self, nfl_id: i64, game_id: i64, role: Option<Role>) -> Result<String> {
        const J: &str = "FROM pff_scouting x JOIN plays p ON p.gameId = x.gameId AND p.playId = x.playId \
                         WHERE x.nflId = ?1 AND x.gameId = ?2";
        let row3 = |sql: String| -> Result<(f64, f64, f64)> {
            Ok(self.conn.query_row(&sql, params![nfl_id, game_id], |r| {
                Ok((
                    r.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                    r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                ))
            })?)
        };
        let plural = |n: f64, w: &str| format!("{n:.0} {w}{}", if n == 1.0 { "" } else { "s" });
        Ok(match role {
            Some(Role::Passer) => {
                let (c, a, sk) = row3(format!(
                    "SELECT TOTAL(p.passResult = 'C'), TOTAL(p.passResult IN ('C','I','IN')), TOTAL(p.passResult = 'S') {J} AND x.pff_role = 'Pass'"
                ))?;
                format!("C/A {c:.0}/{a:.0} · {}", plural(sk, "sack"))
            }
            Some(Role::PassRush) => {
                let (h, u, sk) = row3(format!("SELECT TOTAL(x.pff_hit), TOTAL(x.pff_hurry), TOTAL(x.pff_sack) {J}"))?;
                format!("{} · {} · {}", plural(h, "hit"), plural(u, "hurry").replace("hurrys", "hurries"), plural(sk, "sack"))
            }
            Some(Role::PassBlock) => {
                let (h, u, sk) =
                    row3(format!("SELECT TOTAL(x.pff_hitAllowed), TOTAL(x.pff_hurryAllowed), TOTAL(x.pff_sackAllowed) {J}"))?;
                format!("{} allowed", plural(h + u + sk, "pressure"))
            }
            _ => {
                let top: Option<f64> = self.conn.query_row(
                    "SELECT MAX(max_speed) FROM player_play_stats WHERE nflId = ?1 AND gameId = ?2",
                    params![nfl_id, game_id],
                    |r| r.get(0),
                )?;
                top.map_or_else(String::new, |t| format!("Top speed {t:.1} yd/s"))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::testdata::seeded;

    #[test]
    fn schedule_and_results() {
        let db = seeded().unwrap();
        let g = db.team_games("BAL").unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].opponent_label(), "@ LV");
        assert_eq!(g[0].result_label().as_deref(), Some("L 27–33 (OT)"));
        assert_eq!((g[0].plays_off, g[0].plays_def), (2, 1));
        let nyj = db.team_games("NYJ").unwrap();
        assert_eq!(nyj[0].home, None, "neutral site");
        assert_eq!(nyj[0].opponent_label(), "vs ATL");
    }

    #[test]
    fn roster_and_lineup() {
        let db = seeded().unwrap();
        let r = db.team_roster("LV").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "Maxx Crosby");
        assert_eq!(r[0].role, "Pass Rush");
        assert_eq!(r[0].jersey, Some(98));
        let l = db.team_lineup("LV").unwrap();
        assert_eq!(l.offense.len(), 11);
        assert_eq!(l.defense.len(), 11);
        let edge = l.defense.iter().find(|s| s.player.is_some()).unwrap();
        assert_eq!(edge.label, "EDGE");
        assert_eq!(edge.snaps, 2);
        // Each player appears at most once.
        let ids: Vec<_> = l.defense.iter().chain(&l.offense).filter_map(|s| s.player.as_ref().map(|p| p.nfl_id)).collect();
        assert_eq!(ids.len(), ids.iter().collect::<HashSet<_>>().len());
        assert_eq!(position_group("CB").1, "Cornerbacks");
    }

    #[test]
    fn game_log() {
        let db = seeded().unwrap();
        let g = db.player_games(30).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].opponent, "BAL");
        assert_eq!(g[0].headline, "0 hits · 1 hurry · 1 sack");
        let qb = db.player_games(10).unwrap();
        assert_eq!(qb[0].headline, "C/A 1/1 · 1 sack");
    }
}
