use super::types::PaceColor;
use chrono::{DateTime, Datelike, Utc};

const CYCLE_LENGTH: i64 = 7;

/// Computes the pace color for the weekly usage window.
///
/// This implements the pace-based budgeting algorithm: given the current day of the billing cycle,
/// compute a "ceiling" (how much budget should be consumed by now), then compare actual
/// utilization to that ceiling.
///
/// # Arguments
///
/// * `utilization` - Current weekly usage as a percentage (0.0 - 100.0+)
/// * `daily_budget` - Expected usage percentage per work day (typically 20.0 for 5-day week)
/// * `work_days` - Number of budget days per cycle (1-7, typically 5)
/// * `resets_at` - When the billing cycle resets (from API)
/// * `now` - Current UTC time
///
/// # Returns
///
/// - `Green` if ratio < 0.75 (well under budget)
/// - `Yellow` if ratio < 1.00 (approaching ceiling)
/// - `Red` if ratio >= 1.00 (over budget for today)
pub fn compute_weekly_pace_color(
    utilization: f64,
    daily_budget: f64,
    work_days: u8,
    resets_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> PaceColor {
    let work_day_index = days_into_cycle(resets_at, now).min(work_days);
    let ceiling = work_day_index as f64 * daily_budget;

    // Defensive: avoid division by zero
    if ceiling <= 0.0 {
        return PaceColor::Red;
    }

    let ratio = utilization / ceiling;

    if ratio < 0.75 {
        PaceColor::Green
    } else if ratio < 1.00 {
        PaceColor::Yellow
    } else {
        PaceColor::Red
    }
}

/// Computes the pace color for the hourly (5-hour) usage window.
///
/// Uses fixed thresholds since the window is too short-term for daily budgeting.
///
/// # Returns
///
/// - `Green` if utilization <= 50
/// - `Yellow` if utilization <= 80
/// - `Red` if utilization > 80
pub fn compute_hourly_color(utilization: f64) -> PaceColor {
    if utilization <= 50.0 {
        PaceColor::Green
    } else if utilization <= 80.0 {
        PaceColor::Yellow
    } else {
        PaceColor::Red
    }
}

/// Derives the day index within the billing cycle from the API reset timestamp.
///
/// `days_into_cycle = CYCLE_LENGTH - floor(days_remaining)`.
/// chrono's `num_days()` truncates toward zero, so this implicitly produces a
/// ceiling-like effect on cycle position (e.g., 30min left → day 7).
///
/// When `resets_at < now` (stale data), the result is clamped to CYCLE_LENGTH.
pub fn days_into_cycle(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> u8 {
    let days_remaining = (resets_at - now).num_days();
    let day = CYCLE_LENGTH - days_remaining;
    (day as u8).clamp(1, CYCLE_LENGTH as u8)
}

/// Returns the abbreviated weekday name (3 chars) of the reset day.
pub fn reset_day_name(resets_at: DateTime<Utc>) -> String {
    format!("{:?}", resets_at.weekday())
        .chars()
        .take(3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_utc(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
            .unwrap()
    }

    // --- days_into_cycle tests ---

    #[test]
    fn test_just_after_reset_is_day_1() {
        // Reset is in ~7 days → day 1
        let now = make_utc(2024, 3, 7, 12, 0); // Thursday noon
        let resets_at = make_utc(2024, 3, 14, 9, 0); // Next Thursday 9am
        assert_eq!(days_into_cycle(resets_at, now), 1);
    }

    #[test]
    fn test_mid_cycle_is_day_4() {
        // 3 days remaining → day 4
        let now = make_utc(2024, 3, 11, 12, 0); // Monday noon
        let resets_at = make_utc(2024, 3, 14, 9, 0); // Thursday 9am (2.875 days)
        assert_eq!(days_into_cycle(resets_at, now), 5); // 7 - 2 = 5
    }

    #[test]
    fn test_partial_day_boundary() {
        // 1 day + 30 min remaining → num_days()=1 → day=7-1=6
        let now = make_utc(2024, 3, 12, 20, 30);
        let resets_at = make_utc(2024, 3, 14, 9, 0); // ~1.52 days
        assert_eq!(days_into_cycle(resets_at, now), 6);
    }

    #[test]
    fn test_thirty_minutes_until_reset_is_day_7() {
        // 30 min remaining → num_days()=0 → day=7
        let now = make_utc(2024, 3, 14, 8, 30);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now), 7);
    }

    #[test]
    fn test_stale_resets_at_clamps_to_7() {
        // resets_at is in the past
        let now = make_utc(2024, 3, 15, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now), 7);
    }

    #[test]
    fn test_exactly_on_reset_is_day_7() {
        let t = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(t, t), 7);
    }

    // --- compute_weekly_pace_color tests ---

    #[test]
    fn test_day1_zero_percent_is_green() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(0.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Green);
    }

    #[test]
    fn test_day1_over_ceiling_is_red() {
        // day_index=1, ceiling=20, ratio=22/20=1.10
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(22.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_end_of_cycle_ninety_five_percent_is_yellow() {
        // day_index=7, clamped to work_days=5, ceiling=100, ratio=0.95
        let now = make_utc(2024, 3, 14, 8, 30);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(95.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Yellow);
    }

    #[test]
    fn test_utilization_over_hundred_is_red() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(120.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_zero_daily_budget_is_red() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(10.0, 0.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_work_days_clamps_correctly() {
        // work_days=3, day_index=5 clamped to 3. ceiling=3*33.33=99.99, ratio≈0.55
        let now = make_utc(2024, 3, 12, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let color = compute_weekly_pace_color(55.0, 33.33, 3, resets_at, now);
        assert_eq!(color, PaceColor::Green);
    }

    // --- hourly color tests ---

    #[test]
    fn test_hourly_forty_nine_is_green() {
        assert_eq!(compute_hourly_color(49.0), PaceColor::Green);
    }

    #[test]
    fn test_hourly_fifty_is_green() {
        assert_eq!(compute_hourly_color(50.0), PaceColor::Green);
    }

    #[test]
    fn test_hourly_fifty_one_is_yellow() {
        assert_eq!(compute_hourly_color(51.0), PaceColor::Yellow);
    }

    #[test]
    fn test_hourly_eighty_is_yellow() {
        assert_eq!(compute_hourly_color(80.0), PaceColor::Yellow);
    }

    #[test]
    fn test_hourly_eighty_one_is_red() {
        assert_eq!(compute_hourly_color(81.0), PaceColor::Red);
    }

    #[test]
    fn test_hourly_over_hundred_is_red() {
        assert_eq!(compute_hourly_color(150.0), PaceColor::Red);
    }

    // --- reset_day_name tests ---

    #[test]
    fn test_reset_day_name_thursday() {
        let resets_at = make_utc(2024, 3, 14, 9, 0); // Thursday
        assert_eq!(reset_day_name(resets_at), "Thu");
    }
}
