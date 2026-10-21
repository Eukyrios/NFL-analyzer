use std::collections::BTreeSet;

use etl::{build, fixtures_dir, tracking, Options};
use nfl_core::dal::Db;
use rusqlite::Connection;

fn built(schedule: bool) -> Connection {
    let fx = fixtures_dir();
    let o = Options {
        data: fx.clone(),
        out: fx.join("unused.sqlite"),
        offline: true,
        schedule_file: schedule.then(|| fx.join("schedule.csv")),
        copy_to: None,
    };
    let mut c = Connection::open_in_memory().unwrap();
    build(&mut c, &o).unwrap();
    c
}

fn one<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn flat_tables() {
    let c = built(true);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM games"), 2);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM plays"), 3);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM players"), 2);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM pff_scouting"), 3);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM teams"), 32);
    assert_eq!(one::<String>(&c, "SELECT height FROM players WHERE nflId = 25511"), "6-4");
    assert_eq!(one::<Option<String>>(&c, "SELECT birthDate FROM players WHERE nflId = 35459"), None);
    assert_eq!(one::<Option<i64>>(&c, "SELECT penaltyYards FROM plays WHERE playId = 97"), None);
    assert_eq!(one::<String>(&c, "SELECT gameClock FROM plays WHERE playId = 97"), "13:33");
}

#[test]
fn schedule_enrichment() {
    let c = built(true);
    let db = Db::from_conn(c).unwrap();
    let london = db.game(2021101000).unwrap().unwrap();
    assert!(london.is_neutral());
    assert_eq!(london.stadium.as_deref(), Some("Tottenham Stadium"));
    assert_eq!(london.final_score(), Some((27, 20)));
    assert!(!db.game(2021090900).unwrap().unwrap().is_neutral());
}

#[test]
fn offline_leaves_nulls() {
    let c = built(false);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM games WHERE location IS NOT NULL OR home_score IS NOT NULL"), 0);
    assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM teams WHERE logo_png IS NOT NULL"), 0);
}

#[test]
fn tracking_tables() {
    let db = Db::from_conn(built(true)).unwrap();
    let t = db.tracking(2021090900, 97).unwrap().unwrap();
    assert_eq!(t.frames.len(), 4);
    assert_eq!(t.entities.len(), 3);
    let ball = t.entities.last().unwrap();
    assert!(ball.is_ball() && ball.team == "football");
    // Manual pass_forward wins over the auto tag on another frame.
    assert_eq!(t.events, vec![(2, "ball_snap".to_string()), (4, "pass_forward".to_string())]);
    assert_eq!(t.summary.snap_frame, 2);
    assert_eq!(t.summary.throw_frame, Some(4));
    assert!((t.summary.time_to_throw.unwrap() - 0.2).abs() < 1e-4);

    // Play 137 ran left: normalized and auto snap renamed.
    let t = db.tracking(2021090900, 137).unwrap().unwrap();
    let qb = t.frames[0][0].unwrap();
    assert!((qb.x - 40.0).abs() < 0.01 && (qb.y - 23.3).abs() < 0.01);
    assert!((qb.o.unwrap() - 190.0).abs() < 0.1);
    assert_eq!(t.summary.snap_frame, 1);
    assert_eq!(t.summary.sack_frame, Some(3));
    assert_eq!(t.summary.throw_frame, None);

    let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM player_play_stats", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3);
}

#[test]
fn dedup_rules() {
    let raw: BTreeSet<(i64, String)> =
        [(5, "autoevent_ballsnap"), (6, "ball_snap"), (9, "autoevent_passforward"), (9, "play_action")]
            .into_iter()
            .map(|(f, e)| (f, e.to_string()))
            .collect();
    assert_eq!(
        tracking::dedup_events(&raw),
        vec![(6, "ball_snap".into()), (9, "pass_forward".into()), (9, "play_action".into())]
    );
}

#[test]
fn downscale_logo() {
    let img = image::RgbaImage::from_pixel(500, 500, image::Rgba([1, 2, 3, 255]));
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let small = etl::enrich::downscale_png(png.get_ref()).unwrap();
    let back = image::load_from_memory(&small).unwrap();
    assert_eq!((back.width(), back.height()), (128, 128));
    assert_eq!(etl::enrich::logo_url("LA"), "https://a.espncdn.com/i/teamlogos/nfl/500/lar.png");
}
