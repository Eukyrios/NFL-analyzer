//! App state and navigation, kept free of drawing code so it can be tested
//! without a window.

use std::collections::HashMap;

use nfl_core::dal::{Db, Game, Play, PlayTracking};
use nfl_core::filter::{Dim, Facet, FacetOption, PlayFilter, Side};
use nfl_core::geometry::{dropback_span, frame_at, hit_test, phase_markers, Marker, Playback, Viewport};
use nfl_core::roster::{Lineup, PlayerGame, RosterEntry, TeamGame};
use nfl_core::stats::PlayerStats;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Filters,
    Map,
    Team { abbr: String },
    Viewer { game_id: i64, play_id: i64 },
    Player { nfl_id: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TeamTab {
    #[default]
    Lineup,
    Roster,
    Games,
}

/// Name, position and PFF role for a tracked player (hover tooltip, rings).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityInfo {
    pub name: String,
    pub position: String,
}

pub struct ViewerData {
    pub play: Play,
    pub game: Game,
    pub tracking: Option<PlayTracking>,
    pub info: HashMap<i64, EntityInfo>,
    pub playback: Playback,
    pub markers: Vec<Marker>,
    pub span: Option<(usize, usize)>,
    pub snap: usize,
}

impl ViewerData {
    pub fn home(&self) -> &str {
        &self.game.home
    }
}

pub struct TeamData {
    pub abbr: String,
    pub games: Vec<TeamGame>,
    pub roster: Vec<RosterEntry>,
    pub lineup: Lineup,
    pub tab: TeamTab,
}

impl TeamData {
    /// `(wins, losses, ties)` over games with a known final score.
    pub fn record(&self) -> (u32, u32, u32) {
        self.games.iter().filter_map(|g| g.score).fold((0, 0, 0), |(w, l, t), (a, b)| match a.cmp(&b) {
            std::cmp::Ordering::Greater => (w + 1, l, t),
            std::cmp::Ordering::Less => (w, l + 1, t),
            std::cmp::Ordering::Equal => (w, l, t + 1),
        })
    }
}

pub struct PlayerData {
    pub stats: PlayerStats,
    pub games: Vec<PlayerGame>,
}

/// Dropdowns whose counts are kept up to date on every filter change.
pub const FACETS: [Dim; 11] = [
    Dim::Week,
    Dim::Game,
    Dim::Team,
    Dim::Side,
    Dim::PassResult,
    Dim::DropbackType,
    Dim::Down,
    Dim::Quarter,
    Dim::Coverage,
    Dim::CoverageType,
    Dim::PlayAction,
];

pub struct AppState {
    pub db: Option<Db>,
    /// Fatal problem (missing DB, wrong schema): shown instead of the app.
    pub fatal: Option<String>,
    /// Last non-fatal error, shown as a banner.
    pub error: Option<String>,
    stack: Vec<Screen>,
    pub filter: PlayFilter,
    pub plays: Vec<Play>,
    pub facets: HashMap<Dim, Facet>,
    pub games: HashMap<i64, Game>,
    pub picker_team: Option<String>,
    pub picker_position: Option<String>,
    pub positions: Vec<FacetOption>,
    pub players: Vec<FacetOption>,
    pub selected_play: Option<(i64, i64)>,
    pub viewer: Option<ViewerData>,
    pub team: Option<TeamData>,
    pub player: Option<PlayerData>,
    pub trails: bool,
    pub color_by_role: bool,
    /// State selected on the Teams map (two-letter code).
    pub map_state: Option<String>,
}

impl AppState {
    pub fn new(db: Result<Db, String>) -> Self {
        let (db, fatal) = match db {
            Ok(db) => (Some(db), None),
            Err(e) => (None, Some(e)),
        };
        let mut s = AppState {
            db,
            fatal,
            error: None,
            stack: vec![Screen::Filters],
            filter: PlayFilter::default(),
            plays: Vec::new(),
            facets: HashMap::new(),
            games: HashMap::new(),
            picker_team: None,
            picker_position: None,
            positions: Vec::new(),
            players: Vec::new(),
            selected_play: None,
            viewer: None,
            team: None,
            player: None,
            trails: false,
            color_by_role: false,
            map_state: None,
        };
        if let Some(db) = &s.db {
            match db.games() {
                Ok(g) => s.games = g.into_iter().map(|g| (g.game_id, g)).collect(),
                Err(e) => s.fatal = Some(e.to_string()),
            }
        }
        s.refresh();
        s
    }

