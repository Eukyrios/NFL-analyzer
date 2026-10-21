//! Team logos, downloaded on first use and cached in the database's `logos` table.

use std::io::Read;

use rusqlite::{params, Connection, OptionalExtension};

use crate::game::Res;

/// ESPN's logo CDN; the viewer falls back to it when the server can't provide a logo.
pub fn espn_url(team: &str) -> String {
    let code = match team {
        "LA" => "lar".to_string(),
        "WAS" => "wsh".to_string(),
        t => t.to_lowercase(),
    };
    format!("https://a.espncdn.com/i/teamlogos/nfl/500/{code}.png")
}

fn download(url: &str) -> Res<Vec<u8>> {
    let mut buf = Vec::new();
    ureq::get(url).call()?.into_reader().take(5_000_000).read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn fetch(conn: &Connection, team: &str) -> Res<Vec<u8>> {
    let cached = conn.query_row("SELECT png FROM logos WHERE team = ?1", [team], |r| r.get(0)).optional()?;
    if let Some(png) = cached {
        return Ok(png);
    }
    let png = download(&espn_url(team))?;
    conn.execute("INSERT OR REPLACE INTO logos (team, png) VALUES (?1, ?2)", params![team, png])?;
    Ok(png)
}
