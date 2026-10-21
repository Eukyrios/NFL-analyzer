//! The 32 teams. Primary colors equal `TEAM_COLORS` in `timeline/viewer.html`.

use crate::style::{Rgb, UNKNOWN_TEAM};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamInfo {
    pub abbr: &'static str,
    pub name: &'static str,
    pub primary: Rgb,
    pub secondary: Rgb,
}

const fn t(abbr: &'static str, name: &'static str, p: u32, s: u32) -> TeamInfo {
    TeamInfo {
        abbr,
        name,
        primary: ((p >> 16) as u8, (p >> 8) as u8, p as u8),
        secondary: ((s >> 16) as u8, (s >> 8) as u8, s as u8),
    }
}

pub const TEAMS: [TeamInfo; 32] = [
    t("ARI", "Arizona Cardinals", 0x97233F, 0xFFB612),
    t("ATL", "Atlanta Falcons", 0xA71930, 0x000000),
    t("BAL", "Baltimore Ravens", 0x241773, 0x9E7C0C),
    t("BUF", "Buffalo Bills", 0x00338D, 0xC60C30),
    t("CAR", "Carolina Panthers", 0x0085CA, 0x101820),
    t("CHI", "Chicago Bears", 0x0B162A, 0xC83803),
    t("CIN", "Cincinnati Bengals", 0xFB4F14, 0x000000),
    t("CLE", "Cleveland Browns", 0x311D00, 0xFF3C00),
    t("DAL", "Dallas Cowboys", 0x003594, 0x869397),
    t("DEN", "Denver Broncos", 0xFB4F14, 0x002244),
    t("DET", "Detroit Lions", 0x0076B6, 0xB0B7BC),
    t("GB", "Green Bay Packers", 0x203731, 0xFFB612),
    t("HOU", "Houston Texans", 0x03202F, 0xA71930),
    t("IND", "Indianapolis Colts", 0x002C5F, 0xA2AAAD),
    t("JAX", "Jacksonville Jaguars", 0x006778, 0xD7A22A),
    t("KC", "Kansas City Chiefs", 0xE31837, 0xFFB81C),
    t("LA", "Los Angeles Rams", 0x003594, 0xFFA300),
    t("LAC", "Los Angeles Chargers", 0x0080C6, 0xFFC20E),
    t("LV", "Las Vegas Raiders", 0x000000, 0xA5ACAF),
    t("MIA", "Miami Dolphins", 0x008E97, 0xFC4C02),
    t("MIN", "Minnesota Vikings", 0x4F2683, 0xFFC62F),
    t("NE", "New England Patriots", 0x002244, 0xC60C30),
    t("NO", "New Orleans Saints", 0xD3BC8D, 0x101820),
    t("NYG", "New York Giants", 0x0B2265, 0xA71930),
    t("NYJ", "New York Jets", 0x125740, 0xFFFFFF),
    t("PHI", "Philadelphia Eagles", 0x004C54, 0xA5ACAF),
    t("PIT", "Pittsburgh Steelers", 0xFFB612, 0x101820),
    t("SEA", "Seattle Seahawks", 0x002244, 0x69BE28),
    t("SF", "San Francisco 49ers", 0xAA0000, 0xB3995D),
    t("TB", "Tampa Bay Buccaneers", 0xD50A0A, 0x34302B),
    t("TEN", "Tennessee Titans", 0x0C2340, 0x4B92DB),
    t("WAS", "Washington Football Team", 0x5A1414, 0xFFB612),
];

pub fn team(abbr: &str) -> Option<&'static TeamInfo> {
    TEAMS.iter().find(|t| t.abbr == abbr)
}

/// Primary color, or the web app's fallback grey for unknown codes.
pub fn primary(abbr: &str) -> Rgb {
    team(abbr).map_or(UNKNOWN_TEAM, |t| t.primary)
}

/// ESPN CDN code (same mapping as the web app's `logos.rs`).
pub fn espn_code(abbr: &str) -> String {
    match abbr {
        "LA" => "lar".into(),
        "WAS" => "wsh".into(),
        t => t.to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn repo(p: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    #[test]
    fn unique_and_covers_games_csv() {
        let abbrs: HashSet<_> = TEAMS.iter().map(|t| t.abbr).collect();
        assert_eq!(abbrs.len(), 32);
        let Ok(text) = std::fs::read_to_string(repo("data/games.csv")) else { return };
        for line in text.lines().skip(1) {
            let cols: Vec<_> = line.split(',').map(|c| c.trim_matches('"')).collect();
            assert!(abbrs.contains(cols[5]), "missing {}", cols[5]);
            assert!(abbrs.contains(cols[6]), "missing {}", cols[6]);
        }
    }

    /// Parses `TEAM_COLORS` out of the web viewer so the two apps can't drift apart.
    #[test]
    fn primaries_match_web_viewer() {
        let html = std::fs::read_to_string(repo("timeline/viewer.html")).expect("viewer.html");
        let start = html.find("const TEAM_COLORS = {").expect("TEAM_COLORS") + 21;
        let body = &html[start..start + html[start..].find('}').expect("end")];
        let mut n = 0;
        for entry in body.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (k, v) = entry.split_once(':').expect("k:v");
            let rgb = crate::style::hex(v.trim().trim_matches('\'')).expect("hex");
            assert_eq!(team(k.trim()).expect(k).primary, rgb, "{k}");
            n += 1;
        }
        assert_eq!(n, 32);
    }

    #[test]
    fn espn_codes() {
        assert_eq!(espn_code("LA"), "lar");
        assert_eq!(espn_code("WAS"), "wsh");
        assert_eq!(espn_code("KC"), "kc");
    }
}
