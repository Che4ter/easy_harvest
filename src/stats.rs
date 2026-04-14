use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};

use crate::harvest::models::TimeEntry;
use crate::state::settings::PublicHoliday;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Aggregated stats for a calendar period.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodStats {
    /// Sum of all entry hours.
    pub total_hours: f64,
    /// Expected hours = working days × hours_per_day − public holiday credits.
    pub expected_hours: f64,
    /// total_hours − expected_hours. Positive = overtime, negative = undertime.
    pub balance_hours: f64,
    /// Calendar working days (Mon–Fri) in the period (up to `as_of`).
    pub working_days_expected: u32,
    /// Distinct dates that have at least one entry (includes weekends).
    pub days_with_entries: u32,
}

/// Holiday usage for a calendar year.
#[derive(Debug, Clone, PartialEq)]
pub struct HolidayStats {
    /// Fractional days taken (holiday hours ÷ hours_per_day).
    pub days_taken: f64,
    /// total_days − days_taken.
    pub days_remaining: f64,
    /// Effective total (base × work_percentage + carryover).
    pub total_days: f64,
}

/// Hours summary for a single day.
#[derive(Debug, Clone)]
pub struct DailySummary {
    pub date: NaiveDate,
    pub total_hours: f64,
    pub entry_count: usize,
}

/// Year-to-date balance including overtime carryover from the previous year.
#[derive(Debug, Clone, PartialEq)]
pub struct YearBalance {
    pub period: PeriodStats,
    pub carryover_hours: f64,
    /// Sum of manual overtime adjustments (positive = added, negative = deducted).
    pub manual_adjustments_hours: f64,
    /// carryover + period.balance_hours + manual_adjustments_hours.
    pub total_balance: f64,
}

// ---------------------------------------------------------------------------
// Core calculations (pure functions — no I/O, no clock)
// ---------------------------------------------------------------------------

/// Calculate period stats for entries that fall within [from, to] inclusive.
///
/// `as_of` clamps the upper bound for expected hours so that querying a
/// mid-month or future range does not penalise unworked future days.  Pass
/// `chrono::Local::now().naive_local().date()` for live dashboards.
///
/// Public holidays on weekdays within [from, min(to, as_of)] reduce expected
/// hours by their stored credit.
pub fn period_stats(
    entries: &[TimeEntry],
    from: NaiveDate,
    to: NaiveDate,
    expected_hours_per_day: f64,
    public_holidays: &[PublicHoliday],
    as_of: NaiveDate,
) -> PeriodStats {
    let effective_to = to.min(as_of);

    let in_range: Vec<&TimeEntry> = entries
        .iter()
        .filter(|e| parse_date(&e.spent_date).map(|d| d >= from && d <= to).unwrap_or(false))
        .collect();

    let total_hours: f64 = in_range.iter().map(|e| e.hours).sum();
    let working_days_expected = working_days_in_range(from, effective_to);

    // Subtract public holiday credits that fall on weekdays within the range.
    let holiday_credit: f64 = public_holidays
        .iter()
        .filter(|h| {
            h.date >= from
                && h.date <= effective_to
                && h.date.weekday().num_days_from_monday() < 5
        })
        .map(|h| h.credit_hours(expected_hours_per_day))
        .sum();

    let expected_hours =
        (working_days_expected as f64 * expected_hours_per_day - holiday_credit).max(0.0);

    let distinct_dates: HashSet<&str> = in_range.iter().map(|e| e.spent_date.as_str()).collect();
    let days_with_entries = distinct_dates.len() as u32;

    PeriodStats {
        total_hours,
        expected_hours,
        balance_hours: total_hours - expected_hours,
        working_days_expected,
        days_with_entries,
    }
}

