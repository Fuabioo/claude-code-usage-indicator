use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Instant;

/// Raw API response from the Anthropic OAuth usage endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct UsageResponse {
    pub seven_day: UsageWindow,
    pub five_hour: UsageWindow,
}

/// A single usage window with utilization and reset time.
#[derive(Debug, Clone, Deserialize)]
pub struct UsageWindow {
    pub utilization: f64,
    pub resets_at: DateTime<Utc>,
}

/// Processed budget state held in the applet model.
#[derive(Debug, Clone)]
pub struct BudgetState {
    pub weekly: WindowState,
    pub hourly: WindowState,
    pub last_updated: Instant,
}

/// Computed state for a usage window.
#[derive(Debug, Clone)]
pub struct WindowState {
    pub utilization: f64,
    pub resets_at: DateTime<Utc>,
    pub pace_color: PaceColor,
}

/// Pace-based color indicator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaceColor {
    Green,
    Yellow,
    Red,
}

/// Budget-related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BudgetError {
    #[error("network error: {0}")]
    Network(String),

    #[error("failed to parse API response: {0}")]
    Parse(String),

    #[error("unauthorized -- token may be expired")]
    Unauthorized,

    #[error("rate limited by API (retry after {0}s)")]
    RateLimited(u64),

    #[error("unexpected HTTP status: {0}")]
    UnexpectedStatus(u16),

    #[error("cannot read credentials file: {0}")]
    CredentialsRead(String),

    #[error("cannot parse credentials file: {0}")]
    CredentialsParse(String),

    #[error("credentials file missing access token field")]
    CredentialsMissingToken,
}

/// Formats a duration as a human-readable string.
///
/// # Examples
///
/// - 3 days, 12 hours -> "3d 12h"
/// - 2 hours, 45 minutes -> "2h 45m"
/// - 45 minutes -> "45m"
/// - 0 seconds -> "1m" (minimum)
pub fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds();
    if total_secs < 0 {
        return String::from("0m");
    }

    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;

    match (days, hours, minutes) {
        (d, h, _) if d > 0 && h > 0 => format!("{}d {}h", d, h),
        (d, _, _) if d > 0 => format!("{}d", d),
        (_, h, m) if h > 0 && m > 0 => format!("{}h {}m", h, m),
        (_, h, _) if h > 0 => format!("{}h", h),
        (_, _, m) => format!("{}m", m.max(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_days_hours() {
        let duration = chrono::Duration::seconds(3 * 86400 + 12 * 3600);
        assert_eq!(format_duration(duration), "3d 12h");
    }

    #[test]
    fn test_format_duration_days_only() {
        let duration = chrono::Duration::seconds(5 * 86400);
        assert_eq!(format_duration(duration), "5d");
    }

    #[test]
    fn test_format_duration_hours_minutes() {
        let duration = chrono::Duration::seconds(2 * 3600 + 45 * 60);
        assert_eq!(format_duration(duration), "2h 45m");
    }

    #[test]
    fn test_format_duration_hours_only() {
        let duration = chrono::Duration::seconds(3 * 3600);
        assert_eq!(format_duration(duration), "3h");
    }

    #[test]
    fn test_format_duration_minutes_only() {
        let duration = chrono::Duration::seconds(45 * 60);
        assert_eq!(format_duration(duration), "45m");
    }

    #[test]
    fn test_format_duration_zero() {
        let duration = chrono::Duration::seconds(0);
        assert_eq!(format_duration(duration), "1m");
    }

    #[test]
    fn test_format_duration_negative() {
        let duration = chrono::Duration::seconds(-100);
        assert_eq!(format_duration(duration), "0m");
    }

    #[test]
    fn test_pace_color_equality() {
        assert_eq!(PaceColor::Green, PaceColor::Green);
        assert_ne!(PaceColor::Green, PaceColor::Yellow);
    }
}