    pub fn screen(&self) -> &Screen {
        self.stack.last().unwrap_or(&Screen::Filters)
    }

    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    fn report<T>(&mut self, r: nfl_core::Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        }
    }

    /// Back one screen; the root (Filters or Map) stays.
    pub fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            let top = self.screen().clone();
            self.ensure_loaded(&top);
        }
    }

    /// Switches the root tab (Plays / Teams) and clears the history.
    pub fn set_root(&mut self, s: Screen) {
        self.stack = vec![s];
    }

    fn push(&mut self, s: Screen) {
        if self.screen() != &s {
            self.stack.push(s.clone());
        }
        self.ensure_loaded(&s);
    }

    fn ensure_loaded(&mut self, s: &Screen) {
        match s {
            Screen::Viewer { game_id, play_id } => {
                if self.viewer.as_ref().is_none_or(|v| (v.play.game_id, v.play.play_id) != (*game_id, *play_id)) {
                    self.load_play(*game_id, *play_id);
                }
            }
            Screen::Player { nfl_id } => {
                if self.player.as_ref().is_none_or(|p| p.stats.player.nfl_id != *nfl_id) {
                    self.load_player(*nfl_id);
                }
            }
            Screen::Team { abbr } => {
                if self.team.as_ref().is_none_or(|t| &t.abbr != abbr) {
                    self.load_team(abbr);
                }
            }
            Screen::Filters | Screen::Map => {}
        }
    }

    // ---------- filters ----------

    pub fn set_filter(&mut self, d: Dim, v: Option<String>) {
        self.filter.set(d, v);
        self.refresh();
    }

    pub fn set_side(&mut self, side: Side) {
        self.filter.side = side;
        self.refresh();
    }

    pub fn clear_filters(&mut self) {
        self.filter = PlayFilter::default();
        self.picker_team = None;
        self.picker_position = None;
        self.refresh();
    }

    pub fn set_picker_team(&mut self, t: Option<String>) {
        self.picker_team = t;
        self.picker_position = None;
        self.refresh_picker();
    }

    pub fn set_picker_position(&mut self, p: Option<String>) {
        self.picker_position = p;
        self.refresh_picker();
    }

    /// Re-runs the play query and every facet's counts.
    pub fn refresh(&mut self) {
        let Some(db) = &self.db else { return };
        let plays = db.query_plays(&self.filter);
        let facets: nfl_core::Result<HashMap<Dim, Facet>> =
            FACETS.iter().map(|&d| Ok((d, db.facet(d, &self.filter)?))).collect();
        if let Some(p) = self.report(plays) {
            self.plays = p;
        }
        if let Some(f) = self.report(facets) {
            self.facets = f;
        }
        self.refresh_picker();
    }

    fn refresh_picker(&mut self) {
        let Some(db) = &self.db else { return };
        let Some(team) = self.picker_team.clone() else {
            self.positions.clear();
            self.players.clear();
            return;
        };
        let pos = db.position_options(Some(&team), &self.filter);
        let pl = db.player_options(Some(&team), self.picker_position.as_deref(), &self.filter);
        if let Some(p) = self.report(pos) {
            self.positions = p;
        }
        if let Some(p) = self.report(pl) {
            self.players = p;
        }
    }

    /// Shows the play list for one game (optionally one team's or player's plays).
    pub fn show_game_plays(&mut self, game_id: i64, team: Option<&str>, nfl_id: Option<i64>) {
        let mut f = PlayFilter::default();
        if let Some(g) = self.games.get(&game_id) {
            f.values.insert(Dim::Week, g.week.to_string());
        }
        f.values.insert(Dim::Game, game_id.to_string());
        if let Some(t) = team {
            f.values.insert(Dim::Team, t.to_string());
            f.side = Side::Either;
        }
        if let Some(id) = nfl_id {
            f.values.insert(Dim::Player, id.to_string());
        }
        self.filter = f;
        self.refresh();
        self.push(Screen::Filters);
    }

    // ---------- viewer ----------

    pub fn open_play(&mut self, game_id: i64, play_id: i64) {
        self.selected_play = Some((game_id, play_id));
        self.push(Screen::Viewer { game_id, play_id });
    }

    /// Previous/next play in the current list.
    pub fn step_play(&mut self, by: i32) {
        let Some(v) = &self.viewer else { return };
        let key = (v.play.game_id, v.play.play_id);
        let Some(i) = self.plays.iter().position(|p| (p.game_id, p.play_id) == key) else { return };
        let j = i as i64 + by as i64;
        if j < 0 || j as usize >= self.plays.len() {
            return;
        }
        let p = &self.plays[j as usize];
        let (g, pl) = (p.game_id, p.play_id);
        self.selected_play = Some((g, pl));
        if let Some(top) = self.stack.last_mut() {
            *top = Screen::Viewer { game_id: g, play_id: pl };
        }
        self.load_play(g, pl);
    }

    fn load_play(&mut self, game_id: i64, play_id: i64) {
        let Some(db) = &self.db else { return };
        let r = (|| -> nfl_core::Result<ViewerData> {
            let play = db.play(game_id, play_id)?.ok_or("play not found")?;
            let game = db.game(game_id)?.ok_or("game not found")?;
            let tracking = db.tracking(game_id, play_id)?;
            let mut info = HashMap::new();
            let (mut markers, mut span, mut snap, mut frames) = (Vec::new(), None, 0, 0);
            if let Some(t) = &tracking {
                for e in &t.entities {
                    if let Some(id) = e.nfl_id {
                        if let Some(p) = db.player(id)? {
                            info.insert(id, EntityInfo { name: p.name, position: p.position.unwrap_or_default() });
                        }
                    }
                }
                markers = phase_markers(&t.events);
                span = dropback_span(t.summary.snap_frame, t.summary.throw_frame, t.summary.sack_frame);
                snap = (t.summary.snap_frame - 1).max(0) as usize;
                frames = t.frames.len();
            }
            Ok(ViewerData { play, game, tracking, info, playback: Playback::new(frames), markers, span, snap })
        })();
        if let Some(v) = self.report(r) {
            self.viewer = Some(v);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some(v) = &mut self.viewer {
            v.playback.tick(dt);
        }
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.screen(), Screen::Viewer { .. }) && self.viewer.as_ref().is_some_and(|v| v.playback.playing)
    }

    /// Screen positions `(nflId, x, y)` of every entity at the current frame.
    pub fn entity_points(&self, vp: &Viewport) -> Vec<(Option<i64>, f32, f32)> {
        let Some(v) = &self.viewer else { return Vec::new() };
        let Some(t) = &v.tracking else { return Vec::new() };
        let frame = frame_at(&t.frames, v.playback.frame);
        t.entities
            .iter()
            .zip(frame)
            .filter_map(|(e, p)| {
                let p = p?;
                let (x, y) = vp.to_screen(p.x, p.y);
                Some((e.nfl_id, x, y))
            })
            .collect()
    }

    /// The player under a tap, if any.
    pub fn player_at(&self, vp: &Viewport, pos: (f32, f32), radius: f32) -> Option<i64> {
        hit_test(&self.entity_points(vp), pos, radius)
    }

    /// Tap on the field: opens the player under the tap. Returns their id.
    pub fn tap(&mut self, vp: &Viewport, pos: (f32, f32), radius: f32) -> Option<i64> {
        let id = self.player_at(vp, pos, radius)?;
        self.open_player(id);
        Some(id)
    }

    // ---------- player / team ----------

    pub fn open_player(&mut self, nfl_id: i64) {
        self.push(Screen::Player { nfl_id });
    }

    fn load_player(&mut self, nfl_id: i64) {
        let Some(db) = &self.db else { return };
        let r = (|| -> nfl_core::Result<PlayerData> {
            let stats = db.player_stats(nfl_id)?.ok_or("no stats for this player")?;
            Ok(PlayerData { stats, games: db.player_games(nfl_id)? })
        })();
        if let Some(p) = self.report(r) {
            self.player = Some(p);
        }
    }

    pub fn open_team(&mut self, abbr: &str) {
        self.push(Screen::Team { abbr: abbr.to_string() });
    }

    /// Opens a team page directly on one tab (from the map's state panel).
    pub fn open_team_tab(&mut self, abbr: &str, tab: TeamTab) {
        self.open_team(abbr);
        if let Some(t) = self.team.as_mut().filter(|t| t.abbr == abbr) {
            t.tab = tab;
        }
    }

    fn load_team(&mut self, abbr: &str) {
        let Some(db) = &self.db else { return };
        let r = (|| -> nfl_core::Result<TeamData> {
            Ok(TeamData {
                abbr: abbr.to_string(),
                games: db.team_games(abbr)?,
                roster: db.team_roster(abbr)?,
                lineup: db.team_lineup(abbr)?,
                tab: TeamTab::default(),
            })
        })();
        if let Some(t) = self.report(r) {
            self.team = Some(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfl_core::dal::testdata::seeded;

    fn state() -> AppState {
        AppState::new(seeded().map_err(|e| e.to_string()))
    }

    #[test]
    fn default_screen_and_back() {
        let mut s = state();
        assert_eq!(s.screen(), &Screen::Filters);
        assert!(!s.can_go_back());
        s.open_play(1, 1);
        assert_eq!(s.screen(), &Screen::Viewer { game_id: 1, play_id: 1 });
        s.open_player(10);
        assert_eq!(s.screen(), &Screen::Player { nfl_id: 10 });
        s.back();
        assert_eq!(s.screen(), &Screen::Viewer { game_id: 1, play_id: 1 });
        s.back();
        s.back();
        assert_eq!(s.screen(), &Screen::Filters, "root stays");
    }

    #[test]
    fn missing_db_is_fatal_not_a_crash() {
        let s = AppState::new(Err("database not found".into()));
        assert_eq!(s.fatal.as_deref(), Some("database not found"));
        assert!(s.plays.is_empty());
    }

    #[test]
    fn filter_updates_list_and_other_facets() {
        let mut s = state();
        assert_eq!(s.plays.len(), 4);
        let sacks = |s: &AppState| s.facets[&Dim::PassResult].options.iter().find(|o| o.value == "S").unwrap().count;
        assert_eq!(sacks(&s), 1);
        s.set_filter(Dim::Team, Some("LV".into()));
        assert_eq!(s.plays.len(), 3);
        s.set_filter(Dim::Down, Some("3".into()));
        assert_eq!(s.plays.len(), 1);
        assert_eq!(sacks(&s), 0, "other facets follow the filter");
        assert_eq!(s.facets[&Dim::Down].any, 3, "own facet ignores its own value");
        s.clear_filters();
        assert_eq!(s.plays.len(), 4);
    }

    #[test]
    fn playback_through_state() {
        let mut s = state();
        s.open_play(1, 1);
        let v = s.viewer.as_mut().unwrap();
        assert_eq!(v.playback.frames, 3);
        assert_eq!(v.markers[0].label, "Snap");
        v.playback.toggle();
        s.tick(0.1);
        assert!((s.viewer.as_ref().unwrap().playback.frame - 1.0).abs() < 1e-4);
        assert!(s.is_playing());
        s.tick(10.0);
        assert!(!s.is_playing());
    }

    #[test]
    fn neutral_game_flag_reaches_viewer() {
        let mut s = state();
        s.open_play(2, 1);
        assert!(s.viewer.as_ref().unwrap().game.is_neutral());
        s.back();
        s.open_play(1, 1);
        assert!(!s.viewer.as_ref().unwrap().game.is_neutral());
    }

    #[test]
    fn tap_maps_to_player_and_loads_stats() {
        let mut s = state();
        s.open_play(1, 1);
        let vp = Viewport::fit(0.0, 0.0, 1200.0, 533.0, false);
        // Frame 0: QB (10) at (30, 26), rusher (30) at (36, 20).
        let (x, y) = vp.to_screen(36.0, 20.0);
        assert_eq!(s.tap(&vp, (x + 3.0, y - 2.0), 15.0), Some(30));
        assert_eq!(s.screen(), &Screen::Player { nfl_id: 30 });
        assert_eq!(s.player.as_ref().unwrap().stats.role, nfl_core::stats::Role::PassRush);
        s.back();
        let (x, y) = vp.to_screen(30.0, 26.0);
        assert_eq!(s.tap(&vp, (x, y), 15.0), Some(10));
        assert_eq!(s.player.as_ref().unwrap().stats.role, nfl_core::stats::Role::Passer);
        s.back();
        assert_eq!(s.tap(&vp, (5.0, 5.0), 15.0), None, "empty grass");
    }

    #[test]
    fn team_screen_and_game_list() {
        let mut s = state();
        s.set_root(Screen::Map);
        s.open_team("BAL");
        let t = s.team.as_ref().unwrap();
        assert_eq!(t.games.len(), 1);
        assert_eq!(t.record(), (0, 1, 0));
        s.show_game_plays(1, Some("BAL"), None);
        assert_eq!(s.screen(), &Screen::Filters);
        assert_eq!(s.plays.len(), 3);
        s.back();
        assert_eq!(s.screen(), &Screen::Team { abbr: "BAL".into() });
        s.open_player(10);
        assert_eq!(s.player.as_ref().unwrap().games.len(), 1);
    }

    #[test]
    fn step_through_list() {
        let mut s = state();
        s.open_play(1, 1);
        s.step_play(1);
        assert_eq!(s.screen(), &Screen::Viewer { game_id: 1, play_id: 2 });
        s.step_play(-5);
        assert_eq!(s.screen(), &Screen::Viewer { game_id: 1, play_id: 2 }, "out of range is ignored");
        s.back();
        assert_eq!(s.screen(), &Screen::Filters);
    }
}
