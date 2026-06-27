use super::*;
use super::tasks::{build_vacation_entries, compute_budget_summaries};
use crate::harvest::models::{ClientRef, ProjectRef, TaskRef, TimeEntry, UserRef};
use crate::state::settings::YearCarryover;
use crate::stats::{HolidayStats, PeriodStats, YearBalance};

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

#[test]
fn validate_profile_max_weekly_hours() {
    // 168h/week is the maximum valid value (24h × 7 days).
    let f = profile_form("168", "100", "0", "");
    assert!(f.validate_profile().is_ok());
    // One over the limit must be rejected.
    let f = profile_form("168.01", "100", "0", "");
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
    let f = carryover_form("2025", "16,5", "-10,2");
    let c = f.validate_carryover().unwrap();
    assert_eq!(c.year, 2025);
    assert!((c.holiday_hours - 16.5).abs() < f64::EPSILON);
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

#[test]
fn validate_carryover_year_boundaries() {
    // 2000 and 2100 are both within the valid range.
    assert!(carryover_form("2000", "0", "0").validate_carryover().is_ok());
    assert!(carryover_form("2100", "0", "0").validate_carryover().is_ok());
    // One outside each boundary must be rejected.
    assert!(carryover_form("1999", "0", "0").validate_carryover().is_err());
    assert!(carryover_form("2101", "0", "0").validate_carryover().is_err());
}

// ── compute_budget_summaries tests ─────────────────────────────────────────

fn make_entry(id: i64, project_id: i64, task_id: i64, hours: f64, billable: bool) -> TimeEntry {
    TimeEntry {
        id,
        spent_date: "2025-06-01".into(),
        hours,
        hours_without_timer: None,
        rounded_hours: None,
        notes: None,
        is_locked: false,
        is_running: false,
        is_billed: false,
        approval_status: None,
        billable,
        timer_started_at: None,
        project: ProjectRef { id: project_id, name: format!("P{project_id}"), code: None },
        task: TaskRef { id: task_id, name: format!("T{task_id}") },
        client: ClientRef { id: 1, name: "Client".into() },
        user: UserRef { id: 1, name: None },
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn budget_summary_single_budget() {
    use crate::state::project_budgets::ProjectBudget;

    let budgets = vec![ProjectBudget {
        id: 1,
        name: "Test".into(),
        budget_hours: 100.0,
        project_ids: vec![10],
        task_ids: vec![],
    }];
    let entries = vec![
        make_entry(1, 10, 1, 5.0, false),
        make_entry(2, 10, 2, 3.0, false),
        make_entry(3, 20, 1, 10.0, false), // different project, should be ignored
    ];
    let summaries = compute_budget_summaries(&budgets, &entries);
    assert_eq!(summaries.len(), 1);
    assert!((summaries[0].used_hours - 8.0).abs() < f64::EPSILON);
    assert!((summaries[0].remaining_hours - 92.0).abs() < f64::EPSILON);
}

#[test]
fn budget_summary_no_matching_entries() {
    use crate::state::project_budgets::ProjectBudget;

    let budgets = vec![ProjectBudget {
        id: 1,
        name: "Empty".into(),
        budget_hours: 50.0,
        project_ids: vec![99],
        task_ids: vec![],
    }];
    let entries = vec![make_entry(1, 10, 1, 5.0, false)];
    let summaries = compute_budget_summaries(&budgets, &entries);
    assert_eq!(summaries.len(), 1);
    assert!((summaries[0].used_hours - 0.0).abs() < f64::EPSILON);
    assert!((summaries[0].remaining_hours - 50.0).abs() < f64::EPSILON);
}

#[test]
fn budget_summary_task_id_filtering() {
    use crate::state::project_budgets::ProjectBudget;

    let budgets = vec![ProjectBudget {
        id: 1,
        name: "Filtered".into(),
        budget_hours: 100.0,
        project_ids: vec![10],
        task_ids: vec![1], // only task 1
    }];
    let entries = vec![
        make_entry(1, 10, 1, 5.0, false), // matches
        make_entry(2, 10, 2, 8.0, false), // wrong task, excluded
    ];
    let summaries = compute_budget_summaries(&budgets, &entries);
    assert_eq!(summaries.len(), 1);
    assert!((summaries[0].used_hours - 5.0).abs() < f64::EPSILON);
}

#[test]
fn budget_summary_multiple_budgets() {
    use crate::state::project_budgets::ProjectBudget;

    let budgets = vec![
        ProjectBudget {
            id: 1, name: "A".into(), budget_hours: 100.0,
            project_ids: vec![10], task_ids: vec![],
        },
        ProjectBudget {
            id: 2, name: "B".into(), budget_hours: 50.0,
            project_ids: vec![20], task_ids: vec![],
        },
    ];
    let entries = vec![
        make_entry(1, 10, 1, 10.0, false),
        make_entry(2, 20, 1, 25.0, false),
    ];
    let summaries = compute_budget_summaries(&budgets, &entries);
    assert_eq!(summaries.len(), 2);
    assert!((summaries[0].used_hours - 10.0).abs() < f64::EPSILON);
    assert!((summaries[1].used_hours - 25.0).abs() < f64::EPSILON);
    assert!((summaries[1].pct_used - 0.5).abs() < f64::EPSILON);
}

#[test]
fn budget_summary_running_timer_uses_hours_field() {
    // Running timer entries: Harvest returns is_running=true and hours=accumulated.
    // compute_budget_summaries uses e.hours unconditionally — this test documents
    // and locks that semantic. If the intent ever changes to exclude in-progress
    // timer time, update this test and the implementation together.
    use crate::state::project_budgets::ProjectBudget;

    let budget = ProjectBudget {
        id: 1,
        name: "Test Project".into(),
        project_ids: vec![10],
        task_ids: vec![],
        budget_hours: 100.0,
    };

    let mut running_entry = make_entry(1, 10, 1, 3.5, true);
    running_entry.is_running = true;
    running_entry.hours_without_timer = Some(3.0); // timer has added 0.5h so far

    let summaries = compute_budget_summaries(&[budget], &[running_entry]);

    assert_eq!(summaries.len(), 1);
    // e.hours (3.5) is used, not hours_without_timer (3.0)
    assert!(
        (summaries[0].used_hours - 3.5).abs() < 1e-9,
        "expected used_hours = 3.5 (e.hours), got {}",
        summaries[0].used_hours
    );
}

#[test]
fn adj_form_validate_year_boundary_dates() {
    // Jan 1 and Dec 31 of the target year must both be accepted.
    let jan1 = OvertimeAdjustmentForm {
        date_input: "01.01.2025".into(),
        hours_input: "4".into(),
        reason_input: "Test".into(),
        ..Default::default()
    };
    assert!(jan1.validate(2025).is_ok());

    let dec31 = OvertimeAdjustmentForm {
        date_input: "31.12.2025".into(),
        hours_input: "4".into(),
        reason_input: "Test".into(),
        ..Default::default()
    };
    assert!(dec31.validate(2025).is_ok());
}

// ── BudgetForm::validate tests ─────────────────────────────────────────────

use super::project_tracking::BudgetForm;

#[test]
fn budget_form_validate_valid() {
    let form = BudgetForm {
        name_input: "  Education  ".into(),
        budget_hours_input: "70,5".into(),
        selected_projects: vec![(42, "Proj".into(), "Client".into())],
        ..Default::default()
    };
    let v = form.validate().unwrap();
    assert_eq!(v.name, "Education");
    assert!((v.budget_hours - 70.5).abs() < f64::EPSILON);
    assert_eq!(v.project_ids, vec![42]);
    assert!(v.editing_id.is_none());
}

#[test]
fn budget_form_validate_empty_name() {
    let form = BudgetForm {
        name_input: "  ".into(),
        budget_hours_input: "10".into(),
        selected_projects: vec![(1, "P".into(), "C".into())],
        ..Default::default()
    };
    assert!(form.validate().is_err());
}

#[test]
fn budget_form_validate_bad_hours() {
    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "abc".into(),
        selected_projects: vec![(1, "P".into(), "C".into())],
        ..Default::default()
    };
    assert!(form.validate().is_err());

    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "0".into(),
        selected_projects: vec![(1, "P".into(), "C".into())],
        ..Default::default()
    };
    assert!(form.validate().is_err());

    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "-5".into(),
        selected_projects: vec![(1, "P".into(), "C".into())],
        ..Default::default()
    };
    assert!(form.validate().is_err());
}

