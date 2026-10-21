//! Imports the Big Data Bowl CSVs into SQLite: one table per file, with every
//! `tracking_<gameId>.csv` combined into a single `tracking` table. Column names
//! match the CSV headers (see the dataset README); `NA` becomes NULL.
//!
//! `games` also gets `location` (Home/Neutral) and `stadium` from the nflverse
//! schedule, so neutral-site games such as London are detected automatically.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{params, params_from_iter, types::Value, Connection, Transaction};

use crate::game::Res;

/// nflverse schedule; its `old_game_id` matches the Big Data Bowl `gameId`.
const SCHEDULE_URL: &str = "https://raw.githubusercontent.com/nflverse/nfldata/master/data/games.csv";

const TABLES: [(&str, &str); 4] = [
    ("games", "games.csv"),
    ("plays", "plays.csv"),
    ("players", "players.csv"),
    ("pff_scouting", "pffScoutingData.csv"),
];

const SCHEMA_EXTRAS: &str = "
    CREATE UNIQUE INDEX games_id ON games(gameId);
    CREATE UNIQUE INDEX plays_id ON plays(gameId, playId);
    CREATE UNIQUE INDEX players_id ON players(nflId);
    CREATE INDEX pff_scouting_play ON pff_scouting(gameId, playId);
    CREATE INDEX tracking_play ON tracking(gameId, playId, frameId);
    CREATE TABLE logos (team TEXT PRIMARY KEY, png BLOB NOT NULL);
";

pub fn open(path: &Path) -> Res<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

/// Stores integers and decimals as numbers so SQL comparisons and math work.
fn cell(v: &str) -> Value {
    if v == "NA" || v.is_empty() {
        return Value::Null;
    }
    if let Ok(i) = v.parse::<i64>() {
        return Value::Integer(i);
    }
    if v.bytes().any(|b| b.is_ascii_digit()) {
        if let Ok(f) = v.parse::<f64>() {
            return Value::Real(f);
        }
    }
    Value::Text(v.to_string())
}

fn import_csv(tx: &Transaction, table: &str, paths: &[PathBuf]) -> Res<()> {
    let start = Instant::now();
    let mut insert = None;
    let mut rows = 0u64;
    for path in paths {
        let mut rdr = csv::Reader::from_path(path)?;
        if insert.is_none() {
            let headers = rdr.headers()?;
            let cols: Vec<String> = headers.iter().map(|h| format!("\"{h}\"")).collect();
            tx.execute_batch(&format!("CREATE TABLE {table} ({})", cols.join(", ")))?;
            let marks = vec!["?"; cols.len()].join(", ");
            insert = Some(tx.prepare(&format!("INSERT INTO {table} VALUES ({marks})"))?);
        }
        let insert = insert.as_mut().expect("prepared above");
        let mut rec = csv::StringRecord::new();
        while rdr.read_record(&mut rec)? {
            insert.execute(params_from_iter(rec.iter().map(cell)))?;
            rows += 1;
        }
    }
    println!("  {table:<13} {rows:>9} rows  {:.1}s", start.elapsed().as_secs_f64());
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Res<bool> {
    let mut stmt = conn.prepare("SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2")?;
    Ok(stmt.exists([table, column])?)
}

/// Adds `location` and `stadium` to `games` from the nflverse schedule. Safe to call on
/// every start: it only downloads while the columns are still empty, and a failed
/// download just leaves every game treated as a home game.
pub fn add_schedule(conn: &Connection) -> Res<()> {
    for column in ["location", "stadium"] {
        if !has_column(conn, "games", column)? {
            conn.execute_batch(&format!("ALTER TABLE games ADD COLUMN {column}"))?;
        }
    }
    let filled: i64 = conn.query_row("SELECT COUNT(*) FROM games WHERE location IS NOT NULL", [], |r| r.get(0))?;
    if filled == 0 {
        if let Err(e) = fill_schedule(conn) {
            eprintln!("warning: couldn't load the nflverse schedule ({e}); every game is treated as a home game");
        }
    }
    Ok(())
}

fn fill_schedule(conn: &Connection) -> Res<()> {
    let start = Instant::now();
    let mut rdr = csv::Reader::from_reader(ureq::get(SCHEDULE_URL).call()?.into_reader());
    let headers = rdr.headers()?.clone();
    let col = |name: &str| headers.iter().position(|h| h == name).ok_or(format!("schedule has no {name} column"));
    let (id, location, stadium) = (col("old_game_id")?, col("location")?, col("stadium")?);
    let value = |v: &str| (!matches!(v, "NA" | "")).then(|| v.to_string());

    let tx = conn.unchecked_transaction()?;
    let mut matched = 0;
    {
        let mut update = tx.prepare("UPDATE games SET location = ?2, stadium = ?3 WHERE gameId = ?1")?;
        for rec in rdr.records() {
            let rec = rec?;
            if let Ok(gid) = rec[id].parse::<i64>() {
                matched += update.execute(params![gid, value(&rec[location]), value(&rec[stadium])])?;
            }
        }
    }
    tx.commit()?;
    println!("  schedule      {matched} games matched (nflverse)  {:.1}s", start.elapsed().as_secs_f64());
    Ok(())
}

/// (Re)builds the database at `db_path`; the old file is only replaced once the import succeeds.
pub fn import(data_dir: &Path, db_path: &Path) -> Res<()> {
    println!("Importing {} into {}", data_dir.display(), db_path.display());
    let tmp = db_path.with_extension("db.importing");
    let _ = fs::remove_file(&tmp);

    let mut conn = Connection::open(&tmp)?;
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    let tx = conn.transaction()?;
    for (table, file) in TABLES {
        import_csv(&tx, table, &[data_dir.join(file)])?;
    }

    let mut tracking: Vec<PathBuf> = fs::read_dir(data_dir.join("tracking"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    tracking.sort();
    println!("  tracking: {} files…", tracking.len());
    import_csv(&tx, "tracking", &tracking)?;

    let start = Instant::now();
    tx.execute_batch(SCHEMA_EXTRAS)?;
    tx.commit()?;
    println!("  indexes       {:.1}s", start.elapsed().as_secs_f64());
    add_schedule(&conn)?;
    drop(conn);

    fs::rename(&tmp, db_path)?;
    println!("Database ready ({:.0} MB)", fs::metadata(db_path)?.len() as f64 / 1e6);
    Ok(())
}
