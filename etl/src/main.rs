use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: etl [--data DIR] [--out FILE] [--offline] [--schedule FILE] [--no-copy] [--fixtures]

  --data DIR       dataset folder (default: <repo>/data)
  --out FILE       output database (default: <repo>/data/nfl.sqlite)
  --offline        skip the nflverse schedule and ESPN logos
  --schedule FILE  read the schedule from a local CSV instead of nflverse
  --no-copy        don't copy the result to app/assets/nfl.sqlite
  --fixtures       build from etl/fixtures into target/fixtures.sqlite (offline)";

fn main() -> ExitCode {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut o = etl::Options {
        data: repo.join("data"),
        out: repo.join("data/nfl.sqlite"),
        offline: false,
        schedule_file: None,
        copy_to: Some(repo.join("app/assets/nfl.sqlite")),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => o.data = args.next().map(PathBuf::from).unwrap_or(o.data),
            "--out" => o.out = args.next().map(PathBuf::from).unwrap_or(o.out),
            "--offline" => o.offline = true,
            "--schedule" => o.schedule_file = args.next().map(PathBuf::from),
            "--no-copy" => o.copy_to = None,
            "--fixtures" => {
                let fx = repo.join("etl/fixtures");
                o.data = fx.clone();
                o.out = repo.join("target/fixtures.sqlite");
                o.schedule_file = Some(fx.join("schedule.csv"));
                o.copy_to = None;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other}\n{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }
    match etl::run(&o) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