#[test]
fn budget_form_validate_no_projects() {
    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "10".into(),
        selected_projects: vec![],
        ..Default::default()
    };
    assert!(form.validate().is_err());
}

#[test]
fn budget_form_validate_preserves_editing_id() {
    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "10".into(),
        selected_projects: vec![(1, "P".into(), "C".into())],
        editing_id: Some(42),
        ..Default::default()
    };
    assert_eq!(form.validate().unwrap().editing_id, Some(42));
}

// ── OvertimeAdjustmentForm::validate tests ─────────────────────────────────

use super::stats::OvertimeAdjustmentForm;

#[test]
fn adj_form_validate_valid() {
    let form = OvertimeAdjustmentForm {
        date_input: "15.06.2025".into(),
        hours_input: "-8,5".into(),
        reason_input: " Hours payout ".into(),
        ..Default::default()
    };
    let v = form.validate(2025).unwrap();
    assert_eq!(v.date, NaiveDate::from_ymd_opt(2025, 6, 15).unwrap());
    assert!((v.hours - (-8.5)).abs() < f64::EPSILON);
    assert_eq!(v.reason, "Hours payout");
}

#[test]
fn adj_form_validate_iso_date() {
    let form = OvertimeAdjustmentForm {
        date_input: "2025-03-01".into(),
        hours_input: "4".into(),
        reason_input: "Bonus".into(),
        ..Default::default()
    };
    assert!(form.validate(2025).is_ok());
}

