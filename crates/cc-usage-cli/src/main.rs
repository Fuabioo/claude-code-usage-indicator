//! `cc-usage` — cross-platform CLI for Claude Code usage budget.
//!
//! Reads the Claude credentials file, fetches usage from the Anthropic OAuth usage API,
//! and prints either a machine-readable JSON snapshot (default) or a human-readable
//! status report (`--status`). The JSON form is the stable contract consumed by the macOS
//! menu bar app; it is also handy in a terminal and on Linux.

mod config;
mod creds;
mod output;

use std::time::Duration;

use chrono::Utc;
use clap::Parser;

use cc_usage_budget::fetch_usage;
use config::Config;
use creds::CredsOptions;

#[derive(Parser, Debug)]
#[command(
    name = "cc-usage",
    about = "Monitor Claude Code usage budget (JSON or human-readable status)",
    version
)]
struct Cli {
    /// Print a human-readable status report instead of JSON.
    #[arg(long, conflicts_with = "json")]
    status: bool,

    /// Force JSON output (the default).
    #[arg(long)]
    json: bool,

    /// Path to the Claude credentials file (supports `~`).
    #[arg(long, value_name = "PATH")]
    creds_path: Option<String>,

    /// Expected usage percentage per work day (default 20.0).
    #[arg(long, value_name = "PCT")]
    daily_budget: Option<f64>,

    /// Number of budget work days per cycle, 1-7 (default 5).
    #[arg(long, value_name = "N")]
    work_days: Option<u8>,

    /// HTTP request timeout in seconds.
    #[arg(long, default_value_t = 30, value_name = "SECS")]
    timeout: u64,

    /// macOS only: Keychain generic-password service to read credentials from when no
    /// credentials file is present.
    #[arg(long, default_value = "Claude Code-credentials", value_name = "NAME")]
    keychain_service: String,

    /// Disable the macOS Keychain fallback (only read the credentials file).
    #[arg(long)]
    no_keychain: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let cfg = Config::resolve(cli.creds_path.clone(), cli.daily_budget, cli.work_days);

    std::process::exit(run(&cli, &cfg).await);
}

/// Run the request and emit output. Returns the process exit code (0 ok, 1 on failure).
async fn run(cli: &Cli, cfg: &Config) -> i32 {
    let now = Utc::now();

    match fetch(cli, cfg).await {
        Ok(usage) => {
            if cli.status {
                print!("{}", output::render_status(&usage, cfg, now));
            } else {
                let snap = output::build_snapshot(&usage, cfg, now);
                print_json(&snap);
            }
            0
        }
        Err(err) => {
            if cli.status {
                eprintln!("error: {err}");
            } else {
                // Still emit a valid JSON document so GUI callers can parse the error.
                let snap = output::build_error_snapshot(&err, cfg, now);
                print_json(&snap);
            }
            1
        }
    }
}

/// Resolve the token (file, or macOS Keychain fallback) and fetch usage.
async fn fetch(
    cli: &Cli,
    cfg: &Config,
) -> Result<cc_usage_budget::UsageResponse, cc_usage_budget::BudgetError> {
    let opts = CredsOptions {
        creds_path_explicit: cli.creds_path.is_some(),
        keychain_service: cli.keychain_service.clone(),
        no_keychain: cli.no_keychain,
    };
    let token = creds::resolve_token(cfg, &opts)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(cli.timeout))
        .build()
        .map_err(|e| cc_usage_budget::BudgetError::Network(e.to_string()))?;
    fetch_usage(&token, &client).await
}

fn print_json(snap: &output::Snapshot) {
    match serde_json::to_string_pretty(snap) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialize JSON: {e}"),
    }
}
