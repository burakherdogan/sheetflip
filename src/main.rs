//! Sheetflip: a small web app for double-sided and booklet printing on printers
//! without a duplex unit.

mod cups;
mod error;
mod i18n;
mod job;
mod pdf;
mod routes;
mod ui;

use std::{env, path::PathBuf, process::ExitCode};

const HELP: &str = "\
Sheetflip: double-sided and booklet printing for printers without a duplex unit.

Usage: sheetflip [--help | --version]

Environment:
  LISTEN    address to listen on (default 0.0.0.0:8080)
  DATA_DIR  directory for files of running jobs (default: system temp directory)

Printer test results are stored in $XDG_CONFIG_HOME/sheetflip/printers.json.
";

fn settings_path() -> PathBuf {
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"));
    config.join("sheetflip").join("printers.json")
}

#[tokio::main]
async fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Some("-V" | "--version") => {
            println!("sheetflip {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("unknown argument: {other}\n\n{HELP}");
            return ExitCode::FAILURE;
        }
        None => {}
    }

    let state = routes::AppState {
        jobs: job::JobStore::new(
            env::var_os("DATA_DIR")
                .map_or_else(|| env::temp_dir().join("sheetflip"), PathBuf::from),
        ),
        settings: cups::Settings::new(settings_path()),
    };
    let addr = env::var("LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("sheetflip: cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Sheetflip {} listening on http://{addr}",
        env!("CARGO_PKG_VERSION")
    );
    if let Err(e) = axum::serve(listener, routes::router(state)).await {
        eprintln!("sheetflip: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
