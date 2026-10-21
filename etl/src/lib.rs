//! CSV → compact `nfl.sqlite`. Rebuilds from scratch into a temp file and only
//! replaces the output once everything succeeded (same approach as the web
//! app's importer in `src/db.rs`).

pub mod csv_load;
pub mod enrich;
pub mod tracking;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::Connection;

pub type Result<T> = nfl_core::Result<T>;

#[derive(Debug, Clone)]
pub struct Options {
    pub data: PathBuf,
    pub out: PathBuf,
    /// Skip the network enrichment (schedule and logos).
    pub offline: bool,
    /// Local schedule CSV (used by fixtures/tests); read even when offline.
    pub schedule_file: Option<PathBuf>,
    pub copy_to: Option<PathBuf>,
}

pub fn run(o: &Options) -> Result<()> {
    let start = Instant::now();
    println!("Building {} from {}", o.out.display(), o.data.display());
    let tmp = o.out.with_extension("sqlite.building");
    let _ = fs::remove_file(&tmp);
    if let Some(dir) = o.out.parent() {
        fs::create_dir_all(dir)?;
    }
    {
        let mut conn = Connection::open(&tmp)?;
        build(&mut conn, o)?;
        let t = Instant::now();
        conn.execute_batch("VACUUM; ANALYZE;")?;
        println!("  vacuum        {:.1}s", t.elapsed().as_secs_f64());
    }
    fs::rename(&tmp, &o.out)?;
    let size = fs::metadata(&o.out)?.len();
    println!("Database ready: {} ({:.1} MB) in {:.1}s", o.out.display(), size as f64 / 1e6, start.elapsed().as_secs_f64());
    if let Some(dst) = &o.copy_to {
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::copy(&o.out, dst)?;
        println!("Copied to {}", dst.display());
    }
    Ok(())
}

pub fn build(conn: &mut Connection, o: &Options) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    nfl_core::schema::create_all(conn)?;
    csv_load::load_teams(conn)?;
    for (table, file) in csv_load::FILES {
        csv_load::load_table(conn, table, &o.data.join(file))?;
    }
    tracking::load_dir(conn, &o.data.join("tracking"))?;
    if let Some(f) = &o.schedule_file {
        warn(enrich::schedule_from_reader(conn, fs::File::open(f)?), "schedule");
    } else if !o.offline {
        warn(enrich::schedule_online(conn), "nflverse schedule");
    }
    if !o.offline {
        enrich::logos_online(conn);
    }
    report(conn)
}

fn warn(r: Result<()>, what: &str) {
    if let Err(e) = r {
        eprintln!("warning: couldn't load the {what} ({e}); those columns stay NULL");
    }
}

fn report(conn: &Connection) -> Result<()> {
    for t in nfl_core::schema::TABLES {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?;
        println!("  {t:<18} {n:>9} rows");
    }
    Ok(())
}

/// Repo-relative helper for tests.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}
