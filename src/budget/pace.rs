use super::types::PaceColor;
use chrono::{DateTime, Datelike, Local, Weekday};

/// Computes the pace color for the weekly usage window.
///
/// This implements the pace-based budgeting algorithm: given the current day of the work week,
/// compute a "ceiling" (how much budget should be consumed by now), then compare actual
/// utilization to that ceiling.
///
/// # Arguments
///
/// * `utilization` - Current weekly usage as a percentage (0.0 - 100.0+)
/// * `daily_budget` - Expected usage percentage per work day (typically 20.0 for 5-day week)
/// * `work_days` - Number of work days per week (1-7, typically 5)
/// * `now` - Current date/time in local timezone
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
    now: DateTime<Local>,
) -> PaceColor {
    let work_day_index = weekday_to_work_index(now.weekday(), work_days);
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

/// Maps a weekday to a work day index (1-based).
///
/// - Monday -> 1, Tuesday -> 2, ..., Friday -> 5
/// - Saturday/Sunday -> work_days (uses last work day's ceiling)
/// - Values are clamped to work_days to handle short work weeks.
pub fn weekday_to_work_index(weekday: Weekday, work_days: u8) -> u8 {
    match weekday {
        Weekday::Mon => 1_u8.min(work_days),
        Weekday::Tue => 2_u8.min(work_days),
        Weekday::Wed => 3_u8.min(work_days),
        Weekday::Thu => 4_u8.min(work_days),
        Weekday::Fri => 5_u8.min(work_days),
        Weekday::Sat | Weekday::Sun => work_days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_datetime(year: i32, month: u32, day: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, 12, 0, 0)
            .unwrap()
    }

    #[test]
    fn test_monday_zero_percent_is_green() {
        let monday = make_datetime(2024, 2, 5);
        let color = compute_weekly_pace_color(0.0, 20.0, 5, monday);
        assert_eq!(color, PaceColor::Green);
    }

    #[test]
    fn test_monday_over_ceiling_is_red() {
        // ceiling = 1 * 20 = 20, ratio = 22/20 = 1.10
        let monday = make_datetime(2024, 2, 5);
        let color = compute_weekly_pace_color(22.0, 20.0, 5, monday);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_wednesday_at_boundary_is_yellow() {
        // ceiling = 3 * 20 = 60, ratio = 45/60 = 0.75
        let wednesday = make_datetime(2024, 2, 7);
        let color = compute_weekly_pace_color(45.0, 20.0, 5, wednesday);
        assert_eq!(color, PaceColor::Yellow);
    }

    #[test]
    fn test_friday_ninety_five_percent_is_yellow() {
        // ceiling = 5 * 20 = 100, ratio = 95/100 = 0.95
        let friday = make_datetime(2024, 2, 9);
        let color = compute_weekly_pace_color(95.0, 20.0, 5, friday);
        assert_eq!(color, PaceColor::Yellow);
    }

    #[test]
    fn test_saturday_uses_friday_ceiling() {
        // ceiling = 5 * 20 = 100, ratio = 80/100 = 0.80
        let saturday = make_datetime(2024, 2, 10);
        let color = compute_weekly_pace_color(80.0, 20.0, 5, saturday);
        assert_eq!(color, PaceColor::Yellow);
    }

    #[test]
    fn test_sunday_uses_friday_ceiling() {
        // ceiling = 5 * 20 = 100, ratio = 75/100 = 0.75
        let sunday = make_datetime(2024, 2, 11);
        let color = compute_weekly_pace_color(75.0, 20.0, 5, sunday);
        assert_eq!(color, PaceColor::Yellow);
    }

    #[test]
    fn test_utilization_over_hundred_is_red() {
        let monday = make_datetime(2024, 2, 5);
        let color = compute_weekly_pace_color(120.0, 20.0, 5, monday);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_zero_daily_budget_is_red() {
        let monday = make_datetime(2024, 2, 5);
        let color = compute_weekly_pace_color(10.0, 0.0, 5, monday);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_negative_daily_budget_is_red() {
        let monday = make_datetime(2024, 2, 5);
        let color = compute_weekly_pace_color(10.0, -5.0, 5, monday);
        assert_eq!(color, PaceColor::Red);
    }

    #[test]
    fn test_work_days_clamps_correctly() {
        // work_days=3, Friday -> clamped to 3. ceiling = 3 * 33.33 = 99.99, ratio = 55/99.99 ≈ 0.55
        let friday = make_datetime(2024, 2, 9);
        let color = compute_weekly_pace_color(55.0, 33.33, 3, friday);
        assert_eq!(color, PaceColor::Green);
    }

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

    #[test]
    fn test_weekday_to_work_index_standard_week() {
        assert_eq!(weekday_to_work_index(Weekday::Mon, 5), 1);
        assert_eq!(weekday_to_work_index(Weekday::Tue, 5), 2);
        assert_eq!(weekday_to_work_index(Weekday::Wed, 5), 3);
        assert_eq!(weekday_to_work_index(Weekday::Thu, 5), 4);
        assert_eq!(weekday_to_work_index(Weekday::Fri, 5), 5);
        assert_eq!(weekday_to_work_index(Weekday::Sat, 5), 5);
        assert_eq!(weekday_to_work_index(Weekday::Sun, 5), 5);
    }

    #[test]
    fn test_weekday_to_work_index_short_week() {
        assert_eq!(weekday_to_work_index(Weekday::Mon, 3), 1);
        assert_eq!(weekday_to_work_index(Weekday::Tue, 3), 2);
        assert_eq!(weekday_to_work_index(Weekday::Wed, 3), 3);
        assert_eq!(weekday_to_work_index(Weekday::Thu, 3), 3);
        assert_eq!(weekday_to_work_index(Weekday::Fri, 3), 3);
        assert_eq!(weekday_to_work_index(Weekday::Sat, 3), 3);
        assert_eq!(weekday_to_work_index(Weekday::Sun, 3), 3);
    }
}
