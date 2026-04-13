use super::*;
use super::tasks::build_vacation_entries;

#[test]
fn build_vacation_entries_skips_weekends() {
    // Mon June 2 to Sun June 8 2025: should produce 5 entries (Mon-Fri)
    let mon = NaiveDate::from_ymd_opt(2025, 6, 2).unwrap();
    let sun = NaiveDate::from_ymd_opt(2025, 6, 8).unwrap();
    let result = build_vacation_entries(mon, sun, 2025, 8.0, 1, 1).unwrap();
    assert_eq!(result.len(), 5);
}

#[test]
fn build_vacation_entries_skips_holidays() {
    // Thu Jul 31 + Fri Aug 1 (Bundesfeiertag / Swiss National Day) = only 1 workday
    let thu = NaiveDate::from_ymd_opt(2025, 7, 31).unwrap();
    let fri = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();
    let result = build_vacation_entries(thu, fri, 2025, 8.0, 1, 1).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].spent_date, "2025-07-31");
}

#[test]
fn build_vacation_entries_rejects_cross_year() {
    let dec = NaiveDate::from_ymd_opt(2025, 12, 30).unwrap();
    let jan = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
    let result = build_vacation_entries(dec, jan, 2025, 8.0, 1, 1);
    assert!(result.is_err());
}

#[test]
fn build_vacation_entries_rejects_weekend_only_range() {
    let sat = NaiveDate::from_ymd_opt(2025, 6, 7).unwrap();
    let sun = NaiveDate::from_ymd_opt(2025, 6, 8).unwrap();
    let result = build_vacation_entries(sat, sun, 2025, 8.0, 1, 1);
    assert!(result.is_err());
}

// ── validate_profile tests ──────────────────────────────────────────────────

fn profile_form(weekly: &str, pct: &str, holidays: &str, first_day: &str) -> SettingsFormState {
    SettingsFormState {
        weekly_hours_input: weekly.into(),
        percentage_input: pct.into(),
        holidays_input: holidays.into(),
        first_work_day_input: first_day.into(),
        ..Default::default()
    }
}

#[test]
fn validate_profile_valid() {
    let f = profile_form("41", "80", "25", "01.06.2025");
    let p = f.validate_profile().unwrap();
    assert!((p.weekly_hours - 41.0).abs() < f64::EPSILON);
    assert!((p.percentage - 0.80).abs() < f64::EPSILON);
    assert_eq!(p.holidays, 25);
    assert_eq!(p.first_work_day, Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()));
}

#[test]
fn validate_profile_empty_first_day() {
    let f = profile_form("42", "100", "25", "");
    let p = f.validate_profile().unwrap();
    assert!(p.first_work_day.is_none());
}

#[test]
fn validate_profile_comma_decimal() {
    let f = profile_form("41,5", "80,5", "25", "");
    let p = f.validate_profile().unwrap();
    assert!((p.weekly_hours - 41.5).abs() < f64::EPSILON);
    assert!((p.percentage - 0.805).abs() < f64::EPSILON);
}

#[test]
fn validate_profile_bad_hours() {
    let f = profile_form("0", "80", "25", "");
    assert!(f.validate_profile().is_err());
    let f = profile_form("200", "80", "25", "");
    assert!(f.validate_profile().is_err());
    let f = profile_form("abc", "80", "25", "");
    assert!(f.validate_profile().is_err());
}

#[test]
fn validate_profile_bad_percentage() {
    let f = profile_form("41", "0", "25", "");
    assert!(f.validate_profile().is_err());
    let f = profile_form("41", "101", "25", "");
    assert!(f.validate_profile().is_err());
}

#[test]
fn validate_profile_bad_date() {
    let f = profile_form("41", "80", "25", "2025-06-01");
    assert!(f.validate_profile().is_err());
}

// ── validate_carryover tests ────────────────────────────────────────────────

fn carryover_form(year: &str, holiday: &str, overtime: &str) -> SettingsFormState {
    SettingsFormState {
        carryover_year_input: year.into(),
        carryover_holiday_input: holiday.into(),
        carryover_overtime_input: overtime.into(),
        ..Default::default()
    }
}

#[test]
fn validate_carryover_valid() {
    let f = carryover_form("2025", "3,5", "-10,2");
    let c = f.validate_carryover().unwrap();
    assert_eq!(c.year, 2025);
    assert!((c.holiday_days - 3.5).abs() < f64::EPSILON);
    assert!((c.overtime_hours - (-10.2)).abs() < f64::EPSILON);
}

#[test]
fn validate_carryover_bad_year() {
    let f = carryover_form("1999", "0", "0");
    assert!(f.validate_carryover().is_err());
    let f = carryover_form("abc", "0", "0");
    assert!(f.validate_carryover().is_err());
}

#[test]
fn validate_carryover_bad_hours() {
    let f = carryover_form("2025", "abc", "0");
    assert!(f.validate_carryover().is_err());
    let f = carryover_form("2025", "5", "xyz");
    assert!(f.validate_carryover().is_err());
}
