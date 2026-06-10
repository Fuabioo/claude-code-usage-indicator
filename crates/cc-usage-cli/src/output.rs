//! JSON output DTOs (the stable contract consumed by the macOS Swift app) and the
//! human-readable `--status` formatter. Presentation lives here so `cc-usage-budget`
//! stays free of serde/formatting concerns.

use cc_usage_budget::{
    days_into_cycle, format_duration, reset_day_name, BudgetError, PaceColor, UsageResponse,
};
use chrono::{DateTime, Local, Utc};
use serde::Serialize;

use crate::config::Config;

/// Top-level JSON document. On success `error` is null and all data fields are populated;
/// on failure the data fields are null and `error` describes what went wrong.
#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub fetched_at: DateTime<Utc>,
    pub config: ConfigDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<WindowDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<WindowDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_pace: Option<DailyPaceDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDto>,
}

#[derive(Debug, Serialize)]
pub struct ConfigDto {
    pub daily_budget: f64,
    pub work_days: u8,
    pub poll_interval_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct WindowDto {
    pub utilization: f64,
    pub resets_at: DateTime<Utc>,
    /// Seconds until reset (0 if already past).
    pub resets_in_secs: i64,
    pub pace_color: PaceColorDto,
}

#[derive(Debug, Serialize)]
pub struct DailyPaceDto {
    pub work_day_index: u8,
    pub ceiling: f64,
    /// `ceiling - weekly utilization`. Negative means over today's budget.
    pub remaining: f64,
    pub reset_day_local: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorDto {
    pub kind: String,
    pub message: String,
}

/// Pace color as a lowercase string ("green"/"yellow"/"red") for a language-neutral contract.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaceColorDto {
    Green,
    Yellow,
    Red,
}

impl From<PaceColor> for PaceColorDto {
    fn from(c: PaceColor) -> Self {
        match c {
            PaceColor::Green => PaceColorDto::Green,
            PaceColor::Yellow => PaceColorDto::Yellow,
            PaceColor::Red => PaceColorDto::Red,
        }
    }
}

impl ConfigDto {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            daily_budget: cfg.daily_budget,
            work_days: cfg.work_days,
            poll_interval_secs: cfg.poll_interval_secs,
        }
    }
}

impl ErrorDto {
    /// Map a `BudgetError` to a stable machine-readable `kind` plus its display message.
    pub fn from_budget_error(err: &BudgetError) -> Self {
        let kind = match err {
            BudgetError::Network(_) => "network",
            BudgetError::Parse(_) => "parse",
            BudgetError::Unauthorized => "unauthorized",
            BudgetError::RateLimited(_) => "rate_limited",
            BudgetError::UnexpectedStatus(_) => "unexpected_status",
            BudgetError::CredentialsRead(_) => "credentials_read",
            BudgetError::CredentialsParse(_) => "credentials_parse",
            BudgetError::CredentialsMissingToken => "credentials_missing_token",
        };
        Self {
            kind: kind.to_string(),
            message: err.to_string(),
        }
    }
}

/// Build a successful snapshot from the API response plus config-derived calculations.
pub fn build_snapshot(usage: &UsageResponse, cfg: &Config, now: DateTime<Utc>) -> Snapshot {
    let weekly_color = cc_usage_budget::compute_weekly_pace_color(
        usage.seven_day.utilization,
        cfg.daily_budget,
        cfg.work_days,
        usage.seven_day.resets_at,
        now,
    );
    let session_color = cc_usage_budget::compute_hourly_color(usage.five_hour.utilization);

    let work_day_index = days_into_cycle(usage.seven_day.resets_at, now, cfg.work_days);
    let ceiling = work_day_index as f64 * cfg.daily_budget;

    Snapshot {
        fetched_at: now,
        config: ConfigDto::from_config(cfg),
        weekly: Some(WindowDto {
            utilization: usage.seven_day.utilization,
            resets_at: usage.seven_day.resets_at,
            resets_in_secs: secs_until(usage.seven_day.resets_at, now),
            pace_color: weekly_color.into(),
        }),
        session: Some(WindowDto {
            utilization: usage.five_hour.utilization,
            resets_at: usage.five_hour.resets_at,
            resets_in_secs: secs_until(usage.five_hour.resets_at, now),
            pace_color: session_color.into(),
        }),
        daily_pace: Some(DailyPaceDto {
            work_day_index,
            ceiling,
            remaining: ceiling - usage.seven_day.utilization,
            reset_day_local: reset_day_name(usage.seven_day.resets_at),
        }),
        error: None,
    }
}

/// Build a failure snapshot carrying only the config and the error.
pub fn build_error_snapshot(err: &BudgetError, cfg: &Config, now: DateTime<Utc>) -> Snapshot {
    Snapshot {
        fetched_at: now,
        config: ConfigDto::from_config(cfg),
        weekly: None,
        session: None,
        daily_pace: None,
        error: Some(ErrorDto::from_budget_error(err)),
    }
}

fn secs_until(when: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    when.signed_duration_since(now).num_seconds().max(0)
}

/// Render the human-readable `--status` view (a trimmed port of the applet's diagnostic).
pub fn render_status(usage: &UsageResponse, cfg: &Config, now: DateTime<Utc>) -> String {
    let snap = build_snapshot(usage, cfg, now);
    let weekly = snap.weekly.as_ref().expect("success snapshot has weekly");
    let session = snap.session.as_ref().expect("success snapshot has session");
    let pace = snap.daily_pace.as_ref().expect("success snapshot has daily_pace");

    let mut out = String::new();
    out.push_str("=== Claude Code Usage ===\n\n");
    out.push_str(&format!("now (local)    = {}\n", now.with_timezone(&Local)));
    out.push_str(&format!("resets (local) = {}\n\n", pace.reset_day_local));

    out.push_str("[Config]\n");
    out.push_str(&format!(
        "  daily_budget = {}% per work day\n",
        cfg.daily_budget
    ));
    out.push_str(&format!("  work_days    = {}\n\n", cfg.work_days));

    out.push_str("[Weekly]\n");
    out.push_str(&format!("  utilization  = {:.1}%\n", weekly.utilization));
    out.push_str(&format!(
        "  pace         = {:?}\n",
        weekly.pace_color
    ));
    out.push_str(&format!(
        "  ceiling      = {} work days x {}% = {:.1}%\n",
        pace.work_day_index, cfg.daily_budget, pace.ceiling
    ));
    out.push_str(&format!(
        "  remaining    = {:.1}%\n",
        pace.remaining
    ));
    out.push_str(&format!(
        "  resets_in    = {}\n\n",
        format_duration(weekly.resets_at.signed_duration_since(now))
    ));

    out.push_str("[Session (5h)]\n");
    out.push_str(&format!("  utilization  = {:.1}%\n", session.utilization));
    out.push_str(&format!(
        "  pace         = {:?}\n",
        session.pace_color
    ));
    out.push_str(&format!(
        "  resets_in    = {}\n",
        format_duration(session.resets_at.signed_duration_since(now))
    ));

    out
}
