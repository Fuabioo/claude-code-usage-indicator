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

/// Counts work days elapsed up to the current (possibly incomplete) period.
///
/// For `work_days <= 5`, only weekdays (Mon–Fri) are counted, matching a
/// standard work week. For `work_days > 5`, all calendar days count.
///
/// The cycle starts at `resets_at - 7 days`. Each 24-hour period is a
/// potential work day. The current incomplete period is included so that
/// "today's" budget contributes to the ceiling.
///
/// Result is clamped to `[1, work_days]`.
pub fn days_into_cycle(resets_at: DateTime<Utc>, now: DateTime<Utc>, work_days: u8) -> u8 {
    let cycle_start = resets_at - chrono::Duration::days(CYCLE_LENGTH);

    if now <= cycle_start {
        return 1;
    }

    // num_days() counts complete 24h periods (floor). Add 1 to include the
    // current partial period so today's budget is part of the ceiling.
    let completed = (now - cycle_start).num_days().min(CYCLE_LENGTH) as u64;
    let total_periods = (completed + 1).min(CYCLE_LENGTH as u64);

    if work_days > 5 {
        return (total_periods as u8).clamp(1, work_days);
    }

    let mut weekday_count = 0u8;
    for i in 0..total_periods {
        let period_start = cycle_start + chrono::Duration::days(i as i64);
        match period_start.weekday() {
            Weekday::Sat | Weekday::Sun => {}
            _ => weekday_count += 1,
        }
    }

    weekday_count.clamp(1, work_days)
}

