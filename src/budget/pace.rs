use super::types::PaceColor;
use chrono::{DateTime, Datelike, Utc, Weekday};

const CYCLE_LENGTH: i64 = 7;

/// Computes the pace color for the weekly usage window.
///
/// This implements the pace-based budgeting algorithm: given the current work day
/// within the billing cycle, compute a "ceiling" (how much budget should be consumed
/// by now), then compare actual utilization to that ceiling.
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
    let work_day_index = days_into_cycle(resets_at, now, work_days);
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

/// Counts work days that have fully elapsed in the billing cycle.
///
/// For `work_days <= 5`, only weekdays (Mon–Fri) are counted, matching a
/// standard work week. For `work_days > 5`, all calendar days count.
///
/// The cycle starts at `resets_at - 7 days`. Each full 24-hour period from
/// cycle start is examined: if it falls on a weekday (and work_days <= 5),
/// it increments the count.
///
/// Result is clamped to `[1, work_days]`.
pub fn days_into_cycle(resets_at: DateTime<Utc>, now: DateTime<Utc>, work_days: u8) -> u8 {
    let cycle_start = resets_at - chrono::Duration::days(CYCLE_LENGTH);

    if now <= cycle_start {
        return 1;
    }

    let full_days = (now - cycle_start).num_days().min(CYCLE_LENGTH) as u64;

    if work_days > 5 {
        // For 6-7 day schedules, all calendar days count
        return (full_days as u8).clamp(1, work_days);
    }

    // On the last day of the cycle (6+ of 7 full days elapsed), all work days
    // are available — the final partial day would otherwise be missed since
    // num_days() truncates.
    if full_days >= (CYCLE_LENGTH - 1) as u64 {
        return work_days;
    }

    // Count weekdays (Mon-Fri) only
    let mut weekday_count = 0u8;
    for i in 0..full_days {
        let day = cycle_start + chrono::Duration::days(i as i64);
        match day.weekday() {
            Weekday::Sat | Weekday::Sun => {}
            _ => weekday_count += 1,
        }
    }

    weekday_count.clamp(1, work_days)
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
    // Cycle: resets Thu Mar 14 09:00 → started Thu Mar 7 09:00
    // Calendar:  Thu7  Fri8  Sat9  Sun10  Mon11  Tue12  Wed13
    // Weekdays:  Thu   Fri   -     -      Mon    Tue    Wed  (5 weekdays)

    #[test]
    fn test_just_after_reset_is_day_1() {
        // Thu Mar 7 12:00, cycle just started. 0 full days elapsed → clamp to 1.
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 1);
    }

    #[test]
    fn test_one_weekday_elapsed() {
        // Fri Mar 8 12:00, 1 full day since cycle start (Thu = weekday) → 1
        let now = make_utc(2024, 3, 8, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 1);
    }

    #[test]
    fn test_weekend_days_dont_count() {
        // Sun Mar 10 12:00, 3 full days (Thu Fri Sat). Weekdays = Thu + Fri = 2
        let now = make_utc(2024, 3, 10, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
    }

    #[test]
    fn test_monday_after_weekend() {
        // Mon Mar 11 12:00, 4 full days (Thu Fri Sat Sun). Weekdays = Thu + Fri = 2
        let now = make_utc(2024, 3, 11, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
    }

    #[test]
    fn test_tuesday_three_weekdays() {
        // Tue Mar 12 12:00, 5 full days (Thu Fri Sat Sun Mon). Weekdays = Thu + Fri + Mon = 3
        let now = make_utc(2024, 3, 12, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 3);
    }

    #[test]
    fn test_last_day_of_cycle_returns_all_work_days() {
        // Wed Mar 13 12:00, ~20h before reset. On the last day of the cycle,
        // the full budget is available (no more work days to wait for).
        let now = make_utc(2024, 3, 13, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    #[test]
    fn test_thirty_minutes_until_reset() {
        // Thu Mar 14 08:30, 7 full days. All 5 weekdays counted.
        let now = make_utc(2024, 3, 14, 8, 30);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    #[test]
    fn test_stale_resets_at_clamps_to_work_days() {
        // Past the reset, full 7 days elapsed → 5 weekdays, clamped to work_days
        let now = make_utc(2024, 3, 15, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    #[test]
    fn test_exactly_on_reset() {
        let t = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(t, t, 5), 5);
    }

    #[test]
    fn test_work_days_3_clamps() {
        // Tue Mar 12 12:00, 3 weekdays elapsed (Thu Fri Mon), but work_days=3 → clamp to 3
        let now = make_utc(2024, 3, 12, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 3), 3);
    }

    #[test]
    fn test_work_days_7_counts_all_calendar_days() {
        // Mon Mar 11 12:00, 4 full calendar days elapsed
        let now = make_utc(2024, 3, 11, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 7), 4);
    }

    #[test]
    fn test_work_days_6_counts_all_calendar_days() {
        // Wed Mar 13 12:00, 6 full calendar days → clamped to 6
        let now = make_utc(2024, 3, 13, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 6), 6);
    }

    #[test]
    fn test_before_cycle_start_returns_1() {
        let now = make_utc(2024, 3, 6, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 1);
    }

    // --- User's real scenario ---
    // Cycle resets Fri Mar 13 16:00 UTC (10am Costa Rica)
    // Cycle started: Fri Mar 6 16:00 UTC
    // Now: Tue Mar 10 ~18:00 UTC
    // Full days: 4 (Fri→Sat, Sat→Sun, Sun→Mon, Mon→Tue)
    // Weekdays in those 4 periods: Fri + Mon = 2

    #[test]
    fn test_user_scenario_friday_reset_tuesday_now() {
        let resets_at = make_utc(2026, 3, 13, 16, 0); // Fri Mar 13 16:00 UTC
        let now = make_utc(2026, 3, 10, 18, 0); // Tue Mar 10 18:00 UTC
        // Cycle started Fri Mar 6 16:00. Full days elapsed = 4.
        // Day periods: Fri(wd), Sat(we), Sun(we), Mon(wd) → 2 weekdays
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
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
        // Near reset: all 5 weekdays elapsed, ceiling=100, ratio=0.95
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
        // Tue Mar 12, 3 weekdays elapsed, work_days=3, ceiling=3*33.33=99.99, ratio≈0.55
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
