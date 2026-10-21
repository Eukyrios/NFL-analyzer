//! Role-based player stats, all from SQL aggregates (no tracking scans).

use rusqlite::params;

use crate::dal::{Db, Player};
use crate::style::{self, Rgb};
use crate::{labels, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Passer,
    PassBlock,
    PassRush,
    Route,
    Coverage,
}

impl Role {
    pub fn from_pff(r: &str) -> Option<Role> {
        Some(match r {
            "Pass" => Role::Passer,
            "Pass Block" => Role::PassBlock,
            "Pass Rush" => Role::PassRush,
            "Pass Route" => Role::Route,
            "Coverage" => Role::Coverage,
            _ => return None,
        })
    }
    /// Fallback when a player has no PFF rows.
    pub fn from_position(p: &str) -> Role {
        match p {
            "QB" => Role::Passer,
            "T" | "G" | "C" => Role::PassBlock,
            "DE" | "DT" | "NT" | "OLB" => Role::PassRush,
            "WR" | "TE" | "RB" | "FB" => Role::Route,
            _ => Role::Coverage,
        }
    }
    pub fn pff(self) -> &'static str {
        match self {
            Role::Passer => "Pass",
            Role::PassBlock => "Pass Block",
            Role::PassRush => "Pass Rush",
            Role::Route => "Pass Route",
            Role::Coverage => "Coverage",
        }
    }
    pub fn label(self) -> &'static str {
        labels::role(self.pff())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub label: String,
    pub value: f64,
    pub color: Rgb,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Breakdown {
    pub title: String,
    pub bars: Vec<Bar>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStats {
    pub player: Player,
    pub team: Option<String>,
    pub role: Role,
    pub snaps: u64,
    /// Headline numbers, e.g. ("Avg time to throw", "2.61 s").
    pub numbers: Vec<(String, String)>,
    pub breakdowns: Vec<Breakdown>,
    pub trend_title: String,
    /// (week, value) for weeks with snaps.
    pub trend: Vec<(f64, f64)>,
}

fn bars(rows: Vec<(String, f64)>, color: impl Fn(&str) -> Rgb, label: impl Fn(&str) -> String) -> Vec<Bar> {
    rows.into_iter().map(|(k, v)| Bar { color: color(&k), label: label(&k), value: v }).collect()
}

impl Db {
    pub fn player_role(&self, nfl_id: i64) -> Result<Option<Role>> {
        let r: Option<String> = self
            .conn
            .query_row(
                "SELECT pff_role FROM pff_scouting WHERE nflId = ?1 GROUP BY pff_role ORDER BY COUNT(*) DESC, pff_role LIMIT 1",
                [nfl_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(role) = r.as_deref().and_then(Role::from_pff) {
            return Ok(Some(role));
        }
        Ok(self.player(nfl_id)?.and_then(|p| p.position).map(|p| Role::from_position(&p)))
    }

    fn pairs(&self, sql: &str, id: i64) -> Result<Vec<(String, f64)>> {
        let mut s = self.conn.prepare_cached(sql)?;
        let rows = s.query_map([id], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_else(|| "Unknown".into()), r.get::<_, f64>(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn trend(&self, sql: &str, id: i64) -> Result<Vec<(f64, f64)>> {
        let mut s = self.conn.prepare_cached(sql)?;
        let rows = s.query_map([id], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn scalar(&self, sql: &str, id: i64) -> Result<f64> {
        Ok(self.conn.query_row(sql, params![id], |r| r.get::<_, Option<f64>>(0))?.unwrap_or(0.0))
    }

    pub fn player_stats(&self, nfl_id: i64) -> Result<Option<PlayerStats>> {
        let Some(player) = self.player(nfl_id)? else { return Ok(None) };
        let Some(role) = self.player_role(nfl_id)? else { return Ok(None) };
        let team = self.player_team(nfl_id)?;
        let snaps = self.scalar("SELECT COUNT(*) FROM pff_scouting WHERE nflId = ?1", nfl_id)? as u64;
        let role_mix = Breakdown {
            title: "Snaps by role".into(),
            bars: bars(
                self.pairs("SELECT pff_role, COUNT(*) FROM pff_scouting WHERE nflId = ?1 GROUP BY 1 ORDER BY 2 DESC", nfl_id)?,
                style::role_color,
                |k| labels::role(k).to_string(),
            ),
        };
        const JOIN: &str = "FROM pff_scouting x JOIN plays p ON p.gameId = x.gameId AND p.playId = x.playId \
            JOIN games g ON g.gameId = x.gameId LEFT JOIN play_summary s ON s.gameId = x.gameId AND s.playId = x.playId";
        let mut numbers = vec![("Snaps".to_string(), labels::thousands(snaps))];
        let mut breakdowns = Vec::new();
        let (trend_title, trend_sql): (&str, String) = match role {
            Role::Passer => {
                let w = format!("{JOIN} WHERE x.nflId = ?1 AND x.pff_role = 'Pass'");
                breakdowns.push(Breakdown {
                    title: "Pass result".into(),
                    bars: bars(
                        self.pairs(&format!("SELECT p.passResult, COUNT(*) {w} GROUP BY 1 ORDER BY 2 DESC"), nfl_id)?,
                        style::result_color,
                        |k| labels::pass_result(k).to_string(),
                    ),
                });
                breakdowns.push(Breakdown {
                    title: "Dropback type".into(),
                    bars: bars(
                        self.pairs(&format!("SELECT p.dropBackType, COUNT(*) {w} GROUP BY 1 ORDER BY 2 DESC"), nfl_id)?,
                        |_| style::BAR_NEUTRAL,
                        labels::title_case,
                    ),
                });
                let ttt = self.scalar(&format!("SELECT AVG(s.time_to_throw) {w}"), nfl_id)?;
                let sacks = self.scalar(&format!("SELECT COUNT(*) {w} AND p.passResult = 'S'"), nfl_id)?;
                let pressures = self.scalar(
                    &format!(
                        "SELECT COUNT(*) {w} AND EXISTS (SELECT 1 FROM pff_scouting d WHERE d.gameId = x.gameId \
                         AND d.playId = x.playId AND (d.pff_hit = 1 OR d.pff_hurry = 1 OR d.pff_sack = 1))"
                    ),
                    nfl_id,
                )?;
                let comp = self.scalar(&format!("SELECT AVG(p.passResult = 'C') * 100 {w} AND p.passResult IN ('C','I','IN')"), nfl_id)?;
                numbers.push(("Avg time to throw".into(), format!("{ttt:.2} s")));
                numbers.push(("Completion".into(), format!("{comp:.0}%")));
                numbers.push(("Sacks".into(), format!("{sacks:.0}")));
                numbers.push(("Pressured".into(), format!("{pressures:.0}")));
                (
                    "Completion % by week",
                    format!("SELECT g.week, AVG(p.passResult = 'C') * 100 {w} AND p.passResult IN ('C','I','IN') GROUP BY 1 ORDER BY 1"),
                )
            }
            Role::PassBlock => {
                let w = format!("{JOIN} WHERE x.nflId = ?1 AND x.pff_role = 'Pass Block'");
                let sum = |c: &str| self.scalar(&format!("SELECT TOTAL({c}) {w}"), nfl_id);
                let (h, u, s, b) = (sum("x.pff_hitAllowed")?, sum("x.pff_hurryAllowed")?, sum("x.pff_sackAllowed")?, sum("x.pff_beatenByDefender")?);
                numbers.push(("Pressures allowed".into(), format!("{:.0}", h + u + s)));
                numbers.push(("Beaten".into(), format!("{b:.0}")));
                breakdowns.push(Breakdown {
                    title: "Allowed".into(),
                    bars: vec![
                        Bar { label: "Hits".into(), value: h, color: style::ROLE_RUSH },
                        Bar { label: "Hurries".into(), value: u, color: style::RESULT_IN },
                        Bar { label: "Sacks".into(), value: s, color: style::RESULT_S },
                        Bar { label: "Beaten".into(), value: b, color: style::BAR_NEUTRAL },
                    ],
                });
                breakdowns.push(Breakdown {
                    title: "Block type".into(),
                    bars: bars(
                        self.pairs(&format!("SELECT x.pff_blockType, COUNT(*) {w} GROUP BY 1 ORDER BY 2 DESC"), nfl_id)?,
                        |_| style::BAR_NEUTRAL,
                        |k| k.to_string(),
                    ),
                });
                (
                    "Pressures allowed by week",
                    format!("SELECT g.week, TOTAL(x.pff_hitAllowed) + TOTAL(x.pff_hurryAllowed) + TOTAL(x.pff_sackAllowed) {w} GROUP BY 1 ORDER BY 1"),
                )
            }
            Role::PassRush => {
                let w = format!("{JOIN} WHERE x.nflId = ?1 AND x.pff_role = 'Pass Rush'");
                let sum = |c: &str| self.scalar(&format!("SELECT TOTAL({c}) {w}"), nfl_id);
                let (h, u, s) = (sum("x.pff_hit")?, sum("x.pff_hurry")?, sum("x.pff_sack")?);
                let rush = self.scalar(&format!("SELECT COUNT(*) {w}"), nfl_id)?;
                let pressured = self.scalar(&format!("SELECT COUNT(*) {w} AND (x.pff_hit = 1 OR x.pff_hurry = 1 OR x.pff_sack = 1)"), nfl_id)?;
                let rate = if rush > 0.0 { pressured / rush * 100.0 } else { 0.0 };
                numbers.push(("Pressure rate".into(), format!("{rate:.1}%")));
                numbers.push(("Sacks".into(), format!("{s:.0}")));
                breakdowns.push(Breakdown {
                    title: "Pressures".into(),
                    bars: vec![
                        Bar { label: "Hits".into(), value: h, color: style::ROLE_RUSH },
                        Bar { label: "Hurries".into(), value: u, color: style::RESULT_IN },
                        Bar { label: "Sacks".into(), value: s, color: style::RESULT_S },
                    ],
                });
                (
                    "Pressures by week",
                    format!("SELECT g.week, SUM(x.pff_hit = 1 OR x.pff_hurry = 1 OR x.pff_sack = 1) {w} GROUP BY 1 ORDER BY 1"),
                )
            }
            Role::Route | Role::Coverage => {
                let max = self.scalar("SELECT MAX(max_speed) FROM player_play_stats WHERE nflId = ?1", nfl_id)?;
                let avg = self.scalar("SELECT AVG(avg_speed) FROM player_play_stats WHERE nflId = ?1", nfl_id)?;
                numbers.push(("Top speed".into(), format!("{max:.1} yd/s")));
                numbers.push(("Avg speed".into(), format!("{avg:.1} yd/s")));
                let title = if role == Role::Route { "Coverage faced" } else { "Coverage played" };
                breakdowns.push(Breakdown {
                    title: title.into(),
                    bars: bars(
                        self.pairs(
                            &format!("SELECT p.pff_passCoverageType, COUNT(*) {JOIN} WHERE x.nflId = ?1 AND x.pff_role = '{}' GROUP BY 1 ORDER BY 2 DESC", role.pff()),
                            nfl_id,
                        )?,
                        |_| style::BAR_NEUTRAL,
                        |k| k.to_string(),
                    ),
                });
                (
                    "Top speed by week (yd/s)",
                    "SELECT g.week, MAX(t.max_speed) FROM player_play_stats t JOIN games g ON g.gameId = t.gameId \
                     WHERE t.nflId = ?1 GROUP BY 1 ORDER BY 1"
                        .to_string(),
                )
            }
        };
        breakdowns.push(role_mix);
        let trend = self.trend(&trend_sql, nfl_id)?;
        Ok(Some(PlayerStats { player, team, role, snaps, numbers, breakdowns, trend_title: trend_title.into(), trend }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::testdata::seeded;

    #[test]
    fn roles() {
        let db = seeded().unwrap();
        assert_eq!(db.player_role(10).unwrap(), Some(Role::Passer));
        assert_eq!(db.player_role(20).unwrap(), Some(Role::PassBlock));
        assert_eq!(db.player_role(30).unwrap(), Some(Role::PassRush));
        assert_eq!(db.player_role(999).unwrap(), None);
    }

    #[test]
    fn qb_stats() {
        let db = seeded().unwrap();
        let s = db.player_stats(10).unwrap().unwrap();
        assert_eq!(s.snaps, 2);
        assert_eq!(s.team.as_deref(), Some("BAL"));
        let num = |k: &str| s.numbers.iter().find(|n| n.0 == k).unwrap().1.clone();
        assert_eq!(num("Avg time to throw"), "2.40 s");
        assert_eq!(num("Sacks"), "1");
        assert_eq!(num("Pressured"), "2");
        assert_eq!(s.breakdowns[0].bars.len(), 2);
        assert_eq!(s.trend, vec![(1.0, 100.0)]);
    }

    #[test]
    fn blocker_and_rusher() {
        let db = seeded().unwrap();
        let b = db.player_stats(20).unwrap().unwrap();
        assert_eq!(b.numbers[1], ("Pressures allowed".into(), "1".into()));
        assert_eq!(b.numbers[2], ("Beaten".into(), "1".into()));
        let r = db.player_stats(30).unwrap().unwrap();
        assert_eq!(r.numbers[1], ("Pressure rate".into(), "100.0%".into()));
        assert_eq!(r.trend, vec![(1.0, 2.0)]);
        let c = db.player_stats(40).unwrap().unwrap();
        assert_eq!(c.role, Role::Coverage);
        assert_eq!(c.numbers[1], ("Top speed".into(), "7.5 yd/s".into()));
    }
}