#[test]
fn adj_form_validate_bad_date() {
    let form = OvertimeAdjustmentForm {
        date_input: "not-a-date".into(),
        hours_input: "4".into(),
        reason_input: "Test".into(),
        ..Default::default()
    };
    assert!(form.validate(2025).is_err());
}

#[test]
fn adj_form_validate_wrong_year() {
    let form = OvertimeAdjustmentForm {
        date_input: "15.06.2024".into(),
        hours_input: "4".into(),
        reason_input: "Test".into(),
        ..Default::default()
    };
    assert!(form.validate(2025).is_err());
}

#[test]
fn adj_form_validate_zero_hours() {
    let form = OvertimeAdjustmentForm {
        date_input: "15.06.2025".into(),
        hours_input: "0".into(),
        reason_input: "Test".into(),
        ..Default::default()
    };
    assert!(form.validate(2025).is_err());
}

#[test]
fn adj_form_validate_empty_reason() {
    let form = OvertimeAdjustmentForm {
        date_input: "15.06.2025".into(),
        hours_input: "4".into(),
        reason_input: "  ".into(),
        ..Default::default()
    };
    assert!(form.validate(2025).is_err());
}

#[test]
fn validate_carryover_zero_values_are_valid() {
    // Zero carryover (explicitly clearing a balance) must be accepted.
    let f = carryover_form("2025", "0", "0");
    let c = f.validate_carryover().unwrap();
    assert_eq!(c.year, 2025);
    assert_eq!(c.holiday_hours, 0.0);
    assert_eq!(c.overtime_hours, 0.0);
}

#[test]
fn validate_carryover_negative_overtime_is_valid() {
    // A negative overtime carryover (debt from prior year) must be accepted.
    let f = carryover_form("2026", "0", "-8.2");
    let c = f.validate_carryover().unwrap();
    assert!((c.overtime_hours - (-8.2)).abs() < f64::EPSILON);
}

// ── M2-F3: validate_carryover must reject non-finite inputs ─────────────────

#[test]
fn validate_carryover_rejects_infinity_holiday() {
    let f = carryover_form("2025", "inf", "0");
    assert!(f.validate_carryover().is_err(), "inf holiday_hours must be rejected");
}

#[test]
fn validate_carryover_rejects_infinity_overtime() {
    let f = carryover_form("2025", "0", "inf");
    assert!(f.validate_carryover().is_err(), "inf overtime_hours must be rejected");
}

#[test]
fn validate_carryover_rejects_nan_holiday() {
    // "nan" parses as f64::NAN, which must be rejected.
    let f = carryover_form("2025", "nan", "0");
    assert!(f.validate_carryover().is_err(), "NaN holiday_hours must be rejected");
}

// ── M5-F2: BudgetForm::validate must reject infinity ─────────────────────────

#[test]
fn budget_form_validate_rejects_infinity() {
    let form = BudgetForm {
        name_input: "Test".into(),
        budget_hours_input: "inf".into(),
        selected_projects: vec![(1, "Proj".into(), "Client".into())],
        ..Default::default()
    };
    assert!(form.validate().is_err(), "infinite budget_hours must be rejected");
}

// ── M2-F2: effective_holiday_days_for returns 0 for pre-employment years ────

#[test]
fn effective_holiday_days_before_employment_returns_zero() {
    use crate::state::settings::Settings;
    use chrono::NaiveDate;
    let s = Settings {
        total_holiday_days_per_year: 25,
        first_work_day: Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
        ..Default::default()
    };
    // 2024 is entirely before employment started — must be 0.
    assert_eq!(s.effective_holiday_days_for(2024), 0.0,
        "year before first_work_day must return 0 entitlement");
    // 2023 also 0.
    assert_eq!(s.effective_holiday_days_for(2023), 0.0);
    // 2025 is the employment year — proration applies (non-zero).
    assert!(s.effective_holiday_days_for(2025) > 0.0);
    // 2026 is a full year — full entitlement.
    assert_eq!(s.effective_holiday_days_for(2026), 25.0);
}

