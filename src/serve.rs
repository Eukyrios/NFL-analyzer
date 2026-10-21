//! Minimal local HTTP server; every request reads straight from the SQLite database.
//!
//! Routes:
//!   /                        game list
//!   /game/<gameId>           replay page (fetches the API below)
//!   /api/game/<gameId>       game info + play summaries (JSON)
//!   /api/play/<gameId>/<id>  one play's tracking frames (JSON)
//!   /logos/<TEAM>.png        team logo

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

use serde::Serialize;

use crate::game::{self, GameInfo, Res};
use crate::{db, logos};

const TEMPLATE: &str = include_str!("../timeline/viewer.html");
const HTML: &str = "text/html; charset=utf-8";
const JSON: &str = "application/json";

struct App {
    db: PathBuf,
    teams: HashSet<String>,
}

struct Response {
    status: &'static str,
    ctype: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn ok(ctype: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Response { status: "200 OK", ctype, body: body.into() }
    }

    fn json<T: Serialize>(value: Option<T>) -> Res<Self> {
        Ok(match value {
            Some(v) => Response::ok(JSON, serde_json::to_vec(&v)?),
            None => Response::not_found(),
        })
    }

    fn not_found() -> Self {
        Response { status: "404 Not Found", ctype: "text/plain", body: b"not found".to_vec() }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn logo_img(team: &str) -> String {
    format!(
        "<img src=\"/logos/{t}.png\" alt=\"{t}\" onerror=\"this.onerror=null;this.src='{url}'\">",
        t = html_escape(team),
        url = logos::espn_url(team)
    )
}

fn index_html(games: &[GameInfo]) -> String {
    let rows: String = games
        .iter()
        .map(|g| {
            format!(
                "<li><a href=\"/game/{id}\">{al}{away} {sep} {hl}{home}</a><span>{site}Week {wk} · {date} {time} ET</span></li>\n",
                id = g.id,
                al = logo_img(&g.away),
                away = html_escape(&g.away),
                sep = if g.site.is_some() { "vs" } else { "@" },
                hl = logo_img(&g.home),
                home = html_escape(&g.home),
                site = g.site.as_deref().map(|s| format!("{} · ", html_escape(s))).unwrap_or_default(),
                wk = g.week,
                date = html_escape(&g.date),
                time = html_escape(g.time.get(..5).unwrap_or(&g.time)),
            )
        })
        .collect();

    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>Dropback replays</title><style>body{{margin:0;padding:24px 16px;background:#0e1116;\
         color:#e6edf3;font:15px/1.6 system-ui,sans-serif}}main{{max-width:680px;margin:auto}}\
         ul{{list-style:none;padding:0}}li{{display:flex;justify-content:space-between;align-items:center;\
         gap:12px;padding:6px 0;border-bottom:1px solid #2b323c}}a{{color:#e6edf3;text-decoration:none;\
         display:flex;align-items:center;gap:6px;font-weight:600}}a:hover{{color:#58a6ff}}\
         img{{width:28px;height:28px;object-fit:contain}}span{{color:#8b949e;font-size:13px}}</style>\
         <main><h1>Dropback replays</h1><ul>\n{rows}</ul></main>"
    )
}

fn viewer_html(g: &GameInfo) -> String {
    let title = format!("{} · Week {}", g.matchup(), g.week);
    TEMPLATE.replace("__TITLE__", &html_escape(&title)).replace("__GAME_ID__", &g.id.to_string())
}

impl App {
    fn route(&self, path: &str) -> Res<Response> {
        let conn = db::open(&self.db)?;
        let id = |s: &str| s.parse::<i64>().ok();

        if path == "/" || path == "/index.html" {
            return Ok(Response::ok(HTML, index_html(&game::list_games(&conn)?)));
        }
        if let Some(gid) = path.strip_prefix("/game/").and_then(id) {
            return Ok(match game::game_info(&conn, gid)? {
                Some(g) => Response::ok(HTML, viewer_html(&g)),
                None => Response::not_found(),
            });
        }
        if let Some(gid) = path.strip_prefix("/api/game/").and_then(id) {
            return Response::json(game::game_summary(&conn, gid)?);
        }
        if let Some((gid, pid)) = path.strip_prefix("/api/play/").and_then(|p| p.split_once('/')) {
            if let (Some(gid), Some(pid)) = (id(gid), id(pid)) {
                return Response::json(game::play_tracking(&conn, gid, pid)?);
            }
        }
        if let Some(team) = path.strip_prefix("/logos/").and_then(|p| p.strip_suffix(".png")) {
            if self.teams.contains(team) {
                return Ok(Response::ok("image/png", logos::fetch(&conn, team)?));
            }
        }
        Ok(Response::not_found())
    }
}

fn handle(mut stream: TcpStream, app: &App) -> Res<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
    }
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");

    let res = app.route(path).unwrap_or_else(|e| {
        eprintln!("error serving {path}: {e}");
        Response { status: "500 Internal Server Error", ctype: "text/plain", body: e.to_string().into_bytes() }
    });
    write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        res.status,
        res.ctype,
        res.body.len()
    )?;
    stream.write_all(&res.body)?;
    Ok(())
}

fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    } else if cfg!(windows) {
        Command::new("cmd").args(["/C", "start", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    };
    if cmd.is_err() {
        eprintln!("couldn't open a browser; open {url} yourself");
    }
}

pub fn run(db_path: PathBuf, port: u16, open: bool) -> Res<()> {
    let conn = db::open(&db_path)?;
    db::add_schedule(&conn)?;
    let teams = game::teams(&conn)?;
    let games = game::list_games(&conn)?.len();
    drop(conn);
    let app = Arc::new(App { db: db_path, teams });

    // Use the requested port, or the next free one.
    let listener = (port..port.saturating_add(20))
        .find_map(|p| TcpListener::bind(("127.0.0.1", p)).ok())
        .ok_or(format!("no free port in {port}..{}", port.saturating_add(20)))?;
    let url = format!("http://localhost:{}/", listener.local_addr()?.port());
    println!("Serving {games} games at {url}  (Ctrl+C to stop)");
    if open {
        open_browser(&url);
    }

    for stream in listener.incoming().flatten() {
        let app = Arc::clone(&app);
        thread::spawn(move || {
            if let Err(e) = handle(stream, &app) {
                eprintln!("connection error: {e}");
            }
        });
    }
    Ok(())
}
