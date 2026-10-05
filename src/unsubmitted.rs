//! Detection of past weeks that still contain unsubmitted time.
//!
//! Harvest's API cannot submit a timesheet, so the app can only warn and link
//! to the web timesheet. Everything here is pure so it is unit-testable
//! without a network.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};

use crate::harvest::models::TimeEntry;

/// Weeks named in the banner before the rest collapse into "+N more".
const MAX_LISTED_WEEKS: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct UnsubmittedWeek {
    pub monday: NaiveDate,
    /// Last due day: the week's Sunday, or the month end that cuts it short.
    pub through: NaiveDate,
    pub hours: f64,
}

fn week_monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
}

/// Last day that must already be submitted: the Sunday before the current
/// week, or the end of last month when that is later — a month ending
/// mid-week is submitted up to its last day, before the week is over.
pub fn submission_cutoff(today: NaiveDate) -> NaiveDate {
    let last_sunday = week_monday(today) - Duration::days(1);
    let last_month_end = today.with_day(1).expect("day 1 exists") - Duration::days(1);
    last_sunday.max(last_month_end)
}

/// Group unsubmitted entries up to `submission_cutoff(today)` by ISO week,
/// sum hours, oldest first. Filters on `approval_status` itself so it is
/// correct even without the server-side filter.
pub fn unsubmitted_weeks(entries: &[TimeEntry], today: NaiveDate) -> Vec<UnsubmittedWeek> {
    let cutoff = submission_cutoff(today);
    let mut by_week: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for e in entries {
        if e.approval_status.as_deref() != Some("unsubmitted") {
            continue;
        }
        let Ok(date) = NaiveDate::parse_from_str(&e.spent_date, "%Y-%m-%d") else {
            continue;
        };
        if date <= cutoff {
            *by_week.entry(week_monday(date)).or_insert(0.0) += e.hours;
        }
    }
    by_week
        .into_iter()
        .map(|(monday, hours)| UnsubmittedWeek {
            monday,
            through: (monday + Duration::days(6)).min(cutoff),
            hours,
        })
        .collect()
}

/// "28", or "52/2025" when the week's ISO year differs from today's.
fn week_number(monday: NaiveDate, today: NaiveDate) -> String {
    let iso = monday.iso_week();
    if iso.year() == today.iso_week().year() {
        iso.week().to_string()
    } else {
        format!("{}/{}", iso.week(), iso.year())
    }
}

/// "6–12 Jul", "29 Jun–5 Jul" across two months, or "31 Aug" for one day.
fn date_range(from: NaiveDate, to: NaiveDate) -> String {
    if from == to {
        format!("{} {}", to.day(), to.format("%b"))
    } else if from.month() == to.month() {
        format!("{}–{} {}", from.day(), to.day(), to.format("%b"))
    } else {
        format!("{} {}–{} {}", from.day(), from.format("%b"), to.day(), to.format("%b"))
    }
}

pub fn banner_text(weeks: &[UnsubmittedWeek], today: NaiveDate) -> String {
    match weeks {
        [] => String::new(),
        [w] => format!(
            "Week {} ({}) has {:.1}h unsubmitted",
            week_number(w.monday, today),
            date_range(w.monday, w.through),
            w.hours,
        ),
        _ => {
            let listed: Vec<String> = weeks
                .iter()
                .take(MAX_LISTED_WEEKS)
                .map(|w| format!("W{}", week_number(w.monday, today)))
                .collect();
            let mut text = format!("{} weeks unsubmitted: {}", weeks.len(), listed.join(", "));
            if weeks.len() > MAX_LISTED_WEEKS {
                text.push_str(&format!(" +{} more", weeks.len() - MAX_LISTED_WEEKS));
            }
            text
        }
    }
}

/// Harvest web timesheet for the week starting `monday`.
pub fn week_url(base_uri: &str, monday: NaiveDate) -> String {
    format!("{}/time/week/{}", base_uri.trim_end_matches('/'), monday.format("%Y/%m/%d"))
}

