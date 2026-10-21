//! Replays of every dropback play in NFL Big Data Bowl games, served from SQLite.
//!
//! `cargo run` imports the CSVs into a SQLite database on first use, then serves
//! a local site; each game's plays are loaded from the database on demand.

mod db;
mod game;
mod logos;
mod serve;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

const DEFAULT_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");
const DEFAULT_DB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/nfl.db");

#[derive(Parser)]
#[command(about = "Replays of every dropback play in NFL Big Data Bowl games")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// CSV data directory
    #[arg(long, global = true, default_value = DEFAULT_DATA)]
    data: PathBuf,
    /// SQLite database file
    #[arg(long, global = true, default_value = DEFAULT_DB)]
    db: PathBuf,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve the replays locally and open a browser (the default)
    Serve {
        #[arg(long, default_value_t = 8000)]
        port: u16,
        /// Don't open a browser
        #[arg(long)]
        no_open: bool,
    },
    /// Rebuild the SQLite database from the CSVs
    Import,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd.unwrap_or(Cmd::Serve { port: 8000, no_open: false }) {
        Cmd::Serve { port, no_open } => {
            if cli.db.exists() {
                Ok(())
            } else {
                println!("No database yet; importing the CSVs (one-time).");
                db::import(&cli.data, &cli.db)
            }
            .and_then(|()| serve::run(cli.db, port, !no_open))
        }
        Cmd::Import => db::import(&cli.data, &cli.db),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