// ── M2-F5: Settings::load preserves valid fields even when numeric fields are out of range ──

#[test]
fn settings_load_invalid_hours_preserves_account_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Write a settings.json where total_weekly_hours is out of range (200.0 > 168.0)
    // but all other fields (account_id, holiday_task_ids, etc.) are valid.
    let json = r#"{
        "account_id": "preserve-me",
        "default_break_minutes": 45,
        "total_weekly_hours": 200.0,
        "work_percentage": 1.0,
        "total_holiday_days_per_year": 25,
        "holiday_task_ids": [42]
    }"#;
    std::fs::write(dir.path().join("settings.json"), json).unwrap();

    let loaded = Settings::load(dir.path());
    // account_id must be preserved even though total_weekly_hours was invalid.
    assert_eq!(loaded.account_id, "preserve-me",
        "account_id must be preserved when only numeric fields are invalid");
    assert_eq!(loaded.holiday_task_ids, vec![42],
        "holiday_task_ids must be preserved");
    // The invalid weekly hours must be clamped to the default.
    assert_eq!(loaded.total_weekly_hours, 41.0,
        "invalid total_weekly_hours must be reset to default");
}

// ── State-machine tests (M2-F1, M1-F1, M2-F4) ─────────────────────────────
//
// These tests construct a minimal EasyHarvest via `test_instance`, manipulate
// its settings, dispatch a message, and assert on the resulting state.  The
// Harvest client is None so any background tasks short-circuit to Task::none().

fn zero_balance() -> YearBalance {
    YearBalance {
        period: PeriodStats {
            total_hours: 0.0,
            expected_hours: 0.0,
            balance_hours: 0.0,
            working_days_expected: 0,
            days_with_entries: 0,
        },
        carryover_hours: 0.0,
        manual_adjustments_hours: 0.0,
        total_balance: 10.0,
    }
}

fn zero_holiday_stats() -> HolidayStats {
    HolidayStats { days_taken: 0.0, days_remaining: 5.0, total_days: 25.0 }
}

/// M2-F1: CarryoverReset must preserve entries marked is_user_defined and
/// remove all auto-computed ones.
#[test]
fn carryover_reset_preserves_user_defined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.first_work_day = Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

    // A manually entered carryover — must survive Reset.
    app.settings.carryover.insert(2025, YearCarryover {
        holiday_hours: 16.0,
        overtime_hours: 4.0,
        legacy_holiday_days: 0.0,
        is_user_defined: true,
    });
    // An auto-computed carryover — must be purged by Reset.
    app.settings.carryover.insert(2026, YearCarryover {
        holiday_hours: 8.0,
        overtime_hours: 2.0,
        legacy_holiday_days: 0.0,
        is_user_defined: false,
    });

    let _ = app.update_settings(SettingsMsg::CarryoverReset);

    assert!(
        app.settings.carryover.contains_key(&2025),
        "user-defined 2025 entry must survive CarryoverReset"
    );
    assert_eq!(
        app.settings.carryover[&2025].holiday_hours, 16.0,
        "user-defined holiday_hours must be unchanged"
    );
    assert!(
        !app.settings.carryover.contains_key(&2026),
        "auto-computed 2026 entry must be purged by CarryoverReset"
    );
}

/// M2-F4 / M1-F2: CarryoverSyncLoaded (Ok path) must NOT overwrite an entry
/// that the user manually set (is_user_defined = true).
#[test]
fn carryover_sync_loaded_skips_user_defined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.first_work_day = Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

    // Suppose the user manually entered carryover for 2026.
    app.settings.carryover.insert(2026, YearCarryover {
        holiday_hours: 24.0,
        overtime_hours: -8.0,
        legacy_holiday_days: 0.0,
        is_user_defined: true,
    });

    // Sync completes for year 2025; handler would normally write into 2026.
    let _ = app.update_settings(SettingsMsg::CarryoverSyncLoaded(
        2025,
        Ok((zero_balance(), zero_holiday_stats())),
    ));

    // The user-defined 2026 entry must be completely unchanged.
    let entry = &app.settings.carryover[&2026];
    assert_eq!(entry.holiday_hours, 24.0,
        "user-defined holiday_hours must not be overwritten by sync");
    assert_eq!(entry.overtime_hours, -8.0,
        "user-defined overtime_hours must not be overwritten by sync");
    assert!(entry.is_user_defined,
        "is_user_defined flag must remain true");
}

