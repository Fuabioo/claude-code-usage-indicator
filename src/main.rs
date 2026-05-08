mod app;
mod budget;
mod config;
mod i18n;
mod views;

use app::AppModel;

fn main() -> cosmic::iced::Result {
    // Check for diagnostic subcommands before launching the applet
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--status" => {
                run_status().unwrap_or_else(|e| eprintln!("Error: {e}"));
                return Ok(());
            }
            other => {
                eprintln!("Unknown flag: {other}");
                std::process::exit(1);
            }
        }
    }

    // Initialize i18n system
    if let Err(e) = i18n::init() {
        eprintln!("Warning: Failed to initialize i18n: {}", e);
    }

    // Launch the applet
    cosmic::applet::run::<AppModel>(())
}

/// Diagnostic mode: fetch usage and print all intermediate calculations.
/// Useful for auditing pace labels and ceiling math.
fn run_status() -> Result<(), Box<dyn std::error::Error>> {
    use budget::{compute_hourly_color, compute_weekly_pace_color, days_into_cycle, fetch_usage, format_duration, read_token, reset_day_name};
    use chrono::Utc;
    use cosmic::cosmic_config::CosmicConfigEntry;

    const APP_ID: &str = "dev.fuabioo.CosmicAppletCcUsage";

    // Read the same persisted config the applet uses
    let config = match cosmic::cosmic_config::Config::new(APP_ID, config::CONFIG_VERSION) {
        Ok(helper) => match config::Config::get_entry(&helper) {
            Ok(c) => c.validated(),
            Err((_errs, c)) => c.validated(),
        },
        Err(_) => config::Config::default(),
    };

    let creds_path = config::expand_tilde(&config.creds_path);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()?;

    let token = read_token(&creds_path)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let usage = rt.block_on(fetch_usage(&token, &client))?;
    let now = Utc::now();

    let weekly_color = compute_weekly_pace_color(
        usage.seven_day.utilization,
        config.daily_budget,
        config.work_days,
        usage.seven_day.resets_at,
        now,
    );
    let hourly_color = compute_hourly_color(usage.five_hour.utilization);
    let work_day_index = days_into_cycle(usage.seven_day.resets_at, now, config.work_days);
    let ceiling = work_day_index as f64 * config.daily_budget;

    println!("=== Budget Status ===");
    println!();
    println!("now           = {}", now);
    println!("resets_at     = {} ({})", usage.seven_day.resets_at, reset_day_name(usage.seven_day.resets_at));
    println!();
    println!("[Config]");
    println!("  daily_budget     = {}% per work day", config.daily_budget);
    println!("  work_days        = {}", config.work_days);
    println!("  poll_interval    = {}s", config.poll_interval_secs);
    println!();
    println!("[Weekly]");
    println!("  utilization      = {:.1}%", usage.seven_day.utilization);
    println!("  work_day_index   = {} / {}   (counted weekdays)", work_day_index, config.work_days);
    println!("  ceiling          = {} × {}% = {:.1}%", work_day_index, config.daily_budget, ceiling);
    if ceiling > 0.0 {
        println!("  utilization/ceiling = {:.1} / {:.1} = {:.2}", usage.seven_day.utilization, ceiling, usage.seven_day.utilization / ceiling);
    }
    println!("  pace_color       = {:?}", weekly_color);
    println!("  resets_in        = {}", format_duration(usage.seven_day.resets_at.signed_duration_since(now)));
    println!();
    println!("[Session (5h)]");
    println!("  utilization      = {:.1}%", usage.five_hour.utilization);
    println!("  pace_color       = {:?}", hourly_color);
    println!("  resets_in        = {}", format_duration(usage.five_hour.resets_at.signed_duration_since(now)));

    Ok(())
}
