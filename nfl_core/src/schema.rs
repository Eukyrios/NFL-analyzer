//! Embedded DDL for the compact, read-only `nfl.sqlite`.

use rusqlite::Connection;

/// Stored in `PRAGMA user_version`; the app refuses a DB with another value.
pub const SCHEMA_VERSION: i32 = 1;

pub const DDL: &str = r#"
CREATE TABLE games (
    gameId INTEGER PRIMARY KEY, season INTEGER, week INTEGER, gameDate TEXT, gameTimeEastern TEXT,
    homeTeamAbbr TEXT, visitorTeamAbbr TEXT,
    location TEXT, stadium TEXT, home_score INTEGER, away_score INTEGER, overtime INTEGER
);
CREATE TABLE players (
    nflId INTEGER PRIMARY KEY, height TEXT, weight INTEGER, birthDate TEXT, collegeName TEXT,
    officialPosition TEXT, displayName TEXT
);
CREATE TABLE plays (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, playDescription TEXT, quarter INTEGER, down INTEGER,
    yardsToGo INTEGER, possessionTeam TEXT, defensiveTeam TEXT, yardlineSide TEXT, yardlineNumber INTEGER,
    gameClock TEXT, preSnapHomeScore INTEGER, preSnapVisitorScore INTEGER, passResult TEXT,
    penaltyYards INTEGER, prePenaltyPlayResult INTEGER, playResult INTEGER,
    foulName1 TEXT, foulNFLId1 INTEGER, foulName2 TEXT, foulNFLId2 INTEGER, foulName3 TEXT, foulNFLId3 INTEGER,
    absoluteYardlineNumber INTEGER, offenseFormation TEXT, personnelO TEXT, defendersInBox INTEGER,
    personnelD TEXT, dropBackType TEXT, pff_playAction INTEGER, pff_passCoverage TEXT, pff_passCoverageType TEXT,
    PRIMARY KEY (gameId, playId)
);
CREATE TABLE pff_scouting (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, nflId INTEGER NOT NULL, pff_role TEXT,
    pff_positionLinedUp TEXT, pff_hit INTEGER, pff_hurry INTEGER, pff_sack INTEGER,
    pff_beatenByDefender INTEGER, pff_hitAllowed INTEGER, pff_hurryAllowed INTEGER, pff_sackAllowed INTEGER,
    pff_nflIdBlockedPlayer INTEGER, pff_blockType TEXT, pff_backFieldBlock INTEGER,
    PRIMARY KEY (gameId, playId, nflId)
);
CREATE TABLE teams (
    abbr TEXT PRIMARY KEY, name TEXT NOT NULL, primary_color TEXT NOT NULL, secondary_color TEXT NOT NULL,
    logo_png BLOB
);
CREATE TABLE play_entities (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, slot INTEGER NOT NULL,
    nflId INTEGER, jerseyNumber INTEGER, team TEXT NOT NULL,
    PRIMARY KEY (gameId, playId, slot)
);
CREATE TABLE play_frames (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, frame_count INTEGER NOT NULL, blob BLOB NOT NULL,
    PRIMARY KEY (gameId, playId)
);
CREATE TABLE play_events (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, frameId INTEGER NOT NULL, event TEXT NOT NULL,
    PRIMARY KEY (gameId, playId, frameId, event)
);
CREATE TABLE play_summary (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, snap_frame INTEGER NOT NULL, throw_frame INTEGER,
    sack_frame INTEGER, end_frame INTEGER NOT NULL, time_to_throw REAL,
    PRIMARY KEY (gameId, playId)
);
CREATE TABLE player_play_stats (
    gameId INTEGER NOT NULL, playId INTEGER NOT NULL, nflId INTEGER NOT NULL,
    max_speed REAL, avg_speed REAL, distance REAL,
    PRIMARY KEY (gameId, playId, nflId)
);
CREATE INDEX plays_game ON plays(gameId);
CREATE INDEX plays_offense ON plays(possessionTeam);
CREATE INDEX plays_defense ON plays(defensiveTeam);
CREATE INDEX pff_player ON pff_scouting(nflId);
CREATE INDEX pff_play ON pff_scouting(gameId, playId);
CREATE INDEX pps_player ON player_play_stats(nflId);
CREATE INDEX pe_player ON play_entities(nflId);
"#;

pub const TABLES: [&str; 10] = [
    "games", "players", "plays", "pff_scouting", "teams", "play_entities", "play_frames", "play_events",
    "play_summary", "player_play_stats",
];

pub fn create_all(conn: &Connection) -> crate::Result<()> {
    conn.execute_batch(DDL)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

pub fn version(conn: &Connection) -> crate::Result<i32> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(c: &Connection, t: &str) -> Vec<String> {
        let mut s = c.prepare("SELECT name FROM pragma_table_info(?1)").unwrap();
        s.query_map([t], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn creates_everything() {
        let c = Connection::open_in_memory().unwrap();
        create_all(&c).unwrap();
        assert_eq!(version(&c).unwrap(), SCHEMA_VERSION);
        for t in TABLES {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", [t], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1, "{t}");
        }
        for col in ["location", "stadium", "home_score", "away_score", "overtime"] {
            assert!(columns(&c, "games").contains(&col.to_string()), "{col}");
        }
        assert!(columns(&c, "teams").contains(&"logo_png".to_string()));
        for i in ["plays_game", "plays_offense", "plays_defense", "pff_player", "pff_play", "pps_player"] {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1", [i], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1, "{i}");
        }
    }
}