/// M1-F1: CarryoverSyncLoaded (Err path) must insert a zero tombstone for
/// `year + 1` so that CarryoverSyncStart does not re-fire the same year
/// indefinitely.
#[test]
fn carryover_sync_loaded_err_inserts_tombstone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.first_work_day = Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

    // No carryover for 2026 yet.
    assert!(!app.settings.carryover.contains_key(&2026));

    // Simulate a network failure while syncing year 2025.
    let _ = app.update_settings(SettingsMsg::CarryoverSyncLoaded(
        2025,
        Err("network error".to_string()),
    ));

    // A tombstone (zero-value, auto-computed) entry must be inserted for 2026
    // so the sync chain can advance past this year.
    assert!(
        app.settings.carryover.contains_key(&2026),
        "tombstone must be inserted for year+1 on sync failure"
    );
    let tombstone = &app.settings.carryover[&2026];
    assert!(
        !tombstone.is_user_defined,
        "tombstone must not be marked is_user_defined so future syncs can overwrite it"
    );
}

/// M1-F1 edge: CarryoverSyncLoaded Err must NOT overwrite an existing
/// user-defined entry with a zero tombstone.
#[test]
fn carryover_sync_loaded_err_does_not_overwrite_user_defined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.first_work_day = Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

    // User manually set carryover for 2026 before sync ran.
    app.settings.carryover.insert(2026, YearCarryover {
        holiday_hours: 20.0,
        overtime_hours: 5.0,
        legacy_holiday_days: 0.0,
        is_user_defined: true,
    });

    // Sync fails for 2025.
    let _ = app.update_settings(SettingsMsg::CarryoverSyncLoaded(
        2025,
        Err("timeout".to_string()),
    ));

    // The user's value must be untouched — zero tombstone must NOT replace it.
    let entry = &app.settings.carryover[&2026];
    assert_eq!(entry.holiday_hours, 20.0,
        "sync error tombstone must not overwrite user-defined entry");
    assert!(entry.is_user_defined);
}

// ── M5-F1: vacation prefers non-billable project ──────────────────────────────