/// Returns the reset day as "Wed Apr 1" style string to avoid ambiguity
/// about *which* occurrence of the weekday is meant.
pub fn reset_day_name(resets_at: DateTime<Utc>) -> String {
    resets_at.format("%a %b %-d").to_string()
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
    // Note: the current partial day is included (today counts toward the ceiling).

    #[test]
    fn test_just_after_reset_is_day_1() {
        // Thu Mar 7 12:00, cycle just started. Today (Thu) counted → 1.
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 1);
    }

    #[test]
    fn test_one_weekday_elapsed() {
        // Fri Mar 8 12:00. Days: Thu(wd) + today Fri(wd) = 2
        let now = make_utc(2024, 3, 8, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
    }

    #[test]
    fn test_weekend_days_dont_count() {
        // Sun Mar 10 12:00. Days: Thu(wd) + Fri(wd) + Sat(we) + today Sun(we) = 2
        let now = make_utc(2024, 3, 10, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
    }

    #[test]
    fn test_monday_after_weekend() {
        // Mon Mar 11 12:00. Days: Thu(wd) + Fri(wd) + Sat(we) + Sun(we) + today Mon(wd) = 3
        let now = make_utc(2024, 3, 11, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 3);
    }

    #[test]
    fn test_tuesday_three_weekdays() {
        // Tue Mar 12 12:00. Days: Thu(wd) + Fri(wd) + Sat(we) + Sun(we) + Mon(wd) + today Tue(wd) = 4
        let now = make_utc(2024, 3, 12, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 4);
    }

    #[test]
    fn test_last_day_of_cycle_returns_all_work_days() {
        // Wed Mar 13 12:00. Days through today: 7 calendar days = 5 weekdays.
        let now = make_utc(2024, 3, 13, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    #[test]
    fn test_thirty_minutes_until_reset() {
        // Thu Mar 14 08:30. Days through today: 7 calendar days = 5 weekdays.
        let now = make_utc(2024, 3, 14, 8, 30);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    #[test]
    fn test_stale_resets_at_clamps_to_work_days() {
        // Past the reset. Days through today: capped at 7 calendar days → 5 weekdays, clamped.
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
        // Tue Mar 12: 4 weekdays through today (Thu+Fri+Mon+Tue), clamped to work_days=3.
        let now = make_utc(2024, 3, 12, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 3), 3);
    }

    #[test]
    fn test_work_days_7_counts_all_calendar_days() {
        // Mon Mar 11: 5 calendar days through today (Thu+Fri+Sat+Sun+Mon).
        let now = make_utc(2024, 3, 11, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        assert_eq!(days_into_cycle(resets_at, now, 7), 5);
    }

    #[test]
    fn test_work_days_6_counts_all_calendar_days() {
        // Wed Mar 13: 7 calendar days through today, clamped to work_days=6.
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
    // Days through today: Fri(wd) + Sat(we) + Sun(we) + Mon(wd) + Tue(wd) = 3

    #[test]
    fn test_user_scenario_friday_reset_tuesday_now() {
        let resets_at = make_utc(2026, 3, 13, 16, 0); // Fri Mar 13 16:00 UTC
        let now = make_utc(2026, 3, 10, 18, 0); // Tue Mar 10 18:00 UTC
        // Cycle started Fri Mar 6 16:00. Days through today = 5.
        // Day periods: Fri(wd), Sat(we), Sun(we), Mon(wd), Tue(wd) → 3 weekdays
        assert_eq!(days_into_cycle(resets_at, now, 5), 3);
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
        assert_eq!(reset_day_name(resets_at), "Thu Mar 14");
    }

    // --- Timezone and edge case tests ---

    /// Verify that weekday() on a UTC DateTime is timezone-invariant.
    /// The same instant is Tuesday in every timezone, so the calculation
    /// is correct regardless of the user's locale.
    #[test]
    fn test_weekday_is_timezone_invariant() {
        // resets_at is in UTC. weekday() on a UTC DateTime returns the
        // weekday at that UTC instant, which is globally consistent.
        // e.g. 2024-03-12T12:00:00Z is Tuesday everywhere.
        let t = make_utc(2024, 3, 12, 12, 0);
        assert_eq!(t.weekday(), Weekday::Tue);
    }

    /// Reset at 02:00 UTC may show a different calendar date than local time.
    /// This is cosmetic only — the calculation is correct.
    #[test]
    fn test_resets_at_near_midnight_utc() {
        // Reset at 2024-03-14T02:00:00Z. Now: Mar 13 20:00 UTC.
        // Days through today: Mar7+8+9+10+11+12+13 = 7 calendar days.
        // Weekdays: Mar7(wd)+Mar8(wd)+Mar11(wd)+Mar12(wd)+Mar13(wd) = 5.
        let resets_at = make_utc(2024, 3, 14, 2, 0);
        let now = make_utc(2024, 3, 13, 20, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    /// When the cycle_start is late in the UTC day, calendar-date counting
    /// correctly includes the current date regardless of the hour.
    #[test]
    fn test_cycle_start_at_late_hour() {
        // resets_at = Thu Mar 14 23:00, cycle_start = Thu Mar 7 23:00
        // now = Sat Mar 9 22:00. Calendar dates: Mar7(Thu,wd), Mar8(Fri,wd), Mar9(Sat,we) = 2 weekdays.
        let resets_at = make_utc(2024, 3, 14, 23, 0);
        let now = make_utc(2024, 3, 9, 22, 0);
        assert_eq!(days_into_cycle(resets_at, now, 5), 2);
    }

    /// Verify that when well past the reset (stale data), the calculation
    /// still clamps correctly rather than underflowing.
    #[test]
    fn test_well_past_reset_does_not_underflow() {
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let now = make_utc(2024, 3, 20, 9, 0); // 6 days after reset
        // Days through today capped at 7 calendar → 5 weekdays.
        assert_eq!(days_into_cycle(resets_at, now, 5), 5);
    }

    /// When the cycle just started (same day, few hours later), we're
    /// still on day 1 (clamped minimum).
    #[test]
    fn test_same_day_as_cycle_start() {
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        let now = make_utc(2024, 3, 7, 14, 0); // cycle_start = Mar 7 09:00, now = Mar 7 14:00
        assert_eq!(days_into_cycle(resets_at, now, 5), 1);
    }

    /// Verify that pace color uses the ceiling from days_into_cycle
    /// correctly: on day 1 (ceiling=20%), 15% usage is green (75% of ceiling).
    /// On day 1 (cycle just started, ceiling=20%): 15% usage is 75% of ceiling → Yellow.
    #[test]
    fn test_pace_color_day1_at_seventy_five_percent_ceiling() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        // day_index=1, ceiling=20, utilization=15, ratio=0.75 → 0.75 < 0.75 is false → Yellow
        let color = compute_weekly_pace_color(15.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Yellow);
    }

    /// Verify exact yellow/red boundary at ratio = 1.00
    #[test]
    fn test_pace_color_exact_boundary_green_yellow() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        // day_index=1, ceiling=20, utilization=14.99, ratio=0.7495 → Green
        let color = compute_weekly_pace_color(14.99, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Green);
    }

    /// Verify exact yellow/red boundary at ratio = 1.00
    #[test]
    fn test_pace_color_exact_boundary_yellow_red() {
        let now = make_utc(2024, 3, 7, 12, 0);
        let resets_at = make_utc(2024, 3, 14, 9, 0);
        // day_index=1, ceiling=20, utilization=19.99, ratio=0.9995 → Yellow
        let color = compute_weekly_pace_color(19.99, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Yellow);

        // utilization=20.0, ratio=1.00 → Red (1.00 < 1.00 is false)
        let color = compute_weekly_pace_color(20.0, 20.0, 5, resets_at, now);
        assert_eq!(color, PaceColor::Red);
    }
}
