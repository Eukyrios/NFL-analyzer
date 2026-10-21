//! Flat CSVs → tables. Ported from the web app's `src/db.rs` importer, but into
//! the typed schema from `nfl_core::schema` instead of header-derived tables.

use std::path::Path;
use std::time::Instant;

use rusqlite::{params, params_from_iter, types::Value, Connection};

use crate::Result;

pub const FILES: [(&str, &str); 4] = [
    ("games", "games.csv"),
    ("players", "players.csv"),
    ("plays", "plays.csv"),
    ("pff_scouting", "pffScoutingData.csv"),
];

/// `NA`, empty and tracking's `None` become NULL; numbers are stored as numbers.
pub fn cell(v: &str) -> Value {
    if matches!(v, "NA" | "" | "None") {
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

/// Heights like `6-4` must stay text; the generic `cell` would leave them alone,
/// but dates like `1977-08-03` too. Only the columns below are kept verbatim.
fn keep_text(col: &str) -> bool {
    matches!(col, "height" | "birthDate" | "gameDate" | "gameTimeEastern" | "gameClock" | "personnelO" | "personnelD")
}

/// Loads one CSV in a single transaction. Column names come from the header and
/// must exist in the schema table (checked by SQLite).
pub fn load_table(conn: &mut Connection, table: &str, path: &Path) -> Result<u64> {
    let start = Instant::now();
    let mut rdr = csv::Reader::from_path(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let headers: Vec<String> = rdr.headers()?.iter().map(str::to_string).collect();
    let known: Vec<String> = {
        let mut s = conn.prepare("SELECT name FROM pragma_table_info(?1)")?;
        let names = s.query_map([table], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        names
    };
    for h in &headers {
        if !known.contains(h) {
            return Err(format!("{}: unexpected column {h}", path.display()).into());
        }
    }
    let cols: Vec<String> = headers.iter().map(|h| format!("\"{h}\"")).collect();
    let marks = vec!["?"; cols.len()].join(", ");
    let tx = conn.transaction()?;
    let mut rows = 0u64;
    {
        let mut insert = tx.prepare(&format!("INSERT INTO {table} ({}) VALUES ({marks})", cols.join(", ")))?;
        let mut rec = csv::StringRecord::new();
        while rdr.read_record(&mut rec)? {
            let vals = rec.iter().zip(&headers).map(|(v, h)| {
                if keep_text(h) && !matches!(v, "NA" | "") { Value::Text(v.to_string()) } else { cell(v) }
            });
            insert.execute(params_from_iter(vals))?;
            rows += 1;
        }
    }
    tx.commit()?;
    println!("  {table:<13} {rows:>9} rows  {:.1}s", start.elapsed().as_secs_f64());
    Ok(rows)
}

/// The built-in 32-team table (logos are added later by `enrich`).
pub fn load_teams(conn: &Connection) -> Result<()> {
    let hex = |(r, g, b): (u8, u8, u8)| format!("#{r:02X}{g:02X}{b:02X}");
    let mut s = conn.prepare("INSERT INTO teams (abbr, name, primary_color, secondary_color) VALUES (?1, ?2, ?3, ?4)")?;
    for t in nfl_core::teams::TEAMS {
        s.execute(params![t.abbr, t.name, hex(t.primary), hex(t.secondary)])?;
    }
    Ok(())
}