/// M5-F1: When the holiday task appears in multiple project assignments, the
/// non-billable one must be chosen so vacation entries don't appear on a client
/// invoice.  The billable assignment is placed first to prove first-match is
/// NOT used.
///
/// The selection logic is extracted into `vacation::select_holiday_project_id`
/// so it can be tested without an Iced runtime or a Harvest client.
#[test]
fn vacation_submit_prefers_non_billable_project() {
    use crate::harvest::models::{ProjectAssignment, ProjectTaskAssignment};
    use super::vacation::select_holiday_project_id;

    let task_id: i64 = 42;

    // Billable project — placed first to verify first-match is not used.
    let billable_pa = ProjectAssignment {
        id: 1,
        project: ProjectRef { id: 100, name: "Billable Project".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![ProjectTaskAssignment {
            id: 1,
            task: TaskRef { id: task_id, name: "Holiday".into() },
            is_active: true,
            billable: Some(true),
        }],
    };
    // Non-billable project — placed second; must win the selection.
    let non_billable_pa = ProjectAssignment {
        id: 2,
        project: ProjectRef { id: 200, name: "Internal Project".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![ProjectTaskAssignment {
            id: 2,
            task: TaskRef { id: task_id, name: "Holiday".into() },
            is_active: true,
            billable: Some(false),
        }],
    };

    // Drive the real production selection function (called by FormSubmit handler).
    let assignments = vec![billable_pa, non_billable_pa];
    let project_id = select_holiday_project_id(&assignments, task_id);

    assert_eq!(
        project_id,
        Some(200),
        "non-billable project (id=200) must be preferred over the billable one (id=100)"
    );

    // Additional: when ALL assignments are billable, fall back to the first one.
    let billable_pa2 = ProjectAssignment {
        id: 3,
        project: ProjectRef { id: 300, name: "Billable 2".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![ProjectTaskAssignment {
            id: 3,
            task: TaskRef { id: task_id, name: "Holiday".into() },
            is_active: true,
            billable: Some(true),
        }],
    };
    let all_billable = vec![
        ProjectAssignment {
            id: 4,
            project: ProjectRef { id: 400, name: "First Billable".into(), code: None },
            client: ClientRef { id: 1, name: "Client".into() },
            is_active: true,
            task_assignments: vec![ProjectTaskAssignment {
                id: 4,
                task: TaskRef { id: task_id, name: "Holiday".into() },
                is_active: true,
                billable: Some(true),
            }],
        },
        billable_pa2,
    ];
    let fallback_id = select_holiday_project_id(&all_billable, task_id);
    assert_eq!(fallback_id, Some(400),
        "when all projects are billable, first match (id=400) must be selected");
}

// ── M1-F3: recompute_vacation_summary with expected_per_day = 0.0 ─────────────

/// M1-F3: When work_percentage is 0.0 the expected hours per day is 0.0.
/// Division by expected_per_day must be guarded so used_days / booked_days are
/// 0.0, not NaN or ∞.
#[test]
fn vacation_summary_zero_epd_no_nan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    // 0% work percentage → expected_hours_per_day() == 0.0
    app.settings.work_percentage = 0.0;
    app.settings.holiday_task_ids = vec![99];

    // A holiday entry in the past (before test_instance's current_date 2025-01-15).
    let mut entry = make_entry(1, 1, 99, 8.0, false);
    entry.spent_date = "2025-01-10".into();
    app.vacation.entries = vec![entry];
    app.vacation.year = 2025;

    app.recompute_vacation_summary();

    let summary = app.vacation.summary.expect("summary must be computed");
    assert!(!summary.used_days.is_nan(), "used_days must not be NaN");
    assert!(!summary.used_days.is_infinite(), "used_days must not be Infinite");
    assert!(!summary.booked_days.is_nan(), "booked_days must not be NaN");
    assert!(!summary.booked_days.is_infinite(), "booked_days must not be Infinite");
    assert_eq!(summary.used_days, 0.0,
        "used_days must be 0.0 when expected_per_day is 0 (guard against division by zero)");
    assert_eq!(summary.booked_days, 0.0,
        "booked_days must be 0.0 when expected_per_day is 0");
}

// ── M1-F4: month_summaries uses YTD-capped entries ───────────────────────────

/// M1-F4: `month_summaries_ytd` must only reflect entries up to `balance_end`.
/// An entry dated in the future must not inflate any month's total_hours.
///
/// The filtering is done inside the extracted `tasks::month_summaries_ytd`
/// helper (which `load_stats_task` calls) rather than in the caller.  Passing
/// `all_entries` (with the future entry) exercises the fix directly: if the
/// ytd filter were removed from `month_summaries_ytd`, July would get 8 h and
/// the assertion below would fail.
#[test]
fn month_summaries_excludes_future_entries() {
    use super::tasks::month_summaries_ytd;

    // balance_end = 2025-01-15 (matches test_instance's current_date).
    let balance_end = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    // Past entry: falls within YTD range — must appear in January totals.
    let mut past = make_entry(1, 1, 1, 5.0, false);
    past.spent_date = "2025-01-10".into();

    // Future entry: beyond balance_end — must be excluded from monthly totals.
    let mut future = make_entry(2, 1, 1, 8.0, false);
    future.spent_date = "2025-07-01".into();

    // Pass all_entries (unfiltered) — the helper must apply the ytd cap itself.
    let all_entries = vec![past, future];
    let summaries = month_summaries_ytd(&all_entries, 2025, None, 8.0, &[], balance_end);

    // July must have zero hours — the future entry must be excluded.
    let july = summaries.iter().find(|m| m.month == 7).expect("July summary must exist");
    assert_eq!(july.total_hours, 0.0,
        "future entry in July must not appear when month_summaries_ytd applies the ytd cap");

    // January must still reflect the past entry.
    let jan = summaries.iter().find(|m| m.month == 1).expect("January summary must exist");
    assert!(jan.total_hours > 0.0, "past entry in January must be reflected");
}

// ── M4-F1: TimerStarted clears is_running on other entries ───────────────────

/// M4-F1: When a timer is successfully started on one entry, all other entries
/// in `self.entries` must have `is_running` set to false.
#[test]
fn timer_started_clears_other_running_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    // test_instance uses current_date = 2025-01-15.
    let today = "2025-01-15";

    // Entry A — currently running.
    let mut entry_a = make_entry(1, 1, 1, 2.0, false);
    entry_a.spent_date = today.into();
    entry_a.is_running = true;

    // Entry B — idle; timer will be started on this one.
    let mut entry_b = make_entry(2, 1, 1, 3.0, false);
    entry_b.spent_date = today.into();
    entry_b.is_running = false;

    app.entries = vec![entry_a, entry_b];

    // Simulate the API response that starts entry B's timer.
    let mut updated_b = make_entry(2, 1, 1, 3.0, false);
    updated_b.spent_date = today.into();
    updated_b.is_running = true;

    let _ = app.update_entries(EntryMsg::TimerStarted(Ok(updated_b)));

    // Entry A must now have is_running = false.
    let a = app.entries.iter().find(|e| e.id == 1).expect("entry A must exist");
    assert!(!a.is_running,
        "entry A must have is_running cleared after another entry's timer is started");

    // Entry B must have is_running = true (the started one).
    let b = app.entries.iter().find(|e| e.id == 2).expect("entry B must exist");
    assert!(b.is_running,
        "entry B must remain is_running = true after its timer is started");
}

// ── M4-F3: AssignmentsLoaded generation guard ────────────────────────────────

/// M4-F3: A stale AssignmentsLoaded response (generation < current) must be
/// silently discarded without overwriting self.assignments.
#[test]
fn assignments_loaded_discards_stale_generation() {
    use crate::harvest::models::ProjectAssignment;

    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    // Simulate having made two load requests; generation is now 2.
    app.assignments_gen = 2;
    // No assignments yet.
    assert!(app.assignments.is_empty());

    // A response arrives from the first (stale) request.
    let stale_assignment = ProjectAssignment {
        id: 99,
        project: ProjectRef { id: 999, name: "Stale Project".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![],
    };
    let _ = app.update_entries(EntryMsg::AssignmentsLoaded(1, Ok(vec![stale_assignment])));

    assert!(
        app.assignments.is_empty(),
        "stale AssignmentsLoaded (gen=1) must not update assignments when current gen=2"
    );
}

// ── M5-F3: EditSave rejects break outside work-day envelope ──────────────────

/// M5-F3: EditSave must reject a break whose start time falls before the work
/// day's start_time and set error_banner instead of saving.
#[test]
fn work_day_break_outside_envelope_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    // Set up edit inputs: work day 09:00–17:00, break at 07:00 (before start).
    app.work_day_edit.start_input = "09:00".into();
    app.work_day_edit.end_input   = "17:00".into();
    app.work_day_edit.break_inputs = vec![("07:00".into(), "08:00".into())];

    let _ = app.update_work_day(WorkDayMsg::EditSave);

    assert!(
        app.error_banner.is_some(),
        "EditSave must set error_banner when a break starts before the work day start_time"
    );
}

// ── M4-F4: PageChanged clears date_picker.open ───────────────────────────────

/// M4-F4: Navigating to any page must close the date-picker popup so it does
/// not remain open on pages where it is not rendered.
#[test]
fn page_changed_clears_date_picker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    app.date_picker.open = true;

    let _ = app.update(Message::Nav(NavMsg::PageChanged(Page::Stats)));

    assert!(
        !app.date_picker.open,
        "date_picker.open must be false after PageChanged"
    );
}

