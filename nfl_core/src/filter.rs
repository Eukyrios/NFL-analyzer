//! Play filters, the play query and faceted dropdown options.
//!
//! Every facet count is "plays matching all other active filters plus this
//! option", built by the same WHERE builder as `query_plays` with the facet's
//! own dimension left out. That makes single-choice facet counts add up to the
//! "Any" total by construction. All values are bound parameters.

use std::collections::{BTreeMap, HashMap};

use rusqlite::types::Value;
use rusqlite::params_from_iter;

use crate::dal::{play_from_row, Db, Play, PLAY_COLUMNS};
use crate::labels;
use crate::Result;

/// Filter value meaning "column is NULL" (shown as "Unknown").
pub const NULL_VALUE: &str = "NA";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dim {
    Week,
    Game,
    Team,
    Side,
    PassResult,
    DropbackType,
    Down,
    Quarter,
    Coverage,
    CoverageType,
    PlayAction,
    Player,
}

impl Dim {
    /// Dropdowns shown on the filter screen (Team/Side/Player have their own pickers).
    pub const PLAY_STATE: [Dim; 7] =
        [Dim::PassResult, Dim::DropbackType, Dim::Down, Dim::Quarter, Dim::Coverage, Dim::CoverageType, Dim::PlayAction];

    pub fn title(self) -> &'static str {
        match self {
            Dim::Week => "Week",
            Dim::Game => "Game",
            Dim::Team => "Team",
            Dim::Side => "Side",
            Dim::PassResult => "Pass result",
            Dim::DropbackType => "Dropback type",
            Dim::Down => "Down",
            Dim::Quarter => "Quarter",
            Dim::Coverage => "Coverage",
            Dim::CoverageType => "Coverage type",
            Dim::PlayAction => "Play action",
            Dim::Player => "Player",
        }
    }

    fn column(self) -> Option<&'static str> {
        Some(match self {
            Dim::Week => "g.week",
            Dim::Game => "p.gameId",
            Dim::PassResult => "p.passResult",
            Dim::DropbackType => "p.dropBackType",
            Dim::Down => "p.down",
            Dim::Quarter => "p.quarter",
            Dim::Coverage => "p.pff_passCoverage",
            Dim::CoverageType => "p.pff_passCoverageType",
            Dim::PlayAction => "p.pff_playAction",
            Dim::Team | Dim::Side | Dim::Player => return None,
        })
    }

    fn numeric(self) -> bool {
        matches!(self, Dim::Week | Dim::Game | Dim::Down | Dim::Quarter | Dim::PlayAction | Dim::Player)
    }

    fn bind(self, v: &str) -> Value {
        if self.numeric() {
            v.parse::<i64>().map(Value::Integer).unwrap_or(Value::Null)
        } else {
            Value::Text(v.to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    Offense,
    Defense,
    #[default]
    Either,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Offense, Side::Defense, Side::Either];
    pub fn label(self) -> &'static str {
        match self {
            Side::Offense => "Offense",
            Side::Defense => "Defense",
            Side::Either => "Either",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayFilter {
    /// Selected value per dimension (absent = "Any"). `Side` lives in `side`.
    pub values: BTreeMap<Dim, String>,
    pub side: Side,
}

impl PlayFilter {
    pub fn get(&self, d: Dim) -> Option<&str> {
        self.values.get(&d).map(String::as_str)
    }
    pub fn set(&mut self, d: Dim, v: Option<String>) {
        match v {
            Some(v) => self.values.insert(d, v),
            None => self.values.remove(&d),
        };
        // A game outside the selected week makes no sense.
        if d == Dim::Week {
            self.values.remove(&Dim::Game);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FacetOption {
    pub value: String,
    pub label: String,
    pub detail: String,
    pub count: u64,
    /// Team abbreviation whose logo is shown with the option.
    pub team: Option<String>,
    /// Second logo (away team for games).
    pub team2: Option<String>,
}

impl FacetOption {
    fn new(value: impl Into<String>, label: impl Into<String>, count: u64) -> Self {
        FacetOption { value: value.into(), label: label.into(), detail: String::new(), count, team: None, team2: None }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Facet {
    /// Plays matching the other filters (the "Any" option).
    pub any: u64,
    pub options: Vec<FacetOption>,
}

const FROM: &str = "FROM plays p JOIN games g ON g.gameId = p.gameId \
    LEFT JOIN play_summary s ON s.gameId = p.gameId AND s.playId = p.playId";

/// Role-aware team of a scouted player on a play.
const PLAYER_TEAM: &str = "CASE WHEN x.pff_role IN ('Pass Rush', 'Coverage') THEN p.defensiveTeam ELSE p.possessionTeam END";

struct Where {
    sql: String,
    params: Vec<Value>,
}

fn team_clause(side: Side, team: &str, out: &mut Vec<String>, params: &mut Vec<Value>) {
    params.push(Value::Text(team.to_string()));
    let n = params.len();
    out.push(match side {
        Side::Offense => format!("p.possessionTeam = ?{n}"),
        Side::Defense => format!("p.defensiveTeam = ?{n}"),
        Side::Either => format!("(p.possessionTeam = ?{n} OR p.defensiveTeam = ?{n})"),
    });
}

fn build_where(f: &PlayFilter, skip: &[Dim], side: Side) -> Where {
    let mut parts = Vec::new();
    let mut params = Vec::new();
    for (&d, v) in &f.values {
        if skip.contains(&d) {
            continue;
        }
        match d {
            Dim::Team => team_clause(side, v, &mut parts, &mut params),
            Dim::Player => {
                params.push(d.bind(v));
                parts.push(format!(
                    "EXISTS (SELECT 1 FROM pff_scouting y WHERE y.gameId = p.gameId AND y.playId = p.playId AND y.nflId = ?{})",
                    params.len()
                ));
            }
            Dim::Side => {}
            _ => {
                let Some(col) = d.column() else { continue };
                if v == NULL_VALUE {
                    parts.push(format!("{col} IS NULL"));
                } else {
                    params.push(d.bind(v));
                    parts.push(format!("{col} = ?{}", params.len()));
                }
            }
        }
    }
    let sql = if parts.is_empty() { String::new() } else { format!(" WHERE {}", parts.join(" AND ")) };
    Where { sql, params }
}

fn and(w: &Where) -> &'static str {
    if w.sql.is_empty() { " WHERE " } else { " AND " }
}

impl Db {
    pub fn query_plays(&self, f: &PlayFilter) -> Result<Vec<Play>> {
        let w = build_where(f, &[], f.side);
        let sql = format!("SELECT {PLAY_COLUMNS} {FROM}{} ORDER BY p.gameId, p.playId", w.sql);
        let mut s = self.conn.prepare_cached(&sql)?;
        let rows = s.query_map(params_from_iter(w.params), play_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn count_plays(&self, f: &PlayFilter) -> Result<u64> {
        let w = build_where(f, &[], f.side);
        let sql = format!("SELECT COUNT(*) {FROM}{}", w.sql);
        Ok(self.conn.query_row(&sql, params_from_iter(w.params), |r| r.get::<_, i64>(0))? as u64)
    }

    /// `(value or NULL_VALUE, count)` grouped by `expr` under `w`.
    fn grouped(&self, expr: &str, w: &Where) -> Result<HashMap<String, u64>> {
        let sql = format!("SELECT CAST({expr} AS TEXT), COUNT(*) {FROM}{} GROUP BY 1", w.sql);
        let mut s = self.conn.prepare_cached(&sql)?;
        let rows = s.query_map(params_from_iter(w.params.iter()), |r| {
            Ok((r.get::<_, Option<String>>(0)?.unwrap_or_else(|| NULL_VALUE.into()), r.get::<_, i64>(1)? as u64))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn distinct(&self, sql: &str, params: &[Value]) -> Result<Vec<String>> {
        let mut s = self.conn.prepare_cached(sql)?;
        let rows = s.query_map(params_from_iter(params.iter()), |r| {
            Ok(r.get::<_, Option<String>>(0)?.unwrap_or_else(|| NULL_VALUE.into()))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn facet(&self, d: Dim, f: &PlayFilter) -> Result<Facet> {
        let mut skip = vec![d];
        if d == Dim::Side {
            skip = vec![];
        }
        let w = build_where(f, &skip, f.side);
        let any = {
            let sql = format!("SELECT COUNT(*) {FROM}{}", w.sql);
            self.conn.query_row(&sql, params_from_iter(w.params.iter()), |r| r.get::<_, i64>(0))? as u64
        };
        let mut options = match d {
            Dim::Week => self.week_options(&w)?,
            Dim::Game => self.game_options(&w, f.get(Dim::Week))?,
            Dim::Team => self.team_options(f)?,
            Dim::Side => Side::ALL
                .iter()
                .map(|&s| {
                    let w = build_where(f, &[], s);
                    let sql = format!("SELECT COUNT(*) {FROM}{}", w.sql);
                    let n = self.conn.query_row(&sql, params_from_iter(w.params), |r| r.get::<_, i64>(0))? as u64;
                    Ok(FacetOption::new(s.label(), s.label(), n))
                })
                .collect::<Result<Vec<_>>>()?,
            Dim::Player => Vec::new(),
            _ => {
                let col = d.column().ok_or("no column")?;
                let counts = self.grouped(col, &w)?;
                let sql = format!("SELECT DISTINCT CAST({col} AS TEXT) {FROM}");
                let mut universe = self.distinct(&sql, &[])?;
                sort_universe(d, &mut universe, &counts);
                universe.into_iter().map(|v| option_for(d, &v, *counts.get(&v).unwrap_or(&0), any)).collect()
            }
        };
        // Zero-count options stay visible but go last (stable).
        options.sort_by_key(|o| o.count == 0);
        Ok(Facet { any, options })
    }

    fn week_options(&self, w: &Where) -> Result<Vec<FacetOption>> {
        let sql = format!("SELECT g.week, COUNT(DISTINCT p.gameId), COUNT(*) {FROM}{} GROUP BY g.week", w.sql);
        let mut s = self.conn.prepare_cached(&sql)?;
        let counts: HashMap<i64, (u64, u64)> = s
            .query_map(params_from_iter(w.params.iter()), |r| {
                Ok((r.get(0)?, (r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64)))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut s = self.conn.prepare_cached("SELECT DISTINCT week FROM games ORDER BY week")?;
        let weeks: Vec<i64> = s.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(weeks
            .into_iter()
            .map(|wk| {
                let (g, n) = counts.get(&wk).copied().unwrap_or((0, 0));
                let mut o = FacetOption::new(wk.to_string(), format!("Week {wk}"), n);
                o.detail = format!("{g} games · {} plays", labels::thousands(n));
                o
            })
            .collect())
    }

    fn game_options(&self, w: &Where, week: Option<&str>) -> Result<Vec<FacetOption>> {
        let counts = self.grouped("p.gameId", w)?;
        let games = self.games()?;
        Ok(games
            .into_iter()
            .filter(|g| week.is_none_or(|wk| g.week.to_string() == wk))
            .map(|g| {
                let n = *counts.get(&g.game_id.to_string()).unwrap_or(&0);
                let mut o = FacetOption::new(g.game_id.to_string(), g.matchup(), n);
                let mut detail = vec![g.kickoff_label()];
                detail.extend(g.final_label());
                if g.is_neutral() {
                    detail.push(format!("Neutral · {}", g.stadium.clone().unwrap_or_default()));
                }
                detail.push(format!("{} plays", labels::thousands(n)));
                o.detail = detail.join(" · ");
                // Logos: away first, home second (neutral: listed home first).
                if g.is_neutral() {
                    o.team = Some(g.home.clone());
                    o.team2 = Some(g.away.clone());
                } else {
                    o.team = Some(g.away.clone());
                    o.team2 = Some(g.home.clone());
                }
                o
            })
            .collect())
    }

    fn team_options(&self, f: &PlayFilter) -> Result<Vec<FacetOption>> {
        let w = build_where(f, &[Dim::Team], f.side);
        let off = self.grouped("p.possessionTeam", &w)?;
        let def = self.grouped("p.defensiveTeam", &w)?;
        let mut s = self.conn.prepare_cached("SELECT abbr, name FROM teams ORDER BY abbr")?;
        let teams: Vec<(String, String)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(teams
            .into_iter()
            .map(|(abbr, name)| {
                let (o, d) = (*off.get(&abbr).unwrap_or(&0), *def.get(&abbr).unwrap_or(&0));
                let n = match f.side {
                    Side::Offense => o,
                    Side::Defense => d,
                    Side::Either => o + d,
                };
                let mut opt = FacetOption::new(abbr.clone(), format!("{abbr}  {name}"), n);
                opt.detail = format!("{} off · {} def", labels::thousands(o), labels::thousands(d));
                opt.team = Some(abbr);
                opt
            })
            .collect())
    }

    /// Player picker step 2: positions with the number of distinct players
    /// who have plays under the current filters (player filter ignored).
    pub fn position_options(&self, team: Option<&str>, f: &PlayFilter) -> Result<Vec<FacetOption>> {
        let (rows, all) = self.player_rows(team, None, f)?;
        let mut by_pos: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        for r in &all {
            by_pos.entry(r.position.clone()).or_default().1 += 1;
        }
        for r in &rows {
            by_pos.entry(r.position.clone()).or_default().0 += 1;
        }
        let mut out: Vec<FacetOption> = by_pos
            .into_iter()
            .map(|(pos, (n, _))| {
                let mut o = FacetOption::new(pos.clone(), pos.clone(), n);
                o.detail = format!("{} · {n} players", labels::position(&pos));
                o
            })
            .collect();
        out.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
        out.sort_by_key(|o| o.count == 0);
        Ok(out)
    }

    /// Player picker step 3: players sorted by snaps under the current filters.
    pub fn player_options(&self, team: Option<&str>, position: Option<&str>, f: &PlayFilter) -> Result<Vec<FacetOption>> {
        let (rows, all) = self.player_rows(team, position, f)?;
        let snaps: HashMap<i64, &PlayerRow> = rows.iter().map(|r| (r.nfl_id, r)).collect();
        let mut out: Vec<FacetOption> = all
            .iter()
            .map(|a| {
                let r = snaps.get(&a.nfl_id).copied();
                let n = r.map_or(0, |r| r.snaps);
                let jersey = a.jersey.map(|j| format!("#{j} ")).unwrap_or_default();
                let mut o = FacetOption::new(a.nfl_id.to_string(), format!("{jersey}{}", a.name), n);
                o.detail = format!("{} · {} · {} snaps", a.position, labels::role(&a.role), labels::thousands(n));
                o.team = Some(a.team.clone());
                o
            })
            .collect();
        out.sort_by(|a, b| b.count.cmp(&a.count).then(a.label.cmp(&b.label)));
        out.sort_by_key(|o| o.count == 0);
        Ok(out)
    }

    /// Players under the filter (`rows`) and under no play filter (`all`, the universe).
    fn player_rows(&self, team: Option<&str>, position: Option<&str>, f: &PlayFilter) -> Result<(Vec<PlayerRow>, Vec<PlayerRow>)> {
        let mut no_filter = PlayFilter::default();
        no_filter.side = f.side;
        let rows = self.player_rows_where(team, position, f)?;
        let all = self.player_rows_where(team, position, &no_filter)?;
        Ok((rows, all))
    }

    fn player_rows_where(&self, team: Option<&str>, position: Option<&str>, f: &PlayFilter) -> Result<Vec<PlayerRow>> {
        let mut w = build_where(f, &[Dim::Player], f.side);
        let mut extra = String::new();
        if let Some(t) = team {
            w.params.push(Value::Text(t.into()));
            extra += &format!("{}{PLAYER_TEAM} = ?{}", and(&w), w.params.len());
        }
        if let Some(pos) = position {
            w.params.push(Value::Text(pos.into()));
            let glue = if w.sql.is_empty() && extra.is_empty() { " WHERE " } else { " AND " };
            extra += &format!("{glue}pl.officialPosition = ?{}", w.params.len());
        }
        let sql = format!(
            "SELECT x.nflId, COALESCE(pl.displayName, ''), COALESCE(pl.officialPosition, '?'), x.pff_role, {PLAYER_TEAM}, COUNT(*) \
             {FROM} JOIN pff_scouting x ON x.gameId = p.gameId AND x.playId = p.playId \
             LEFT JOIN players pl ON pl.nflId = x.nflId{}{extra} GROUP BY x.nflId, x.pff_role, 5",
            w.sql
        );
        let mut s = self.conn.prepare_cached(&sql)?;
        let mut acc: BTreeMap<i64, (PlayerRow, HashMap<String, u64>, HashMap<String, u64>)> = BTreeMap::new();
        let rows = s.query_map(params_from_iter(w.params.iter()), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get::<_, i64>(5)? as u64,
            ))
        })?;
        for row in rows {
            let (id, name, pos, role, tm, n) = row?;
            let e = acc.entry(id).or_insert_with(|| {
                (PlayerRow { nfl_id: id, name, position: pos, role: String::new(), team: String::new(), snaps: 0, jersey: None }, HashMap::new(), HashMap::new())
            });
            e.0.snaps += n;
            *e.1.entry(role).or_default() += n;
            *e.2.entry(tm).or_default() += n;
        }
        let mut jersey = self.conn.prepare_cached("SELECT jerseyNumber FROM play_entities WHERE nflId = ?1 LIMIT 1")?;
        acc.into_values()
            .map(|(mut r, roles, teams)| {
                r.role = mode(&roles);
                r.team = mode(&teams);
                r.jersey = jersey.query_row([r.nfl_id], |x| x.get(0)).ok().flatten();
                Ok(r)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct PlayerRow {
    nfl_id: i64,
    name: String,
    position: String,
    role: String,
    team: String,
    snaps: u64,
    jersey: Option<i64>,
}

fn mode(m: &HashMap<String, u64>) -> String {
    m.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(k, _)| k.clone()).unwrap_or_default()
}

fn sort_universe(d: Dim, u: &mut [String], counts: &HashMap<String, u64>) {
    const RESULTS: [&str; 5] = ["C", "I", "S", "IN", "R"];
    match d {
        Dim::PassResult => u.sort_by_key(|v| RESULTS.iter().position(|r| r == v).unwrap_or(99)),
        Dim::Down | Dim::Quarter | Dim::PlayAction => u.sort_by_key(|v| v.parse::<i64>().unwrap_or(i64::MAX)),
        _ => u.sort_by(|a, b| {
            let (ca, cb) = (counts.get(a).unwrap_or(&0), counts.get(b).unwrap_or(&0));
            (a == NULL_VALUE).cmp(&(b == NULL_VALUE)).then(cb.cmp(ca)).then(a.cmp(b))
        }),
    }
}

fn option_for(d: Dim, v: &str, n: u64, any: u64) -> FacetOption {
    let unknown = v == NULL_VALUE;
    let label = if unknown {
        "Unknown".to_string()
    } else {
        match d {
            Dim::PassResult => labels::pass_result(v).to_string(),
            Dim::DropbackType => labels::title_case(v),
            Dim::Down => v.parse().map(labels::down).unwrap_or_else(|_| v.into()),
            Dim::Quarter => v.parse().map(labels::quarter).unwrap_or_else(|_| v.into()),
            Dim::PlayAction => if v == "1" { "Yes".into() } else { "No".into() },
            _ => v.to_string(),
        }
    };
    let mut o = FacetOption::new(v, label, n);
    o.detail = match d {
        Dim::PassResult if any > 0 => format!("{} · {}%", labels::thousands(n), (n * 100 + any / 2) / any),
        Dim::Coverage if !unknown => {
            format!("{} · {} · {} plays", labels::coverage_kind(v), labels::coverage_description(v), labels::thousands(n))
        }
        _ => format!("{} plays", labels::thousands(n)),
    };
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::testdata::seeded;
    use proptest::prelude::*;

    fn with(pairs: &[(Dim, &str)]) -> PlayFilter {
        let mut f = PlayFilter::default();
        for (d, v) in pairs {
            f.values.insert(*d, v.to_string());
        }
        f
    }

    #[test]
    fn each_dimension() {
        let db = seeded().unwrap();
        let n = |f: PlayFilter| db.count_plays(&f).unwrap();
        assert_eq!(n(PlayFilter::default()), 4);
        assert_eq!(n(with(&[(Dim::Week, "1")])), 3);
        assert_eq!(n(with(&[(Dim::Game, "2")])), 1);
        assert_eq!(n(with(&[(Dim::Team, "BAL")])), 3);
        let mut off = with(&[(Dim::Team, "BAL")]);
        off.side = Side::Offense;
        assert_eq!(n(off.clone()), 2);
        off.side = Side::Defense;
        assert_eq!(n(off), 1);
        assert_eq!(n(with(&[(Dim::PassResult, "S")])), 1);
        assert_eq!(n(with(&[(Dim::DropbackType, "DESIGNED_ROLLOUT_RIGHT")])), 1);
        assert_eq!(n(with(&[(Dim::Down, "1")])), 2);
        assert_eq!(n(with(&[(Dim::Quarter, "2")])), 1);
        assert_eq!(n(with(&[(Dim::Coverage, "Cover-1")])), 2);
        assert_eq!(n(with(&[(Dim::Coverage, NULL_VALUE)])), 1);
        assert_eq!(n(with(&[(Dim::CoverageType, "Man")])), 2);
        assert_eq!(n(with(&[(Dim::PlayAction, "1")])), 1);
        assert_eq!(n(with(&[(Dim::Player, "30")])), 2);
        assert_eq!(n(with(&[(Dim::Player, "30"), (Dim::PassResult, "S")])), 1);
        assert_eq!(db.query_plays(&with(&[(Dim::Coverage, "Cover-1"), (Dim::Down, "3")])).unwrap()[0].play_id, 3);
    }

    #[test]
    fn labels_and_zero_last() {
        let db = seeded().unwrap();
        let f = with(&[(Dim::Game, "1"), (Dim::Down, "1")]);
        let pr = db.facet(Dim::PassResult, &f).unwrap();
        assert_eq!(pr.any, 1);
        assert_eq!(pr.options[0].label, "Complete");
        assert_eq!(pr.options[0].detail, "1 · 100%");
        assert!(pr.options[1..].iter().all(|o| o.count == 0));
        let dt = db.facet(Dim::DropbackType, &PlayFilter::default()).unwrap();
        assert!(dt.options.iter().any(|o| o.label == "Designed rollout right"));
        let q = db.facet(Dim::Quarter, &PlayFilter::default()).unwrap();
        assert_eq!(q.options[0].label, "Q1");
        let d = db.facet(Dim::Down, &PlayFilter::default()).unwrap();
        assert_eq!(d.options[0].label, "1st");
        let g = db.facet(Dim::Game, &PlayFilter::default()).unwrap();
        assert!(g.options.iter().any(|o| o.label == "ATL vs NYJ" && o.detail.contains("Neutral · Tottenham Stadium")));
        let w = db.facet(Dim::Week, &PlayFilter::default()).unwrap();
        assert_eq!(w.options[0].detail, "1 games · 3 plays");
        let t = db.facet(Dim::Team, &PlayFilter::default()).unwrap();
        let bal = t.options.iter().find(|o| o.value == "BAL").unwrap();
        assert_eq!(bal.detail, "2 off · 1 def");
    }

    #[test]
    fn player_picker() {
        let db = seeded().unwrap();
        let pos = db.position_options(Some("LV"), &PlayFilter::default()).unwrap();
        assert!(pos.iter().any(|o| o.value == "DE" && o.count == 1));
        let players = db.player_options(Some("LV"), None, &PlayFilter::default()).unwrap();
        assert_eq!(players[0].label, "#98 Maxx Crosby");
        assert_eq!(players[0].detail, "DE · Pass rush · 2 snaps");
        let f = with(&[(Dim::PassResult, "C")]);
        let players = db.player_options(Some("LV"), None, &f).unwrap();
        let carr = players.iter().find(|o| o.value == "50").unwrap();
        assert_eq!(carr.count, 0);
        assert_eq!(players.last().unwrap().count, 0);
    }

    // Feature: native-play-viewer, Property 4: Facet counts sum to Any
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn facet_counts_sum(week in prop::option::of(prop::sample::select(vec!["1", "5"])),
                            pr in prop::option::of(prop::sample::select(vec!["C", "I", "S"])),
                            down in prop::option::of(prop::sample::select(vec!["1", "2", "3"])),
                            team in prop::option::of(prop::sample::select(vec!["BAL", "LV", "NYJ"])),
                            side in prop::sample::select(vec![Side::Offense, Side::Defense])) {
            let db = seeded().unwrap();
            let mut f = PlayFilter { side, ..Default::default() };
            for (d, v) in [(Dim::Week, week), (Dim::PassResult, pr), (Dim::Down, down), (Dim::Team, team)] {
                if let Some(v) = v { f.values.insert(d, v.to_string()); }
            }
            for d in [Dim::Week, Dim::Game, Dim::Team, Dim::PassResult, Dim::DropbackType, Dim::Down, Dim::Quarter,
                      Dim::Coverage, Dim::CoverageType, Dim::PlayAction] {
                let facet = db.facet(d, &f).unwrap();
                let sum: u64 = facet.options.iter().map(|o| o.count).sum();
                prop_assert_eq!(sum, facet.any, "{:?}", d);
                let mut cleared = f.clone();
                cleared.values.remove(&d);
                prop_assert_eq!(facet.any, db.count_plays(&cleared).unwrap());
                let first_zero = facet.options.iter().position(|o| o.count == 0).unwrap_or(facet.options.len());
                prop_assert!(facet.options[first_zero..].iter().all(|o| o.count == 0));
            }
        }
    }
}