/// Calculate holiday usage for entries in `year` whose task ID is in
/// `holiday_task_ids`. Hours are converted to days using `hours_per_day`.
pub fn holiday_stats(
    entries: &[TimeEntry],
    year: i32,
    holiday_task_ids: &[i64],
    total_days: f64,
    hours_per_day: f64,
) -> HolidayStats {
    let holiday_hours: f64 = entries
        .iter()
        .filter(|e| {
            let in_year = parse_date(&e.spent_date)
                .map(|d| d.year() == year)
                .unwrap_or(false);
            in_year && holiday_task_ids.contains(&e.task.id)
        })
        .map(|e| e.hours)
        .sum();

    let days_taken = if hours_per_day > 0.0 {
        holiday_hours / hours_per_day
    } else {
        0.0
    };

    HolidayStats {
        days_taken,
        days_remaining: total_days - days_taken,
        total_days,
    }
}

/// Year-to-date balance: period stats from `effective_start` (or Jan 1) to `as_of`, plus carryover.
///
/// `effective_start` overrides the Jan 1 lower bound — pass `Some(first_work_day)` when the
/// employee did not work the full year so expected hours are prorated from their start date.
#[allow(clippy::too_many_arguments)]
pub fn year_to_date_balance(
    entries: &[TimeEntry],
    year: i32,
    effective_start: Option<NaiveDate>,
    expected_hours_per_day: f64,
    public_holidays: &[PublicHoliday],
    carryover_hours: f64,
    manual_adjustments_hours: f64,
    as_of: NaiveDate,
) -> YearBalance {
    let (year_start, to) = year_bounds(year);
    let from = effective_start
        .filter(|d| d.year() == year && *d > year_start)
        .unwrap_or(year_start);
    let period = period_stats(entries, from, to, expected_hours_per_day, public_holidays, as_of);
    YearBalance {
        carryover_hours,
        manual_adjustments_hours,
        total_balance: carryover_hours + period.balance_hours + manual_adjustments_hours,
        period,
    }
}