// ── M4-F6: PageChanged must clear work_day_edit ────────────────────────────────

#[test]
fn page_changed_clears_work_day_edit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    app.work_day_edit.edit_mode = true;
    app.work_day_edit.start_input = "08:00".into();

    let _ = app.update(Message::Nav(NavMsg::PageChanged(Page::Stats)));

    assert!(
        !app.work_day_edit.edit_mode,
        "edit_mode must be cleared on PageChanged"
    );
    assert!(
        app.work_day_edit.start_input.is_empty(),
        "start_input must be cleared on PageChanged"
    );
}

// ── M2-F7: WizardProfileContinue must reject holidays > 365 ───────────────────

#[test]
fn wizard_profile_continue_rejects_holidays_above_365() {
    use crate::app::settings::SettingsMsg;
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(tmp.path());

    // Set up a valid profile in the form except for holidays
    app.settings_form.weekly_hours_input = "40".into();
    app.settings_form.percentage_input = "100".into();
    app.settings_form.first_work_day_input = "01.01.2020".into();
    app.settings_form.holidays_input = "366".into(); // one above maximum

    let _ = app.update_settings(SettingsMsg::WizardProfileContinue);

    assert!(
        app.settings_form.profile_error.is_some(),
        "expected profile_error to be set for holidays > 365"
    );
    assert_ne!(
        app.settings.total_holiday_days_per_year, 366,
        "settings must not have been updated with the invalid value"
    );
}

// ── M6-F1: WizardBack from step 2 navigates forward into the app ──────────────

