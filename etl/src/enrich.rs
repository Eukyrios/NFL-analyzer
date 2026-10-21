//! Optional online enrichment: venue and final scores from the nflverse
//! schedule, team logos from ESPN's CDN. Ported from the web app's
//! `src/db.rs::fill_schedule` and `src/logos.rs`, extended with scores,
//! overtime and 128 × 128 downscaling. Failures only warn.

use std::io::{Cursor, Read};
use std::time::Instant;

use image::{imageops::FilterType, ImageFormat};
use rusqlite::{params, Connection};

use crate::Result;

/// nflverse schedule; its `old_game_id` matches the Big Data Bowl `gameId`.
pub const SCHEDULE_URL: &str = "https://raw.githubusercontent.com/nflverse/nfldata/master/data/games.csv";

pub fn schedule_online(conn: &Connection) -> Result<()> {
    let resp = ureq::get(SCHEDULE_URL).timeout(std::time::Duration::from_secs(30)).call()?;
    schedule_from_reader(conn, resp.into_reader())
}

pub fn schedule_from_reader(conn: &Connection, r: impl Read) -> Result<()> {
    let start = Instant::now();
    let mut rdr = csv::Reader::from_reader(r);
    let headers = rdr.headers()?.clone();
    let col = |name: &str| headers.iter().position(|h| h == name).ok_or(format!("schedule has no {name} column"));
    let (id, loc, stadium, hs, aws, ot) =
        (col("old_game_id")?, col("location")?, col("stadium")?, col("home_score")?, col("away_score")?, col("overtime")?);
    let text = |v: &str| (!matches!(v, "NA" | "")).then(|| v.to_string());
    let int = |v: &str| v.parse::<i64>().ok();
    let tx = conn.unchecked_transaction()?;
    let mut matched = 0;
    {
        let mut up = tx.prepare(
            "UPDATE games SET location = ?2, stadium = ?3, home_score = ?4, away_score = ?5, overtime = ?6 WHERE gameId = ?1",
        )?;
        for rec in rdr.records() {
            let rec = rec?;
            if let Some(gid) = int(&rec[id]) {
                matched += up.execute(params![gid, text(&rec[loc]), text(&rec[stadium]), int(&rec[hs]), int(&rec[aws]), int(&rec[ot])])?;
            }
        }
    }
    tx.commit()?;
    println!("  schedule      {matched} games matched  {:.1}s", start.elapsed().as_secs_f64());
    Ok(())
}

pub fn logo_url(abbr: &str) -> String {
    format!("https://a.espncdn.com/i/teamlogos/nfl/500/{}.png", nfl_core::teams::espn_code(abbr))
}

/// Downscales any image to fit 128 × 128 and re-encodes it as PNG.
pub fn downscale_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes)?;
    let small = img.resize(128, 128, FilterType::Lanczos3);
    let mut out = Cursor::new(Vec::new());
    small.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

fn download(url: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .call()?
        .into_reader()
        .take(5_000_000)
        .read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn logos_online(conn: &Connection) {
    let start = Instant::now();
    let mut ok = 0;
    for t in nfl_core::teams::TEAMS {
        match download(&logo_url(t.abbr)).and_then(|b| downscale_png(&b)) {
            Ok(png) => match conn.execute("UPDATE teams SET logo_png = ?2 WHERE abbr = ?1", params![t.abbr, png]) {
                Ok(_) => ok += 1,
                Err(e) => eprintln!("warning: logo {}: {e}", t.abbr),
            },
            Err(e) => eprintln!("warning: logo {} not downloaded ({e})", t.abbr),
        }
    }
    println!("  logos         {ok}/32  {:.1}s", start.elapsed().as_secs_f64());
}