/// Group entries by date and return sorted daily summaries.
pub fn daily_summaries(entries: &[TimeEntry]) -> Vec<DailySummary> {
    let mut by_date: BTreeMap<NaiveDate, (f64, usize)> = BTreeMap::new();

    for e in entries {
        if let Ok(date) = parse_date(&e.spent_date) {
            let slot = by_date.entry(date).or_insert((0.0, 0));
            slot.0 += e.hours;
            slot.1 += 1;
        }
    }

    by_date
        .into_iter()
        .map(|(date, (total_hours, entry_count))| DailySummary {
            date,
            total_hours,
            entry_count,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Count working days (Monday–Friday) in [from, to] inclusive.
pub fn working_days_in_range(from: NaiveDate, to: NaiveDate) -> u32 {
    if from > to {
        return 0;
    }
    let total = (to - from).num_days() as u32 + 1;
    let from_dow = from.weekday().num_days_from_monday(); // 0=Mon … 6=Sun
    let remainder = total % 7;
    let weekend_in_remainder = (0..remainder)
        .filter(|&i| {
            let dow = (from_dow + i) % 7;
            dow == 5 || dow == 6
        })
        .count() as u32;
    let full_weeks = total / 7;
    total - full_weeks * 2 - weekend_in_remainder
}

/// Monday of the ISO week containing `date`.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

/// Monday and Sunday (inclusive) of the week containing `date`.
pub fn week_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = week_start(date);
    (start, start + Duration::days(6))
}

/// First and last day of a calendar month.
pub fn month_bounds(year: i32, month: u32) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(year, month, 1).expect("invalid month");
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("invalid month");
    (start, next - Duration::days(1))
}

/// First and last day of a calendar year.
pub fn year_bounds(year: i32) -> (NaiveDate, NaiveDate) {
    (
        NaiveDate::from_ymd_opt(year, 1, 1).expect("invalid year"),
        NaiveDate::from_ymd_opt(year, 12, 31).expect("invalid year"),
    )
}

fn parse_date(s: &str) -> Result<NaiveDate, chrono::ParseError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;
    use crate::harvest::models::{ClientRef, ProjectRef, TaskRef, TimeEntry, UserRef};

    fn entry(date: &str, hours: f64, task_id: i64, locked: bool) -> TimeEntry {
        TimeEntry {
            id: 0,
            spent_date: date.to_string(),
            hours,
            hours_without_timer: None,
            rounded_hours: None,
            notes: None,
            is_locked: locked,
            is_running: false,
            is_billed: false,
            approval_status: None,
            billable: false,
            timer_started_at: None,
            project: ProjectRef { id: 1, name: "Proj".into(), code: None },
            task: TaskRef { id: task_id, name: "Task".into() },
            client: ClientRef { id: 1, name: "Client".into() },
            user: UserRef { id: 1, name: None },
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        }
    }

    /// A date far in the future so that as_of clamping has no effect on
    /// historical test data.
    fn far_future() -> NaiveDate {
        NaiveDate::from_ymd_opt(2099, 12, 31).unwrap()
    }

    // --- working_days_in_range ---

    #[test]
    fn test_working_days_full_week() {
        let mon = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();  // Monday
        let fri = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(); // Friday
        assert_eq!(working_days_in_range(mon, fri), 5);
    }

    #[test]
    fn test_working_days_includes_weekend() {
        let mon = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();  // Monday
        let sun = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap(); // Sunday
        assert_eq!(working_days_in_range(mon, sun), 5);
    }

    #[test]
    fn test_working_days_single_day_weekday() {
        let wed = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        assert_eq!(working_days_in_range(wed, wed), 1);
    }

    #[test]
    fn test_working_days_single_day_weekend() {
        let sat = NaiveDate::from_ymd_opt(2025, 1, 11).unwrap();
        assert_eq!(working_days_in_range(sat, sat), 0);
    }

    #[test]
    fn test_working_days_reversed_range() {
        let a = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let b = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        assert_eq!(working_days_in_range(a, b), 0);
    }

    #[test]
    fn test_working_days_whole_month_jan_2025() {
        // Jan 2025: 31 days, 23 working days
        let from = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
        assert_eq!(working_days_in_range(from, to), 23);
    }

    // --- period_stats ---

    #[test]
    fn test_period_stats_basic() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false), // Monday
            entry("2025-01-07", 7.5, 1, false), // Tuesday
            entry("2025-01-08", 8.0, 1, false), // Wednesday
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let stats = period_stats(&entries, from, to, 8.0, &[], far_future());

        assert_eq!(stats.total_hours, 23.5);
        assert_eq!(stats.expected_hours, 40.0); // 5 working days × 8h
        assert_eq!(stats.balance_hours, 23.5 - 40.0);
        assert_eq!(stats.working_days_expected, 5);
        assert_eq!(stats.days_with_entries, 3);
    }

    #[test]
    fn test_period_stats_overtime() {
        let entries = vec![
            entry("2025-01-06", 10.0, 1, false),
            entry("2025-01-07", 10.0, 1, false),
            entry("2025-01-08", 10.0, 1, false),
            entry("2025-01-09", 10.0, 1, false),
            entry("2025-01-10", 10.0, 1, false),
        ];
        let (from, to) = week_bounds(NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        let stats = period_stats(&entries, from, to, 8.0, &[], far_future());

        assert_eq!(stats.total_hours, 50.0);
        assert_eq!(stats.expected_hours, 40.0);
        assert_eq!(stats.balance_hours, 10.0);
    }

    #[test]
    fn test_period_stats_filters_out_of_range() {
        let entries = vec![
            entry("2025-01-05", 8.0, 1, false), // Sunday — outside range
            entry("2025-01-06", 8.0, 1, false), // Monday — in range
            entry("2025-01-13", 8.0, 1, false), // next week — outside range
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let stats = period_stats(&entries, from, to, 8.0, &[], far_future());

        assert_eq!(stats.total_hours, 8.0);
        assert_eq!(stats.days_with_entries, 1);
    }

    #[test]
    fn test_period_stats_weekend_entries_counted() {
        // Working on Saturday — should appear in days_with_entries and total_hours,
        // but not inflate working_days_expected.
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false), // Monday
            entry("2025-01-11", 4.0, 1, false), // Saturday
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        let stats = period_stats(&entries, from, to, 8.0, &[], far_future());

        assert_eq!(stats.total_hours, 12.0);
        assert_eq!(stats.days_with_entries, 2);
        assert_eq!(stats.working_days_expected, 5); // only Mon–Fri
        assert_eq!(stats.balance_hours, 12.0 - 40.0);
    }

    // --- as_of clamping ---

    #[test]
    fn test_period_stats_clamps_to_as_of() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false), // Monday
            entry("2025-01-07", 8.0, 1, false), // Tuesday
            entry("2025-01-08", 8.0, 1, false), // Wednesday
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        // as_of is Wednesday — only 3 working days expected
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        let stats = period_stats(&entries, from, to, 8.0, &[], as_of);

        assert_eq!(stats.working_days_expected, 3);
        assert_eq!(stats.expected_hours, 24.0);
        // Still counts all entries through `to` (Fri), not just through as_of
        assert_eq!(stats.total_hours, 24.0);
        assert_eq!(stats.balance_hours, 0.0);
    }

    #[test]
    fn test_period_stats_fully_future_range() {
        // Entire range is past as_of → 0 expected hours
        let entries = vec![
            entry("2025-02-03", 8.0, 1, false),
        ];
        let from = NaiveDate::from_ymd_opt(2025, 2, 3).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 2, 7).unwrap();
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
        let stats = period_stats(&entries, from, to, 8.0, &[], as_of);

        assert_eq!(stats.working_days_expected, 0);
        assert_eq!(stats.expected_hours, 0.0);
        assert_eq!(stats.total_hours, 8.0);
        assert_eq!(stats.balance_hours, 8.0);
    }

    // --- public holidays ---

    #[test]
    fn test_period_stats_subtracts_public_holiday() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false), // Mon
            entry("2025-01-07", 8.0, 1, false), // Tue
            entry("2025-01-09", 8.0, 1, false), // Thu
            entry("2025-01-10", 8.0, 1, false), // Fri
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        // Wed Jan 8 is a public holiday (full day)
        let holidays = vec![PublicHoliday {
            date: NaiveDate::from_ymd_opt(2025, 1, 8).unwrap(),
            name: "Test".into(),
            half_day: false,
        }];
        let stats = period_stats(&entries, from, to, 8.0, &holidays, far_future());

        assert_eq!(stats.working_days_expected, 5);
        assert_eq!(stats.expected_hours, 32.0); // 5 × 8 − 8 = 32
        assert_eq!(stats.total_hours, 32.0);
        assert_eq!(stats.balance_hours, 0.0);
    }

    #[test]
    fn test_period_stats_half_day_holiday() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 4.0, 1, false), // half day worked
        ];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();
        // Tue Jan 7 is a half-day holiday (4h credit at 8h/day)
        let holidays = vec![PublicHoliday {
            date: NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            name: "Half Day".into(),
            half_day: true,
        }];
        let stats = period_stats(&entries, from, to, 8.0, &holidays, far_future());

        assert_eq!(stats.expected_hours, 12.0); // 2 × 8 − 4 = 12
        assert_eq!(stats.total_hours, 12.0);
        assert_eq!(stats.balance_hours, 0.0);
    }

    #[test]
    fn test_period_stats_weekend_holiday_ignored() {
        // A public holiday on Saturday should not reduce expected hours.
        let entries = vec![entry("2025-01-06", 8.0, 1, false)];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        let holidays = vec![PublicHoliday {
            date: NaiveDate::from_ymd_opt(2025, 1, 11).unwrap(), // Saturday
            name: "Sat Holiday".into(),
            half_day: false,
        }];
        let stats = period_stats(&entries, from, to, 8.0, &holidays, far_future());

        assert_eq!(stats.expected_hours, 40.0); // unchanged — Sat holiday has no effect
    }

    #[test]
    fn test_period_stats_holiday_outside_range_ignored() {
        let entries = vec![entry("2025-01-06", 8.0, 1, false)];
        let from = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let holidays = vec![PublicHoliday {
            date: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(), // next week
            name: "Out of Range".into(),
            half_day: false,
        }];
        let stats = period_stats(&entries, from, to, 8.0, &holidays, far_future());

        assert_eq!(stats.expected_hours, 40.0);
    }

    // --- year_to_date_balance ---

    #[test]
    fn test_year_to_date_with_carryover() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 8.0, 1, false),
            entry("2025-01-08", 8.0, 1, false),
            entry("2025-01-09", 8.0, 1, false),
            entry("2025-01-10", 8.0, 1, false),
        ];
        // as_of = Jan 10 → Jan 1 (Wed) + Jan 2 (Thu) + Jan 3 (Fri) + Jan 6-10 = 8 working days
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let ytd = year_to_date_balance(&entries, 2025, None, 8.0, &[], 5.5, 0.0, as_of);

        assert_eq!(ytd.period.total_hours, 40.0);
        assert_eq!(ytd.period.expected_hours, 64.0); // 8 days × 8h
        assert_eq!(ytd.carryover_hours, 5.5);
        // total = 5.5 + (40 − 64) = −18.5
        assert!((ytd.total_balance - (-18.5)).abs() < 1e-9);
    }

    #[test]
    fn test_year_to_date_with_negative_carryover() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 8.0, 1, false),
            entry("2025-01-08", 8.0, 1, false),
            entry("2025-01-09", 8.0, 1, false),
            entry("2025-01-10", 8.0, 1, false),
        ];
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let ytd = year_to_date_balance(&entries, 2025, None, 8.0, &[], -10.0, 0.0, as_of);

        // total = −10 + (40 − 64) = −34
        assert!((ytd.total_balance - (-34.0)).abs() < 1e-9);
    }

    #[test]
    fn test_year_to_date_with_holidays() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 8.0, 1, false),
            entry("2025-01-08", 8.0, 1, false),
            entry("2025-01-09", 8.0, 1, false),
            entry("2025-01-10", 8.0, 1, false),
        ];
        // Jan 1 (Wed) + Jan 2 (Thu) are public holidays
        let holidays = vec![
            PublicHoliday {
                date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                name: "Neujahr".into(),
                half_day: false,
            },
            PublicHoliday {
                date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                name: "Berchtoldstag".into(),
                half_day: false,
            },
        ];
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let ytd = year_to_date_balance(&entries, 2025, None, 8.0, &holidays, 0.0, 0.0, as_of);

        // 8 working days × 8h − 16h holidays = 48h expected
        assert_eq!(ytd.period.expected_hours, 48.0);
        // 40h worked − 48h expected = −8h
        assert_eq!(ytd.period.balance_hours, -8.0);
        assert_eq!(ytd.total_balance, -8.0);
    }

    // --- year_to_date_balance with manual adjustments ---

    #[test]
    fn test_year_to_date_with_positive_adjustment() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 8.0, 1, false),
            entry("2025-01-08", 8.0, 1, false),
            entry("2025-01-09", 8.0, 1, false),
            entry("2025-01-10", 8.0, 1, false),
        ];
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        // 5.0h carryover + 10.0h manual adjustment
        let ytd = year_to_date_balance(&entries, 2025, None, 8.0, &[], 5.0, 10.0, as_of);
        assert_eq!(ytd.manual_adjustments_hours, 10.0);
        // total = 5.0 + (40 − 64) + 10.0 = −9.0
        assert!((ytd.total_balance - (-9.0)).abs() < 1e-9);
    }

    #[test]
    fn test_year_to_date_with_negative_adjustment() {
        let entries = vec![
            entry("2025-01-06", 8.0, 1, false),
            entry("2025-01-07", 8.0, 1, false),
            entry("2025-01-08", 8.0, 1, false),
            entry("2025-01-09", 8.0, 1, false),
            entry("2025-01-10", 8.0, 1, false),
        ];
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        // −8h payout adjustment
        let ytd = year_to_date_balance(&entries, 2025, None, 8.0, &[], 0.0, -8.0, as_of);
        assert_eq!(ytd.manual_adjustments_hours, -8.0);
        // total = 0.0 + (40 − 64) + (−8.0) = −32.0
        assert!((ytd.total_balance - (-32.0)).abs() < 1e-9);
    }

    // --- holiday_stats ---

    #[test]
    fn test_holiday_stats_full_week() {
        let holiday_task = 999;
        let entries = vec![
            entry("2025-01-06", 8.0, holiday_task, false),
            entry("2025-01-07", 8.0, holiday_task, false),
            entry("2025-01-08", 8.0, 1, false), // not holiday
        ];
        let stats = holiday_stats(&entries, 2025, &[holiday_task], 30.0, 8.0);

        assert_eq!(stats.days_taken, 2.0);
        assert_eq!(stats.days_remaining, 28.0);
        assert_eq!(stats.total_days, 30.0);
    }

    #[test]
    fn test_holiday_stats_partial_day() {
        let holiday_task = 999;
        let entries = vec![entry("2025-06-01", 4.0, holiday_task, false)];
        let stats = holiday_stats(&entries, 2025, &[holiday_task], 25.0, 8.0);

        assert_eq!(stats.days_taken, 0.5);
        assert_eq!(stats.days_remaining, 24.5);
    }

    #[test]
    fn test_holiday_stats_filters_other_years() {
        let holiday_task = 999;
        let entries = vec![
            entry("2024-12-31", 8.0, holiday_task, false), // wrong year
            entry("2025-01-02", 8.0, holiday_task, false),
        ];
        let stats = holiday_stats(&entries, 2025, &[holiday_task], 25.0, 8.0);

        assert_eq!(stats.days_taken, 1.0);
    }

    #[test]
    fn test_holiday_stats_zero_hours_per_day() {
        let entries = vec![entry("2025-01-06", 8.0, 999, false)];
        let stats = holiday_stats(&entries, 2025, &[999], 25.0, 0.0);
        assert_eq!(stats.days_taken, 0.0);
    }

    // --- daily_summaries ---

    #[test]
    fn test_daily_summaries_groups_same_day() {
        let entries = vec![
            entry("2025-01-06", 3.0, 1, false),
            entry("2025-01-06", 4.5, 2, false),
            entry("2025-01-07", 8.0, 1, false),
        ];
        let summaries = daily_summaries(&entries);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].date, NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        assert_eq!(summaries[0].total_hours, 7.5);
        assert_eq!(summaries[0].entry_count, 2);
        assert_eq!(summaries[1].total_hours, 8.0);
    }

    // --- week / month / year bounds ---

    #[test]
    fn test_week_bounds() {
        let wed = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        let (mon, sun) = week_bounds(wed);
        assert_eq!(mon.weekday(), Weekday::Mon);
        assert_eq!(sun.weekday(), Weekday::Sun);
        assert_eq!((sun - mon).num_days(), 6);
    }

    #[test]
    fn test_month_bounds() {
        let (start, end) = month_bounds(2025, 2);
        assert_eq!(start, NaiveDate::from_ymd_opt(2025, 2, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2025, 2, 28).unwrap());

        let (_, end_jan) = month_bounds(2025, 1);
        assert_eq!(end_jan, NaiveDate::from_ymd_opt(2025, 1, 31).unwrap());
    }

    #[test]
    fn test_month_bounds_december_wraps() {
        let (start, end) = month_bounds(2025, 12);
        assert_eq!(start, NaiveDate::from_ymd_opt(2025, 12, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

}