/// M6-F1 regression test: dispatching WizardBack while wizard_step == 2 must
/// navigate to Page::Day (skip forward), not back to the credentials screen.
/// The bug was that WizardBack decremented wizard_step without setting page.
#[test]
fn wizard_back_from_step2_navigates_to_day() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(tmp.path());

    // Simulate the state after successful credential entry: client is set and
    // the wizard has advanced to step 2 (profile configuration).
    // test_instance sets client = None; stub it so the wizard_step == 2 path
    // matches real conditions (client present, profile step open).
    app.wizard_step = 2;
    app.page = Page::Settings;

    let _ = app.update_settings(SettingsMsg::WizardBack);

    // Skip must navigate into the app, not back to credentials.
    assert_eq!(
        app.page,
        Page::Day,
        "WizardBack from step 2 must set page = Day (skip forward into app)"
    );
    // wizard_step must be != 2 so the profile wizard is no longer rendered.
    assert_ne!(
        app.wizard_step, 2,
        "WizardBack from step 2 must leave wizard_step != 2 to hide the wizard overlay"
    );
}

// ── M6-F2: ProjectSelected (project_tracking) uses active-only index ─────────

/// M6-F2 verified not a bug: the view enumerates AFTER filter(is_active) and
/// the handler uses .filter(is_active).nth(idx) — both use the same
/// active-only subsequence. This test documents that with inactive assignments
/// interspersed, clicking on an active one still selects the correct project.
#[test]
fn project_tracking_project_selected_correct_with_inactive_interspersed() {
    use crate::harvest::models::ProjectAssignment;

    let dir = tempfile::TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());

    // assignments = [inactive_B, active_A, active_C]
    // active-only subsequence: [active_A (idx 0), active_C (idx 1)]
    let inactive_b = ProjectAssignment {
        id: 10,
        project: ProjectRef { id: 1000, name: "Inactive B".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: false,
        task_assignments: vec![],
    };
    let active_a = ProjectAssignment {
        id: 20,
        project: ProjectRef { id: 2000, name: "Active A".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![],
    };
    let active_c = ProjectAssignment {
        id: 30,
        project: ProjectRef { id: 3000, name: "Active C".into(), code: None },
        client: ClientRef { id: 1, name: "Client".into() },
        is_active: true,
        task_assignments: vec![],
    };
    app.assignments = vec![inactive_b, active_a, active_c];

    // Open the budget form
    let _ = app.update_project_tracking(ProjectTrackingMsg::ShowForm);
    assert!(app.project_tracking.form.is_some());

    // Click the 2nd active project (active_C, active-only idx 1).
    // The inactive_B at the start must NOT shift the index.
    let _ = app.update_project_tracking(ProjectTrackingMsg::ProjectSelected(1));

    let selected = &app.project_tracking.form.as_ref().unwrap().selected_projects;
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].0, 3000,
        "active-only idx 1 must resolve to Active C (project_id 3000), not the inactive or Active A"
    );
}

// ── M6-F3: EntryForm.submitting is cleared on Created/Updated error ───────────

/// M6-F3: When an API response arrives with an error, the submitting flag must
/// be cleared so the user can retry without dismissing the form.
#[test]
fn entry_form_submitting_cleared_on_created_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());

    // Manually set up a form with submitting = true (as if a request is in flight).
    let mut form = EntryForm::new();
    form.submitting = true;
    app.entry_form = Some(form);

    // Simulate a network error response for a create attempt.
    let _ = app.update_entries(EntryMsg::Created(Err("network timeout".into())));

    let f = app.entry_form.as_ref()
        .expect("form must still be present after a create error");
    assert!(!f.submitting,
        "submitting must be reset to false after Created(Err(...)) so the user can retry");
    assert!(f.error.is_some(),
        "error message must be set for the user to see");
}

/// M6-F3: Same as above for the update path.
#[test]
fn entry_form_submitting_cleared_on_updated_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());

    let mut form = EntryForm::new();
    form.editing_id = Some(42);
    form.submitting = true;
    app.entry_form = Some(form);

    let _ = app.update_entries(EntryMsg::Updated(Err("server error".into())));

    let f = app.entry_form.as_ref()
        .expect("form must still be present after an update error");
    assert!(!f.submitting,
        "submitting must be reset to false after Updated(Err(...)) so the user can retry");
    assert!(f.error.is_some());
}

// ── M6-F4: vacation_row division guard when expected_per_day == 0.0 ──────────
//
// vacation_row is a view function (fn(&EasyHarvest) -> Element) that requires
// a full Iced runtime and cannot be called in unit tests.  The state-level
// guard (recompute_vacation_summary uses `if expected_per_day > 0.0 { … }`) is
// already exercised by `vacation_summary_zero_epd_no_nan` above.  The per-row
// guard in vacation_view.rs is verified by code inspection.
// Spec reference: docs/superpowers/specs — 06-ui.md F4.