/// Trim, strip trailing slashes and default to `https://`; `None` when empty.
pub fn normalize_web_address(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        None
    } else if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Some(trimmed.to_string())
    } else {
        Some(format!("https://{trimmed}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harvest::models::{ClientRef, ProjectRef, TaskRef, UserRef};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn entry(date: &str, hours: f64, status: Option<&str>) -> TimeEntry {
        TimeEntry {
            id: 0,
            spent_date: date.to_string(),
            hours,
            hours_without_timer: None,
            rounded_hours: None,
            notes: None,
            is_locked: false,
            is_running: false,
            is_billed: false,
            approval_status: status.map(str::to_string),
            billable: false,
            timer_started_at: None,
            project: ProjectRef { id: 1, name: "P".into(), code: None },
            task: TaskRef { id: 1, name: "T".into() },
            client: ClientRef { id: 1, name: "C".into() },
            user: UserRef { id: 1, name: None },
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn week(monday: NaiveDate, hours: f64) -> UnsubmittedWeek {
        UnsubmittedWeek { monday, through: monday + Duration::days(6), hours }
    }

    /// Week cut short by a month end: only days up to `through` are due.
    fn partial(monday: NaiveDate, through: NaiveDate, hours: f64) -> UnsubmittedWeek {
        UnsubmittedWeek { monday, through, hours }
    }

    const U: Option<&str> = Some("unsubmitted");

    #[test]
    fn cutoff_excludes_the_open_current_week() {
        // Wed 2026-10-07 → Sun 2026-10-04
        assert_eq!(submission_cutoff(d(2026, 10, 7)), d(2026, 10, 4));
        // Monday itself → the day before
        assert_eq!(submission_cutoff(d(2026, 10, 5)), d(2026, 10, 4));
        // Sunday → the Sunday a week earlier (today's week is still open)
        assert_eq!(submission_cutoff(d(2026, 10, 11)), d(2026, 10, 4));
    }

    #[test]
    fn groups_monday_and_sunday_into_one_week_and_sums_hours() {
        let entries = [entry("2026-07-06", 3.0, U), entry("2026-07-12", 5.0, U)];
        assert_eq!(unsubmitted_weeks(&entries, d(2026, 10, 7)), vec![week(d(2026, 7, 6), 8.0)]);
    }

    #[test]
    fn sorts_multiple_weeks_oldest_first() {
        let entries = [entry("2026-09-01", 1.0, U), entry("2026-07-07", 2.0, U)];
        assert_eq!(
            unsubmitted_weeks(&entries, d(2026, 10, 7)),
            vec![week(d(2026, 7, 6), 2.0), week(d(2026, 8, 31), 1.0)],
        );
    }

    #[test]
    fn drops_current_week_on_sunday() {
        // today = Sun 2026-10-11; its week starts Mon 2026-10-05.
        let entries = [entry("2026-10-05", 4.0, U), entry("2026-10-04", 2.0, U)];
        assert_eq!(unsubmitted_weeks(&entries, d(2026, 10, 11)), vec![week(d(2026, 9, 28), 2.0)]);
    }

    #[test]
    fn groups_across_iso_year_boundary() {
        // Mon 2025-12-29 and Sun 2026-01-04 are both ISO week 1 of 2026.
        let entries = [entry("2025-12-29", 1.0, U), entry("2026-01-04", 1.0, U)];
        assert_eq!(unsubmitted_weeks(&entries, d(2026, 3, 1)), vec![week(d(2025, 12, 29), 2.0)]);
    }

    #[test]
    fn ignores_other_statuses_and_bad_dates() {
        let entries = [
            entry("2026-07-06", 1.0, Some("submitted")),
            entry("2026-07-06", 1.0, Some("approved")),
            entry("2026-07-06", 1.0, None),
            entry("not-a-date", 1.0, U),
        ];
        assert!(unsubmitted_weeks(&entries, d(2026, 10, 7)).is_empty());
    }

    #[test]
    fn empty_input_gives_no_weeks() {
        assert!(unsubmitted_weeks(&[], d(2026, 10, 7)).is_empty());
    }

    #[test]
    fn banner_text_single_week() {
        assert_eq!(
            banner_text(&[week(d(2026, 7, 6), 8.0)], d(2026, 10, 7)),
            "Week 28 (6–12 Jul) has 8.0h unsubmitted",
        );
    }

    #[test]
    fn banner_text_single_week_spanning_months() {
        assert_eq!(
            banner_text(&[week(d(2026, 6, 29), 1.5)], d(2026, 10, 7)),
            "Week 27 (29 Jun–5 Jul) has 1.5h unsubmitted",
        );
    }

    #[test]
    fn banner_text_several_weeks() {
        let weeks = [week(d(2026, 7, 6), 1.0), week(d(2026, 8, 31), 1.0), week(d(2026, 9, 21), 1.0)];
        assert_eq!(banner_text(&weeks, d(2026, 10, 7)), "3 weeks unsubmitted: W28, W36, W39");
    }

    #[test]
    fn banner_text_marks_other_year() {
        let weeks = [week(d(2025, 12, 22), 1.0), week(d(2026, 7, 6), 1.0)];
        assert_eq!(banner_text(&weeks, d(2026, 10, 7)), "2 weeks unsubmitted: W52/2025, W28");
        assert_eq!(
            banner_text(&[week(d(2025, 12, 22), 2.0)], d(2026, 10, 7)),
            "Week 52/2025 (22–28 Dec) has 2.0h unsubmitted",
        );
    }

    #[test]
    fn banner_text_caps_long_lists() {
        let weeks: Vec<_> = (0..8).map(|i| week(d(2026, 1, 5) + Duration::weeks(i), 1.0)).collect();
        assert_eq!(
            banner_text(&weeks, d(2026, 10, 7)),
            "8 weeks unsubmitted: W2, W3, W4, W5, W6 +3 more",
        );
    }

    #[test]
    fn week_url_is_zero_padded() {
        assert_eq!(
            week_url("https://acme.harvestapp.com", d(2026, 7, 6)),
            "https://acme.harvestapp.com/time/week/2026/07/06",
        );
        assert_eq!(
            week_url("https://acme.harvestapp.com/", d(2026, 7, 6)),
            "https://acme.harvestapp.com/time/week/2026/07/06",
        );
    }

    #[test]
    fn normalize_web_address_variants() {
        let want = Some("https://acme.harvestapp.com".to_string());
        assert_eq!(normalize_web_address("acme.harvestapp.com"), want);
        assert_eq!(normalize_web_address("  https://acme.harvestapp.com/ "), want);
        assert_eq!(normalize_web_address("https://acme.harvestapp.com"), want);
        assert_eq!(normalize_web_address("   "), None);
        assert_eq!(normalize_web_address(""), None);
    }

    #[test]
    fn cutoff_is_last_sunday_in_an_ordinary_week() {
        // Mon 2026-10-05: September ended Wed 30th, already before last Sunday.
        assert_eq!(submission_cutoff(d(2026, 10, 5)), d(2026, 10, 4));
    }

    #[test]
    fn cutoff_moves_to_month_end_mid_week() {
        // September 2026 ends on Wednesday; from Thursday on Mon–Wed are due.
        assert_eq!(submission_cutoff(d(2026, 10, 1)), d(2026, 9, 30));
        assert_eq!(submission_cutoff(d(2026, 10, 4)), d(2026, 9, 30));
    }

    #[test]
    fn cutoff_month_ending_on_saturday_or_sunday() {
        // October 2026 ends on Saturday: on Sunday Mon–Sat are due.
        assert_eq!(submission_cutoff(d(2026, 11, 1)), d(2026, 10, 31));
        // May 2026 ends on Sunday: same as the ordinary weekly rule.
        assert_eq!(submission_cutoff(d(2026, 6, 3)), d(2026, 5, 31));
    }

    #[test]
    fn cutoff_first_of_month_on_monday_and_new_year() {
        // Mon 2026-06-01: both rules give Sunday 31 May.
        assert_eq!(submission_cutoff(d(2026, 6, 1)), d(2026, 5, 31));
        // Thu 2026-01-01: December 2025 ended Wednesday.
        assert_eq!(submission_cutoff(d(2026, 1, 1)), d(2025, 12, 31));
    }

    #[test]
    fn month_end_makes_part_of_current_week_due() {
        // Thu 2026-10-01: Tue 29 Sep is due, Thu 1 Oct is not.
        let entries = [entry("2026-09-29", 6.0, U), entry("2026-10-01", 2.0, U)];
        assert_eq!(
            unsubmitted_weeks(&entries, d(2026, 10, 1)),
            vec![partial(d(2026, 9, 28), d(2026, 9, 30), 6.0)],
        );
    }

    #[test]
    fn banner_text_shows_partial_week_range() {
        let weeks = [partial(d(2026, 9, 28), d(2026, 9, 30), 6.0)];
        assert_eq!(banner_text(&weeks, d(2026, 10, 1)), "Week 40 (28–30 Sep) has 6.0h unsubmitted");
    }

    #[test]
    fn banner_text_single_due_day() {
        // August 2026 ends on Monday: on Tuesday only Mon 31 Aug is due.
        let weeks = unsubmitted_weeks(&[entry("2026-08-31", 4.0, U)], d(2026, 9, 1));
        assert_eq!(banner_text(&weeks, d(2026, 9, 1)), "Week 36 (31 Aug) has 4.0h unsubmitted");
    }
}
