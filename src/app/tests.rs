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
        app.carryover_sync_gen,
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
        app.carryover_sync_gen,
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
        app.carryover_sync_gen,
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

// ── FillRemaining targets worked time, not the expected daily-hours setting ──

/// Regression test: FillRemaining must propose the gap between the work-day
/// timer's clocked hours and what's booked, not `max(worked, expected)`.
/// The latter meant that early in the day — before worked hours reached the
/// expected daily target — Fill proposed the full daily target instead of
/// actual worked time, leading to overbooking.
#[test]
fn fill_remaining_targets_worked_time_not_expected_daily_hours() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());

    // Default settings (41h/week, 100%) give an 8.2h expected daily target,
    // comfortably larger than the ~1h17m worked so far.
    assert!((app.settings.expected_hours_per_day() - 8.2).abs() < 1e-9);

    let now = chrono::Local::now().naive_local().time();
    // `NaiveTime` subtraction wraps at midnight instead of erroring, so
    // blindly subtracting 77 minutes near local midnight would wrap to a
    // "start" time-of-day that looks later than `now`, making
    // worked_duration() see start > now and return zero. Clamp to midnight
    // instead, and compute the expected worked duration the same way, so
    // the assertion stays exact no matter what time of day the suite runs.
    let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let target_start = now - chrono::Duration::minutes(77);
    let start = if target_start <= now { target_start } else { midnight };
    // Round the same way production does (seconds/3600 hours, then *60
    // rounded) rather than truncating, so this matches exactly.
    let worked_secs = now.signed_duration_since(start).num_seconds();
    let worked_mins = (worked_secs as f64 / 60.0).round() as i64;

    let mut work_day = crate::state::work_day::WorkDay::new(app.current_date);
    work_day.start(start);
    app.work_day_store.set(work_day);

    app.entry_form = Some(EntryForm::new());
    let _ = app.update_entries(EntryMsg::FillRemaining);

    let hours_input = app.entry_form.as_ref().unwrap().hours_input.clone();
    let expected = format!("{}:{:02}", worked_mins / 60, worked_mins % 60);
    assert_eq!(hours_input, expected,
        "Fill must propose the worked time, not the larger expected daily target (8:12)");
}

// ── M6-F4: vacation_row division guard when expected_per_day == 0.0 ──────────
//
// vacation_row is a view function (fn(&EasyHarvest) -> Element) that requires
// a full Iced runtime and cannot be called in unit tests.  The state-level
// guard (recompute_vacation_summary uses `if expected_per_day > 0.0 { … }`) is
// already exercised by `vacation_summary_zero_epd_no_nan` above.  The per-row
// guard in vacation_view.rs is verified by code inspection.
// Spec reference: docs/superpowers/specs — 06-ui.md F4.

// ── Task 3: TeamSettings wired into app struct/startup ───────────────────────

#[test]
fn test_instance_has_empty_team_roster() {
    let dir = tempfile::tempdir().unwrap();
    let app = EasyHarvest::test_instance(dir.path());
    assert!(app.team_settings.members.is_empty());
    assert!(!app.settings.team_lead_mode);
}

// ── Task 4: TeamMsg / update_team ─────────────────────────────────────────────

use crate::app::TeamMsg;
use crate::state::team::TeamMember;
use crate::state::overtime_adjustments::OvertimeAdjustmentStore;

fn team_member(id: i64, name: &str) -> TeamMember {
    TeamMember {
        harvest_user_id: id,
        display_name: name.into(),
        work_percentage: 1.0,
        total_holiday_days_per_year: 25,
        holiday_task_ids: vec![],
        first_work_day: None,
        carryover: std::collections::HashMap::new(),
        overtime_adjustments: OvertimeAdjustmentStore::default(),
    }
}

#[test]
fn add_member_rejects_empty_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.add_form.id_input = "12345".into();
    let _ = app.update_team(TeamMsg::AddMember);
    assert!(app.team.add_form.error.is_some());
    assert!(app.team_settings.members.is_empty());
}

#[test]
fn add_member_rejects_non_numeric_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.add_form.name_input = "Alex".into();
    app.team.add_form.id_input = "not-a-number".into();
    let _ = app.update_team(TeamMsg::AddMember);
    assert!(app.team.add_form.error.is_some());
    assert!(app.team_settings.members.is_empty());
}

#[test]
fn add_member_rejects_duplicate_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(99, "Existing"));
    app.team.add_form.name_input = "Alex".into();
    app.team.add_form.id_input = "99".into();
    let _ = app.update_team(TeamMsg::AddMember);
    assert!(app.team.add_form.error.is_some());
    assert_eq!(app.team_settings.members.len(), 1);
}

#[test]
fn add_member_succeeds_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.add_form.name_input = "Alex".into();
    app.team.add_form.id_input = "555".into();
    let _ = app.update_team(TeamMsg::AddMember);
    assert!(app.team.add_form.error.is_none());
    assert_eq!(app.team_settings.members.len(), 1);
    assert_eq!(app.team_settings.members[0].harvest_user_id, 555);
    assert_eq!(app.team_settings.members[0].display_name, "Alex");
    // New members default to 100% work percentage regardless of the team
    // lead's own current percentage.
    assert_eq!(app.team_settings.members[0].work_percentage, 1.0);

    let reloaded = crate::state::team::TeamSettings::load(dir.path());
    assert_eq!(reloaded.members.len(), 1, "AddMember must persist to disk");
}

#[test]
fn remove_member_clears_stats_and_forms() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Bye"));
    app.team.stats.insert(7, crate::app::TeamMemberStats::default());
    app.team.adjustment_forms.insert(7, crate::app::OvertimeAdjustmentForm::default());

    let _ = app.update_team(TeamMsg::RemoveMember(7));

    assert!(app.team_settings.members.is_empty());
    assert!(!app.team.stats.contains_key(&7));
    assert!(!app.team.adjustment_forms.contains_key(&7));
    let reloaded = crate::state::team::TeamSettings::load(dir.path());
    assert!(reloaded.members.is_empty(), "RemoveMember must persist to disk");
}

#[test]
fn work_percentage_save_parses_and_persists_valid_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Alex"));
    let _ = app.update_team(TeamMsg::WorkPercentageInputChanged(7, "50".into()));
    let _ = app.update_team(TeamMsg::WorkPercentageSave(7));

    assert_eq!(app.team_settings.member(7).unwrap().work_percentage, 0.5);
    assert!(!app.team.work_percentage_inputs.contains_key(&7));
    let reloaded = crate::state::team::TeamSettings::load(dir.path());
    assert_eq!(reloaded.member(7).unwrap().work_percentage, 0.5);
}

#[test]
fn work_percentage_save_rejects_out_of_range_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Alex"));
    let _ = app.update_team(TeamMsg::WorkPercentageInputChanged(7, "150".into()));
    let _ = app.update_team(TeamMsg::WorkPercentageSave(7));

    assert_eq!(app.team_settings.member(7).unwrap().work_percentage, 1.0,
        "invalid input must not change the stored percentage");
    assert!(app.error_banner.is_some());
}

#[test]
fn first_work_day_detected_sets_member_and_seeds_carryover() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Alex"));
    let detected = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();

    let _ = app.update_team(TeamMsg::FirstWorkDayDetected(7, Ok(Some(detected))));

    assert_eq!(app.team_settings.member(7).unwrap().first_work_day, Some(detected));
    assert!(app.team_settings.member(7).unwrap().carryover.contains_key(&2026));
    let reloaded = crate::state::team::TeamSettings::load(dir.path());
    assert_eq!(reloaded.member(7).unwrap().first_work_day, Some(detected));
}

#[test]
fn first_work_day_detected_none_leaves_member_unset_without_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Alex"));

    let _ = app.update_team(TeamMsg::FirstWorkDayDetected(7, Ok(None)));

    assert_eq!(app.team_settings.member(7).unwrap().first_work_day, None);
    assert!(app.error_banner.is_none(), "no entries yet is expected, not an error");
}

#[test]
fn first_work_day_detected_err_sets_error_banner() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(7, "Alex"));

    let _ = app.update_team(TeamMsg::FirstWorkDayDetected(7, Err("boom".into())));

    assert_eq!(app.team_settings.member(7).unwrap().first_work_day, None);
    assert!(app.error_banner.is_some());
}

#[test]
fn member_stats_loaded_ignores_stale_generation() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.r#gen = 5;
    let _ = app.update_team(TeamMsg::MemberStatsLoaded(
        4, // stale gen
        1,
        Err("should be ignored".into()),
    ));
    assert!(!app.team.stats.contains_key(&1));
}

#[test]
fn member_stats_loaded_stores_result_for_current_generation() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.r#gen = 1;
    let _ = app.update_team(TeamMsg::MemberStatsLoaded(1, 1, Err("boom".into())));
    let s = app.team.stats.get(&1).expect("entry must be created");
    assert_eq!(s.error.as_deref(), Some("boom"));
    assert!(!s.loading);
}

#[test]
fn impersonation_start_sets_id_navigates_to_day_and_bumps_gens() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.page = Page::Stats;
    let before_entries = app.entries_gen;
    let before_stats = app.stats_gen;
    let before_vacation = app.vacation_gen;
    let before_assignments = app.assignments_gen;
    let before_billable = app.billable_gen;
    let before_project_tracking = app.project_tracking_gen;

    let _ = app.update_team(TeamMsg::ImpersonationStart(9));

    assert_eq!(app.impersonating, Some(9));
    assert_eq!(app.page, Page::Day);
    assert_eq!(app.entries_gen, before_entries + 1);
    assert_eq!(app.stats_gen, before_stats + 1);
    assert_eq!(app.vacation_gen, before_vacation + 1);
    assert_eq!(app.assignments_gen, before_assignments, "assignments_gen must not be bumped");
    assert_eq!(app.billable_gen, before_billable, "billable_gen must not be bumped");
    assert_eq!(app.project_tracking_gen, before_project_tracking, "project_tracking_gen must not be bumped");
}

#[test]
fn impersonation_start_clears_open_forms_and_pending_delete() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.entry_form = Some(EntryForm::new());
    app.pending_delete = Some(1);
    app.overtime_adj_form = Some(OvertimeAdjustmentForm::default());
    app.vacation.form = Some(VacationForm::new());

    let _ = app.update_team(TeamMsg::ImpersonationStart(9));

    assert!(app.entry_form.is_none());
    assert!(app.pending_delete.is_none());
    assert!(app.overtime_adj_form.is_none());
    assert!(app.vacation.form.is_none());
}

#[test]
fn impersonation_exit_clears_id_keeps_page_and_bumps_gens() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);
    app.page = Page::Stats;
    let before_entries = app.entries_gen;
    let before_stats = app.stats_gen;
    let before_vacation = app.vacation_gen;

    let _ = app.update_team(TeamMsg::ImpersonationExit);

    assert_eq!(app.impersonating, None);
    assert_eq!(app.page, Page::Stats, "exit must not force navigation");
    assert_eq!(app.entries_gen, before_entries + 1);
    assert_eq!(app.stats_gen, before_stats + 1);
    assert_eq!(app.vacation_gen, before_vacation + 1);
}

#[test]
fn remove_member_clears_impersonation_of_removed_member() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);

    let _ = app.update_team(TeamMsg::RemoveMember(9));

    assert_eq!(app.impersonating, None);
}

#[test]
fn remove_member_leaves_other_impersonation_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.team_settings.members.push(team_member(10, "Alex"));
    app.impersonating = Some(9);

    let _ = app.update_team(TeamMsg::RemoveMember(10));

    assert_eq!(app.impersonating, Some(9));
}

// Removing the member currently being impersonated must run the *full*
// exit path, not just clear the flag: otherwise the app keeps rendering the
// removed member's cached entries/vacation/stats with `impersonating == None`
// — no read-only banner, every mutation control re-enabled, against data that
// belongs to somebody else.
#[test]
fn remove_member_of_impersonated_member_performs_full_impersonation_exit() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);

    // Seed the cached state that belongs to the impersonated member.
    app.entries = vec![make_entry(1, 10, 1, 5.0, false)];
    app.vacation.entries = vec![make_entry(2, 10, 1, 8.0, false)];
    app.vacation.summary = Some(crate::app::VacationSummary {
        used_days: 1.0,
        booked_days: 1.0,
        days_remaining: 24.0,
        total_days: 25.0,
        carryover_days: 0.0,
    });
    app.year_balance = Some(zero_balance());
    app.holiday_stats = Some(zero_holiday_stats());
    app.month_summaries = Some(Vec::new());
    app.entry_form = Some(EntryForm::new());
    app.pending_delete = Some(1);
    app.overtime_adj_form = Some(OvertimeAdjustmentForm::default());
    app.vacation.form = Some(VacationForm::new());

    let before_entries = app.entries_gen;
    let before_stats = app.stats_gen;
    let before_vacation = app.vacation_gen;

    let _ = app.update_team(TeamMsg::RemoveMember(9));

    assert_eq!(app.impersonating, None);
    assert!(app.team_settings.member(9).is_none());
    // Everything ImpersonationExit clears must actually be cleared.
    assert!(app.entries.is_empty(), "cached entries of the removed member must be dropped");
    assert!(app.vacation.entries.is_empty());
    assert!(app.vacation.summary.is_none());
    assert!(app.year_balance.is_none());
    assert!(app.holiday_stats.is_none());
    assert!(app.month_summaries.is_none());
    assert!(app.entry_form.is_none());
    assert!(app.pending_delete.is_none());
    assert!(app.overtime_adj_form.is_none());
    assert!(app.vacation.form.is_none());
    assert_eq!(app.entries_gen, before_entries + 1);
    assert_eq!(app.stats_gen, before_stats + 1);
    assert_eq!(app.vacation_gen, before_vacation + 1);
}

// `ImpersonationExit` clears `error_banner`, so the delegation must happen
// *before* `save_team_or_warn()` — otherwise removing the impersonated member
// while the settings file cannot be written silently swallows the save error.
#[test]
fn remove_impersonated_member_keeps_save_failure_banner() {
    let dir = tempfile::tempdir().unwrap();
    // A regular file where the data dir should be ⇒ every save fails.
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.data_dir = blocked;
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);

    let _ = app.update_team(TeamMsg::RemoveMember(9));

    assert_eq!(app.impersonating, None, "the exit still runs");
    assert!(
        app.error_banner.is_some(),
        "a failed team-settings save must stay visible, not be wiped by the impersonation exit"
    );
}

// ── effective_user_id: the branch's most safety-critical resolution ──────────
//
// `effective_user_id()` decides *whose* Harvest data every fetch targets.
// Both call sites (`load_entries_task`, `load_vacation_task`) sit behind a
// `self.client.is_none()` early return, so no state-machine test can reach
// them; these test the resolution directly instead.

#[test]
fn effective_user_id_prefers_impersonated_member() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.harvest_user_id = Some(1);
    app.impersonating = Some(9);

    assert_eq!(app.effective_user_id(), Some(9),
        "while impersonating, fetches must target the member, not the lead");

    // Also true when the lead's own id was never resolved.
    app.harvest_user_id = None;
    assert_eq!(app.effective_user_id(), Some(9));
}

#[test]
fn effective_user_id_falls_back_to_own_id_when_not_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = None;
    app.harvest_user_id = Some(1);

    assert_eq!(app.effective_user_id(), Some(1));

    app.harvest_user_id = None;
    assert_eq!(app.effective_user_id(), None);
}

#[test]
fn impersonated_member_looks_up_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));

    assert!(app.impersonated_member().is_none());
    app.impersonating = Some(9);
    assert_eq!(app.impersonated_member().unwrap().display_name, "Sam");
    app.impersonating = Some(404);
    assert!(app.impersonated_member().is_none());
}

#[test]
fn page_changed_to_billable_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.page = Page::Day;
    let before_gen = app.billable_gen;

    let _ = app.update(Message::Nav(NavMsg::PageChanged(Page::Billable)));

    assert_eq!(app.page, Page::Day, "navigation to Billable must be rejected while impersonating");
    assert_eq!(app.billable_gen, before_gen);
}

#[test]
fn page_changed_to_project_tracking_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.page = Page::Day;
    let before_gen = app.project_tracking_gen;

    let _ = app.update(Message::Nav(NavMsg::PageChanged(Page::ProjectTracking)));

    assert_eq!(app.page, Page::Day);
    assert_eq!(app.project_tracking_gen, before_gen);
}

#[test]
fn page_changed_to_billable_still_works_when_not_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = None;
    app.page = Page::Day;

    let _ = app.update(Message::Nav(NavMsg::PageChanged(Page::Billable)));

    assert_eq!(app.page, Page::Billable);
}

// ── Task 3: Day page re-scoping + entry-mutation guards ──────────────────────

#[test]
fn load_entries_task_is_noop_without_client_regardless_of_impersonation() {
    // load_entries_task always short-circuits on `self.client.is_none()`
    // (test_instance has no client), so the fetch itself can't be observed
    // here. What *is* observable is the id the task would fetch for — see
    // `effective_user_id_prefers_impersonated_member` for the direct
    // coverage of that resolution.
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.harvest_user_id = Some(1);
    app.impersonating = Some(9);

    assert_eq!(app.effective_user_id(), Some(9),
        "with a client, this task would fetch the impersonated member's entries");

    let _ = app.load_entries_task();

    assert!(app.entries.is_empty(), "no client ⇒ nothing is fetched or applied");
}

#[test]
fn entry_show_form_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);

    let _ = app.update_entries(EntryMsg::ShowForm);

    assert!(app.entry_form.is_none());
}

#[test]
fn entry_edit_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.entries = vec![make_entry(1, 10, 1, 5.0, false)];

    let _ = app.update_entries(EntryMsg::Edit(1));

    assert!(app.entry_form.is_none());
}

#[test]
fn entry_submit_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.entry_form = Some(EntryForm::new());
    let before = app.entries.clone();

    let _ = app.update_entries(EntryMsg::Submit);

    assert_eq!(app.entries.len(), before.len());
    assert!(app.entry_form.is_some(), "guard must return before touching the form");
}

#[test]
fn entry_delete_request_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);

    let _ = app.update_entries(EntryMsg::DeleteRequest(1));

    assert!(app.pending_delete.is_none());
}

#[test]
fn entry_delete_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.entries = vec![make_entry(1, 10, 1, 5.0, false)];

    let _ = app.update_entries(EntryMsg::Delete(1));

    assert_eq!(app.entries.len(), 1, "entry must not be locally removed pre-confirm while impersonating");
}

#[test]
fn entry_timer_start_stop_no_op_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.entries = vec![make_entry(1, 10, 1, 5.0, false)];

    let _ = app.update_entries(EntryMsg::TimerStart(1));
    let _ = app.update_entries(EntryMsg::TimerStop(1));

    assert!(!app.entries[0].is_running);
}

// A lead-initiated Submit can still be in flight (Created not yet delivered)
// when the lead clicks Impersonate. Without a guard on the *response* arm the
// lead's own new entry is pushed into what is now the impersonated member's
// entry list, indistinguishable from the member's own data. Same race class as
// `vacation_entries_created_no_ops_while_impersonating`.
#[test]
fn entry_created_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);

    let _ = app.update_entries(EntryMsg::Created(Ok(make_entry(77, 10, 1, 3.0, false))));

    assert!(app.entries.is_empty(),
        "an in-flight create response must not land in the impersonated member's list");
}

// Symmetric to the above: an in-flight Delete response must not retain() over
// the impersonated member's displayed entries.
#[test]
fn entry_deleted_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.entries = vec![make_entry(1, 10, 1, 5.0, false)];
    app.impersonating = Some(9);

    let _ = app.update_entries(EntryMsg::Deleted(Ok(1)));

    assert_eq!(app.entries.len(), 1,
        "an in-flight delete response must not remove an impersonated member's entry");
}

#[test]
fn entry_fill_remaining_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.entry_form = Some(EntryForm::new());

    let _ = app.update_entries(EntryMsg::FillRemaining);

    assert!(app.entry_form.as_ref().unwrap().hours_input.is_empty());
}

#[test]
fn entry_show_form_works_when_not_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = None;

    let _ = app.update_entries(EntryMsg::ShowForm);

    assert!(app.entry_form.is_some());
}

// ── Task 4: Stats page re-scoping / overtime-adjustment guards ────────────────

#[test]
fn stats_config_resolution_uses_member_when_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.total_weekly_hours = 40.0;
    let mut member = team_member(9, "Sam");
    member.work_percentage = 0.5;
    member.total_holiday_days_per_year = 20;
    app.team_settings.members.push(member);
    app.impersonating = Some(9);

    let m = app.impersonated_member().expect("member must resolve");
    assert_eq!(m.expected_hours_per_day(app.settings.total_weekly_hours), 4.0);

    app.impersonating = None;
    assert!(app.impersonated_member().is_none());
}

#[test]
fn stats_show_adj_form_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);

    let _ = app.update_stats(StatsMsg::ShowAdjForm);

    assert!(app.overtime_adj_form.is_none());
}

#[test]
fn stats_adj_submit_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.overtime_adj_form = Some(OvertimeAdjustmentForm {
        date_input: format!("01.06.{}", app.overtime_year),
        hours_input: "2".into(),
        reason_input: "test".into(),
        error: None,
    });
    let before = app.overtime_adjustments.adjustments_for(app.overtime_year).len();

    let _ = app.update_stats(StatsMsg::AdjSubmit);

    assert_eq!(app.overtime_adjustments.adjustments_for(app.overtime_year).len(), before);
}

#[test]
fn stats_adj_delete_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let year = app.overtime_year;
    let id = app.overtime_adjustments.next_id;
    app.overtime_adjustments.next_id += 1;
    app.overtime_adjustments.adjustments_for_mut(year).push(
        crate::state::overtime_adjustments::OvertimeAdjustment {
            id, date: format!("{year}-06-01"), hours: 2.0, reason: "test".into(),
        },
    );
    app.impersonating = Some(9);

    let _ = app.update_stats(StatsMsg::AdjDelete(id));

    assert_eq!(app.overtime_adjustments.adjustments_for(year).len(), 1);
}

#[test]
fn stats_show_adj_form_works_when_not_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = None;

    let _ = app.update_stats(StatsMsg::ShowAdjForm);

    assert!(app.overtime_adj_form.is_some());
}

/// `StatsMsg::Loaded` derives next-year carryover from the loaded balance and
/// persists it into `self.settings` — but while impersonating, the loaded
/// balance belongs to the impersonated member, not the lead. That
/// persistence must be skipped entirely (though the member's stats must
/// still populate `year_balance`/`holiday_stats` for display).
#[test]
fn stats_loaded_does_not_persist_carryover_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    // test_instance sets overtime_year = 2025 and first_work_day = None; the
    // real wall-clock year is > 2025, so the persistence branch's year/
    // after_employment conditions are satisfied and only the impersonation
    // guard is what should prevent the write.
    app.impersonating = Some(9);
    let r#gen = app.stats_gen;

    let mut balance = zero_balance();
    balance.total_balance = 12345.0; // distinctive value that must never reach lead settings

    let _ = app.update_stats(StatsMsg::Loaded(
        r#gen,
        Ok((balance, zero_holiday_stats(), Vec::new())),
    ));

    assert!(app.year_balance.is_some(), "member's stats must still populate for display");
    assert!(
        !app.settings.carryover.contains_key(&2026),
        "member's balance must never be persisted into the lead's own settings.json"
    );
}

#[test]
fn carryover_delete_removes_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.carryover.insert(2026, crate::state::settings::YearCarryover::default());
    app.team_settings.members.push(m);

    let _ = app.update_team(TeamMsg::CarryoverDelete(3, 2026));

    assert!(!app.team_settings.member(3).unwrap().carryover.contains_key(&2026));
}

/// Correcting a team member's first_work_day to an earlier date must purge
/// stale auto-computed carryover entries left from the old date, otherwise
/// first_missing_carryover_year sees them as "already done" and the chain
/// never recomputes them — silently understating overtime forever.
#[test]
fn first_work_day_save_purges_stale_carryover_on_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    // Stale auto-computed entries from the original (wrong) first_work_day.
    m.carryover.insert(2021, crate::state::settings::YearCarryover {
        overtime_hours: 5.0, ..Default::default()
    });
    // A manually entered value — must survive the correction.
    m.carryover.insert(2022, crate::state::settings::YearCarryover {
        overtime_hours: 99.0, is_user_defined: true, ..Default::default()
    });
    app.team_settings.members.push(m);

    app.team.first_work_day_inputs.insert(3, "01.01.2018".into());
    let _ = app.update_team(TeamMsg::FirstWorkDaySave(3));

    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.first_work_day, Some(chrono::NaiveDate::from_ymd_opt(2018, 1, 1).unwrap()));
    assert!(
        !member.carryover.contains_key(&2021),
        "stale auto-computed entry from the old first_work_day must be purged"
    );
    assert!(
        member.carryover.contains_key(&2022) && member.carryover[&2022].overtime_hours == 99.0,
        "user-defined entry must survive the correction"
    );
}

#[test]
fn first_work_day_detected_purges_stale_carryover_on_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    // Stale auto-computed entries from the original (wrong) first_work_day.
    m.carryover.insert(2021, crate::state::settings::YearCarryover {
        overtime_hours: 5.0, ..Default::default()
    });
    // A manually entered value — must survive the correction.
    m.carryover.insert(2022, crate::state::settings::YearCarryover {
        overtime_hours: 99.0, is_user_defined: true, ..Default::default()
    });
    app.team_settings.members.push(m);

    let _ = app.update_team(TeamMsg::FirstWorkDayDetected(
        3,
        Ok(Some(chrono::NaiveDate::from_ymd_opt(2018, 1, 1).unwrap())),
    ));

    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.first_work_day, Some(chrono::NaiveDate::from_ymd_opt(2018, 1, 1).unwrap()));
    assert!(
        !member.carryover.contains_key(&2021),
        "stale auto-computed entry from the old first_work_day must be purged"
    );
    assert!(
        member.carryover.contains_key(&2022) && member.carryover[&2022].overtime_hours == 99.0,
        "user-defined entry must survive the correction"
    );
}

#[test]
fn carryover_sync_loaded_preserves_user_defined_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    m.carryover.insert(2026, crate::state::settings::YearCarryover {
        holiday_hours: 12.0, overtime_hours: -2.0, is_user_defined: true, ..Default::default()
    });
    app.team_settings.members.push(m);

    let _ = app.update_team(TeamMsg::CarryoverSyncLoaded(
        3, 0, 2025,
        Ok((crate::stats::YearBalance {
            period: crate::stats::PeriodStats {
                total_hours: 0.0,
                expected_hours: 0.0,
                balance_hours: 0.0,
                working_days_expected: 0,
                days_with_entries: 0,
            },
            carryover_hours: 0.0, manual_adjustments_hours: 0.0, total_balance: 999.0,
        }, crate::stats::HolidayStats { days_taken: 0.0, days_remaining: 0.0, total_days: 0.0 })),
    ));

    let entry = &app.team_settings.member(3).unwrap().carryover[&2026];
    assert_eq!(entry.holiday_hours, 12.0, "user-defined entry must not be overwritten");
    assert_eq!(entry.overtime_hours, -2.0);
}

/// Carryover must chain sequentially across *multiple* years, not just one:
/// a member with several years of history and no prior carryover entries
/// should have each year filled in, in order, as each background fetch
/// completes — driven by `CarryoverSyncLoaded` re-invoking `CarryoverSyncStart`
/// after every insert (`src/app/team.rs`).
#[test]
fn carryover_sync_chains_across_multiple_years() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    app.team_settings.members.push(m);

    let current_year = 2026;
    let balance_for = |total_balance: f64| crate::stats::YearBalance {
        period: crate::stats::PeriodStats {
            total_hours: 0.0,
            expected_hours: 0.0,
            balance_hours: 0.0,
            working_days_expected: 0,
            days_with_entries: 0,
        },
        carryover_hours: 0.0,
        manual_adjustments_hours: 0.0,
        total_balance,
    };
    let holidays_for = |days_remaining: f64| crate::stats::HolidayStats {
        days_taken: 0.0,
        days_remaining,
        total_days: 25.0,
    };

    // Before any sync, the whole 2023..2026 range is a gap; 2023 is earliest.
    let member = app.team_settings.member(3).unwrap();
    assert_eq!(
        crate::stats::first_missing_carryover_year(&member.carryover, 2023, current_year),
        Some(2023)
    );

    // Step 1: 2023's fetch completes → fills carryover[2024].
    let _ = app.update_team(TeamMsg::CarryoverSyncLoaded(
        3, 0, 2023,
        Ok((balance_for(10.0), holidays_for(3.0))),
    ));
    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.carryover[&2024].overtime_hours, 10.0);
    assert_eq!(
        crate::stats::first_missing_carryover_year(&member.carryover, 2023, current_year),
        Some(2024),
        "chain must advance to the next gap after 2024 is filled"
    );

    // Step 2: 2024's fetch completes → fills carryover[2025].
    let _ = app.update_team(TeamMsg::CarryoverSyncLoaded(
        3, 0, 2024,
        Ok((balance_for(15.0), holidays_for(2.0))),
    ));
    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.carryover[&2025].overtime_hours, 15.0);
    assert_eq!(
        crate::stats::first_missing_carryover_year(&member.carryover, 2023, current_year),
        Some(2025),
        "chain must advance to the next gap after 2025 is filled"
    );

    // Step 3: 2025's fetch completes → fills carryover[2026], catching up to current_year.
    let _ = app.update_team(TeamMsg::CarryoverSyncLoaded(
        3, 0, 2025,
        Ok((balance_for(-5.0), holidays_for(0.0))),
    ));
    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.carryover[&2026].overtime_hours, -5.0);
    assert_eq!(
        crate::stats::first_missing_carryover_year(&member.carryover, 2023, current_year),
        None,
        "member must be fully caught up after all three years chain through"
    );

    // Every year's distinct value survived independently — no overwriting
    // across the chain.
    let member = app.team_settings.member(3).unwrap();
    assert_eq!(member.carryover[&2024].overtime_hours, 10.0);
    assert_eq!(member.carryover[&2025].overtime_hours, 15.0);
    assert_eq!(member.carryover[&2026].overtime_hours, -5.0);
}

/// A `CarryoverSyncLoaded` response from a chain that was superseded by a
/// newer purge (e.g. a second first-work-day edit before the first chain
/// finished) must be discarded, not reinserted — otherwise the stale value
/// sits at a year `first_missing_carryover_year` will never revisit.
#[test]
fn carryover_sync_loaded_with_stale_gen_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    app.team_settings.members.push(m);

    // A newer chain has already started (e.g. via FirstWorkDaySave), bumping
    // the gen past what this in-flight response was launched under.
    app.team.carryover_sync_gen.insert(3, 1);

    let balance = crate::stats::YearBalance {
        period: crate::stats::PeriodStats {
            total_hours: 0.0,
            expected_hours: 0.0,
            balance_hours: 0.0,
            working_days_expected: 0,
            days_with_entries: 0,
        },
        carryover_hours: 0.0,
        manual_adjustments_hours: 0.0,
        total_balance: 999.0,
    };
    let holidays =
        crate::stats::HolidayStats { days_taken: 0.0, days_remaining: 0.0, total_days: 0.0 };

    let _ = app.update_team(TeamMsg::CarryoverSyncLoaded(3, 0, 2023, Ok((balance, holidays))));

    let member = app.team_settings.member(3).unwrap();
    assert!(
        !member.carryover.contains_key(&2024),
        "a response launched under a superseded gen must not write any carryover"
    );
}

/// A lead correcting their own weekly hours invalidates every team member's
/// stored carryover (computed against the lead's baseline) and must purge
/// and re-sync the whole roster.
#[test]
fn save_profile_weekly_hours_change_resets_team_carryover() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.team_lead_mode = true;
    app.settings.total_weekly_hours = 40.0;
    app.settings.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());

    let mut m = team_member(3, "Carry");
    m.first_work_day = Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    m.carryover.insert(2025, crate::state::settings::YearCarryover::default());
    m.carryover.insert(2026, crate::state::settings::YearCarryover {
        overtime_hours: 12.0,
        ..Default::default()
    });
    app.team_settings.members.push(m);
    let gen_before = *app.team.carryover_sync_gen.entry(3).or_default();

    app.settings_form.weekly_hours_input = "42".into();
    app.settings_form.percentage_input = "100".into();
    app.settings_form.holidays_input = "25".into();
    app.settings_form.first_work_day_input = "01.01.2025".into();

    let _ = app.update_settings(crate::app::SettingsMsg::SaveProfile);

    assert_eq!(app.settings.total_weekly_hours, 42.0);
    let member = app.team_settings.member(3).unwrap();
    assert!(
        !member.carryover.contains_key(&2026),
        "auto-computed carryover must be purged when the lead's weekly hours change"
    );
    assert!(
        *app.team.carryover_sync_gen.get(&3).unwrap() > gen_before,
        "the per-member sync epoch must be bumped so any in-flight stale response is discarded"
    );
}

#[test]
fn adj_submit_rejects_invalid_form_without_mutating_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Adj"));
    app.team.adjustment_forms.insert(9, crate::app::OvertimeAdjustmentForm {
        date_input: "not-a-date".into(),
        hours_input: "4".into(),
        reason_input: "test".into(),
        error: None,
    });

    let _ = app.update_team(TeamMsg::AdjSubmit(9));

    assert!(app.team.adjustment_forms.get(&9).unwrap().error.is_some());
    assert!(app.team_settings.member(9).unwrap().overtime_adjustments.adjustments_for(chrono::Local::now().naive_local().date().year()).is_empty());
}

#[test]
fn adj_delete_removes_entry_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(9, "Adj");
    let year = chrono::Local::now().naive_local().date().year();
    m.overtime_adjustments.adjustments_for_mut(year).push(
        crate::state::overtime_adjustments::OvertimeAdjustment {
            id: 1, date: format!("{year}-03-01"), hours: 2.0, reason: "Bonus".into(),
        }
    );
    app.team_settings.members.push(m);

    let _ = app.update_team(TeamMsg::AdjDelete(9, 1));

    assert!(app.team_settings.member(9).unwrap().overtime_adjustments.adjustments_for(year).is_empty());
}

/// If the team-settings save fails, `AdjSubmit` must roll back the in-memory
/// push so the lead doesn't see an adjustment applied that wasn't persisted,
/// and must leave the form open (not silently dismissed) so they can retry.
#[test]
fn adj_submit_rolls_back_on_save_failure() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.data_dir = blocked;
    app.team_settings.members.push(team_member(9, "Adj"));
    let year = chrono::Local::now().naive_local().date().year();
    app.team.adjustment_forms.insert(9, crate::app::OvertimeAdjustmentForm {
        date_input: format!("01.03.{year}"),
        hours_input: "4".into(),
        reason_input: "test".into(),
        error: None,
    });

    let _ = app.update_team(TeamMsg::AdjSubmit(9));

    assert!(
        app.team_settings.member(9).unwrap().overtime_adjustments.adjustments_for(year).is_empty(),
        "the pushed adjustment must be rolled back when the save fails"
    );
    assert!(
        app.team.adjustment_forms.contains_key(&9),
        "the form must stay open on a failed save instead of silently closing"
    );
    assert!(app.error_banner.is_some());
}

/// Same rollback guarantee for `AdjDelete`: a failed save must restore the
/// removed adjustment rather than leave it silently gone from disk-truth.
#[test]
fn adj_delete_rolls_back_on_save_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut m = team_member(9, "Adj");
    let year = chrono::Local::now().naive_local().date().year();
    m.overtime_adjustments.adjustments_for_mut(year).push(
        crate::state::overtime_adjustments::OvertimeAdjustment {
            id: 1, date: format!("{year}-03-01"), hours: 2.0, reason: "Bonus".into(),
        }
    );
    app.team_settings.members.push(m);
    // Save the initial state successfully first, then block the data dir so
    // only the delete's save fails.
    let _ = app.team_settings.save(&app.settings.data_dir);
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    app.settings.data_dir = blocked;

    let _ = app.update_team(TeamMsg::AdjDelete(9, 1));

    assert_eq!(
        app.team_settings.member(9).unwrap().overtime_adjustments.adjustments_for(year).len(),
        1,
        "the removed adjustment must be restored when the save fails"
    );
    assert!(app.error_banner.is_some());
}

// ── Task 6: Page::Team / Message::Team routing, dispatch, startup sync ───────

#[test]
fn dispatch_page_load_team_does_not_panic_without_client() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(1, "X"));
    // client is None in test_instance, so TeamMsg::Refresh must no-op entirely
    // rather than marking members as loading — otherwise the cards would be
    // stuck on "Loading…" forever with no fetch task ever able to clear the
    // flag. That no-op (and not panicking) is the observable effect we pin here.
    let _ = app.dispatch_page_load(&Page::Team);
    assert!(!app.team.stats.get(&1).map(|s| s.loading).unwrap_or(false));
}

#[test]
fn start_all_team_carryover_syncs_noop_when_team_lead_mode_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.team_lead_mode = false;
    app.team_settings.members.push(team_member(1, "X"));
    // Should not panic; with client: None every branch resolves to Task::none()
    // regardless, but this also documents that the gate exists.
    let _ = app.start_all_team_carryover_syncs();
}

// ── Company user directory (roster picker) ────────────────────────────────

fn dummy_directory_user(id: i64, first: &str, last: &str) -> crate::harvest::models::User {
    crate::harvest::models::User {
        id,
        first_name: first.into(),
        last_name: last.into(),
        email: format!("{first}.{last}@example.com").to_lowercase(),
        weekly_capacity: Some(144000),
        is_admin: false,
        is_active: true,
    }
}

#[test]
fn directory_ensure_loaded_noops_when_not_admin() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.harvest_user_is_admin = false;
    let _ = app.update_team(TeamMsg::DirectoryEnsureLoaded);
    assert!(app.team.directory.is_empty());
    assert!(!app.team.directory_loading);
}

#[test]
fn directory_ensure_loaded_reads_valid_cache_without_a_client() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.harvest_user_is_admin = true;
    let users = vec![dummy_directory_user(1, "Ada", "Lovelace")];
    crate::state::cache::UserDirectoryCache::new(users)
        .save(dir.path())
        .expect("cache save");

    // client is None in test_instance — a cache hit must serve without one,
    // proving DirectoryEnsureLoaded checks the cache before the client guard
    // would otherwise make it a no-op.
    let _ = app.update_team(TeamMsg::DirectoryEnsureLoaded);

    assert_eq!(app.team.directory.len(), 1);
    assert_eq!(app.team.directory[0].email, "ada.lovelace@example.com");
    assert!(!app.team.directory_loading);
}

#[test]
fn directory_ensure_loaded_skips_refetch_once_already_populated() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.harvest_user_is_admin = true;
    app.team.directory = vec![dummy_directory_user(1, "Ada", "Lovelace")];

    // No client and no cache on disk — if this tried to fetch or reload,
    // it would either panic or clear the directory. Neither happens.
    let _ = app.update_team(TeamMsg::DirectoryEnsureLoaded);

    assert_eq!(app.team.directory.len(), 1);
}

#[test]
fn directory_loaded_err_sets_error_and_clears_loading() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.directory_loading = true;

    let _ = app.update_team(TeamMsg::DirectoryLoaded(Err("boom".into())));

    assert!(!app.team.directory_loading);
    assert_eq!(app.team.directory_error.as_deref(), Some("boom"));
}

#[test]
fn directory_pick_fills_add_form_and_clears_query() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.directory = vec![dummy_directory_user(42, "Grace", "Hopper")];
    app.team.directory_query = "gra".into();

    let _ = app.update_team(TeamMsg::DirectoryPick(42));

    assert_eq!(app.team.add_form.name_input, "Grace Hopper");
    assert_eq!(app.team.add_form.id_input, "42");
    assert!(app.team.directory_query.is_empty());
}

#[test]
fn directory_pick_unknown_id_noops_add_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team.directory_query = "gra".into();

    let _ = app.update_team(TeamMsg::DirectoryPick(999));

    assert!(app.team.add_form.name_input.is_empty());
    assert!(app.team.add_form.id_input.is_empty());
    // Query still clears — an unmatched pick shouldn't leave stale search text.
    assert!(app.team.directory_query.is_empty());
}

#[test]
fn vacation_show_form_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);

    let _ = app.update_vacation(VacationMsg::ShowForm);

    assert!(app.vacation.form.is_none());
}

#[test]
fn vacation_form_submit_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    let mut form = VacationForm::new();
    form.from_input = "01.06.2025".into();
    form.selected_task_id = Some(1);
    app.vacation.form = Some(form);

    let _ = app.update_vacation(VacationMsg::FormSubmit);

    assert!(!app.vacation.form.as_ref().unwrap().submitting);
    // Without the guard, FormSubmit would still run (assignments are empty in
    // test_instance, so it fails to resolve a project id and sets `error`
    // rather than `submitting`) — assert `error` stays None too so this test
    // actually discriminates on the guard rather than passing vacuously.
    assert!(app.vacation.form.as_ref().unwrap().error.is_none());
}

#[test]
fn vacation_delete_entry_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.vacation.entries = vec![make_entry(1, 1, 1, 8.0, false)];
    app.impersonating = Some(9);

    let _ = app.update_vacation(VacationMsg::DeleteEntry(1));

    // Pins the invariant that DeleteEntry never removes locally — it only
    // fires the request; removal happens in EntryDeleted. Note this cannot
    // discriminate on the impersonation guard itself (DeleteEntry touches no
    // local state either way, and test_instance has no client); the guard is
    // covered by `vacation_entry_deleted_no_ops_while_impersonating` below.
    assert_eq!(app.vacation.entries.len(), 1,
        "DeleteEntry must not optimistically remove the entry locally");
}

#[test]
fn vacation_show_form_works_when_not_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = None;

    let _ = app.update_vacation(VacationMsg::ShowForm);

    assert!(app.vacation.form.is_some());
}

// A lead-initiated FormSubmit can still be in flight (EntriesCreated not yet
// delivered) when the lead starts impersonating a team member. Without a
// guard here, the lead's own newly-created entries would land in the
// impersonated member's displayed vacation.entries/summary once the response
// arrives -- the same "wrong person's data" shape as Task 4's Bug 2, just
// race-gated instead of unconditional.
#[test]
fn vacation_entries_created_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    app.vacation.year = 2025;

    let entry = make_entry(1, 1, 1, 8.0, false);
    let _ = app.update_vacation(VacationMsg::EntriesCreated(Ok(vec![entry])));

    assert!(app.vacation.entries.is_empty());
}

// Symmetric to vacation_entries_created_no_ops_while_impersonating: a
// lead-initiated DeleteEntry can still be in flight (EntryDeleted not yet
// delivered) when the lead starts impersonating a team member. Without a
// guard here, the response would run retain()/recompute_vacation_summary()
// against the impersonated member's displayed vacation.entries.
#[test]
fn vacation_entry_deleted_no_ops_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.vacation.year = 2025;
    let entry = make_entry(1, 1, 1, 8.0, false);
    app.vacation.entries = vec![entry];
    app.impersonating = Some(9);

    let _ = app.update_vacation(VacationMsg::EntryDeleted(Ok(1)));

    assert_eq!(app.vacation.entries.len(), 1);
}

#[test]
fn settings_save_profile_unaffected_by_impersonation() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);
    app.settings_form.weekly_hours_input = "42".into();
    app.settings_form.percentage_input = "100".into();
    app.settings_form.holidays_input = "25".into();
    app.settings_form.first_work_day_input = String::new();

    let _ = app.update_settings(SettingsMsg::SaveProfile);

    assert_eq!(app.settings.total_weekly_hours, 42.0, "Settings must remain editable while impersonating");
}

#[test]
fn team_add_member_unaffected_by_impersonation() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.team_settings.members.push(team_member(9, "Sam"));
    app.impersonating = Some(9);
    app.team.add_form.name_input = "Jamie".into();
    app.team.add_form.id_input = "42".into();

    let _ = app.update_team(TeamMsg::AddMember);

    assert!(app.team_settings.member(42).is_some(), "Team tab must remain editable while impersonating");
}

#[test]
fn nav_date_next_still_works_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    let before = app.current_date;

    let _ = app.update(Message::Nav(NavMsg::DateNext));

    assert_eq!(app.current_date, before + chrono::Duration::days(1));
}

#[test]
fn stats_year_next_still_works_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    let before = app.overtime_year;

    let _ = app.update_stats(StatsMsg::YearNext);

    assert_eq!(app.overtime_year, before + 1);
}

#[test]
fn vacation_year_next_still_works_while_impersonating() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.impersonating = Some(9);
    let before = app.vacation.year;

    let _ = app.update_vacation(VacationMsg::YearNext);

    assert_eq!(app.vacation.year, before + 1);
}

// ── Unsubmitted-weeks warning ──────────────────────────────────────────────

use crate::unsubmitted::UnsubmittedWeek;
use super::unsubmitted_warning::CHECK_INTERVAL;

fn one_week() -> Vec<UnsubmittedWeek> {
    vec![UnsubmittedWeek {
        monday: NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
        through: NaiveDate::from_ymd_opt(2026, 7, 12).unwrap(),
        hours: 8.0,
    }]
}

#[test]
fn loaded_weeks_show_banner() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    assert!(!app.show_unsubmitted_banner());

    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Ok(one_week())));

    assert_eq!(app.unsubmitted_weeks, one_week());
    assert!(app.show_unsubmitted_banner());
}

#[test]
fn dismiss_survives_later_results() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Ok(one_week())));

    let _ = app.update_unsubmitted(UnsubmittedMsg::Dismiss);
    assert!(!app.show_unsubmitted_banner());

    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Ok(one_week())));
    assert!(!app.show_unsubmitted_banner(), "dismiss lasts until restart");
}

#[test]
fn failed_check_keeps_previous_result_and_error_banner() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Ok(one_week())));

    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Err("boom".into())));

    assert_eq!(app.unsubmitted_weeks, one_week());
    assert_eq!(app.error_banner, None);
}

#[test]
fn check_throttle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    let now = std::time::Instant::now();

    assert!(app.should_check_unsubmitted(false, now), "never checked → check");
    app.unsubmitted_checked_at = Some(now);
    assert!(!app.should_check_unsubmitted(false, now), "just checked → skip");
    assert!(app.should_check_unsubmitted(true, now), "force bypasses throttle");
    assert!(app.should_check_unsubmitted(false, now + CHECK_INTERVAL));
}

#[test]
fn company_address_fills_empty_setting_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    let _ = app.update_unsubmitted(UnsubmittedMsg::CompanyLoaded(Ok("https://acme.harvestapp.com/".into())));
    assert_eq!(app.settings.harvest_web_address.as_deref(), Some("https://acme.harvestapp.com"));
    assert_eq!(app.settings_form.web_address_input, "https://acme.harvestapp.com");

    let _ = app.update_unsubmitted(UnsubmittedMsg::CompanyLoaded(Ok("https://other.harvestapp.com".into())));
    assert_eq!(app.settings.harvest_web_address.as_deref(), Some("https://acme.harvestapp.com"));
}

#[test]
fn save_web_address_normalizes_and_clears() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());

    let _ = app.update_settings(SettingsMsg::WebAddressChanged(" acme.harvestapp.com/ ".into()));
    let _ = app.update_settings(SettingsMsg::SaveWebAddress);
    assert_eq!(app.settings.harvest_web_address.as_deref(), Some("https://acme.harvestapp.com"));
    assert_eq!(app.settings_form.web_address_input, "https://acme.harvestapp.com");

    let _ = app.update_settings(SettingsMsg::WebAddressChanged(String::new()));
    let _ = app.update_settings(SettingsMsg::SaveWebAddress);
    assert_eq!(app.settings.harvest_web_address, None);
}

#[test]
fn reset_unsubmitted_forgets_previous_account() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.unsubmitted_weeks = one_week();
    app.unsubmitted_checked_at = Some(std::time::Instant::now());
    app.unsubmitted_dismissed = true;
    app.settings.harvest_web_address = Some("https://acme.harvestapp.com".into());
    app.settings_form.web_address_input = "https://acme.harvestapp.com".into();

    app.reset_unsubmitted();

    assert!(app.unsubmitted_weeks.is_empty());
    assert!(app.unsubmitted_checked_at.is_none());
    assert!(!app.unsubmitted_dismissed);
    assert_eq!(app.settings.harvest_web_address, None);
    assert!(app.settings_form.web_address_input.is_empty());
}

fn timer_test_app(dir: &std::path::Path) -> EasyHarvest {
    let mut app = EasyHarvest::test_instance(dir);
    app.current_date = chrono::Local::now().naive_local().date();
    app.cached_project_options = vec![crate::state::favorites::ProjectOption {
        project_id: 10,
        task_id: 1,
        client_name: "C".into(),
        project_name: "P".into(),
        task_name: "T".into(),
        is_pinned: false,
        use_count: 0,
        search_text: "C > P — T".into(),
        search_text_lower: "c > p — t".into(),
    }];
    let mut form = EntryForm::new();
    form.selected_project_key = Some((10, 1));
    app.entry_form = Some(form);
    app
}

#[test]
fn start_timer_needs_no_hours() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = timer_test_app(dir.path());
    let _ = app.update_entries(EntryMsg::Submit);
    assert_eq!(
        app.entry_form.as_ref().unwrap().error.as_deref(),
        Some("Enter a valid number of hours"),
        "control: a plain save still requires hours",
    );

    let mut app = timer_test_app(dir.path());
    let _ = app.update_entries(EntryMsg::StartTimer);
    assert_eq!(app.entry_form.as_ref().unwrap().error, None);
}

#[test]
fn start_timer_refused_on_other_days() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = timer_test_app(dir.path());
    app.current_date -= chrono::Duration::days(1);
    let _ = app.update_entries(EntryMsg::StartTimer);
    assert_eq!(
        app.entry_form.as_ref().unwrap().error.as_deref(),
        Some("A timer can only be started today"),
    );
}

#[test]
fn created_running_entry_stops_other_timers() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    let mut old = make_entry(1, 10, 1, 2.0, false);
    old.is_running = true;
    app.entries = vec![old];
    let mut new = make_entry(2, 10, 1, 0.0, false);
    new.is_running = true;
    new.spent_date = app.current_date.format("%Y-%m-%d").to_string();

    let _ = app.update_entries(EntryMsg::Created(Ok(new)));

    assert!(!app.entries[0].is_running, "Harvest runs one timer at a time");
    assert!(app.entries[1].is_running);
}

#[test]
fn entry_hours_keeps_typed_time_when_starting_a_timer() {
    use super::entries::entry_hours;
    assert_eq!(entry_hours("", true), Ok(None), "blank: timer starts at zero");
    assert_eq!(entry_hours("2:30", true), Ok(Some(2.5)), "typed: timer continues from it");
    assert!(entry_hours("abc", true).is_err());
    assert_eq!(entry_hours("1.5", false), Ok(Some(1.5)));
    assert_eq!(entry_hours("", false), Err("Enter a valid number of hours"));
}

#[test]
fn submit_ignored_while_request_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = timer_test_app(dir.path());
    app.entry_form.as_mut().unwrap().submitting = true;
    let _ = app.update_entries(EntryMsg::Submit);
    assert_eq!(app.entry_form.as_ref().unwrap().error, None, "second press must not re-validate or re-send");
}

#[test]
fn created_entry_for_another_day_is_not_listed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = EasyHarvest::test_instance(dir.path());
    app.entry_form = Some(EntryForm::new());
    let mut other_day = make_entry(2, 10, 1, 1.0, false);
    other_day.spent_date = "2025-01-14".into(); // test_instance shows 2025-01-15

    let _ = app.update_entries(EntryMsg::Created(Ok(other_day)));

    assert!(app.entries.is_empty(), "user navigated away while the create was in flight");
    assert!(app.entry_form.is_none());
}

#[test]
fn open_in_harvest_without_address_goes_to_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings.harvest_web_address = None;
    let _ = app.update_unsubmitted(UnsubmittedMsg::Loaded(Ok(one_week())));

    // Returns a navigation task rather than silently doing nothing.
    let task = app.update_unsubmitted(UnsubmittedMsg::OpenInHarvest);
    assert!(task.units() > 0);
}

#[test]
fn wizard_profile_continue_saves_web_address() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = EasyHarvest::test_instance(dir.path());
    app.settings_form.weekly_hours_input = "41".into();
    app.settings_form.percentage_input = "100".into();
    app.settings_form.holidays_input = "25".into();
    app.settings_form.first_work_day_input = String::new();
    app.settings_form.web_address_input = "acme.harvestapp.com/".into();

    let _ = app.update_settings(SettingsMsg::WizardProfileContinue);

    assert_eq!(app.settings.harvest_web_address.as_deref(), Some("https://acme.harvestapp.com"));
}
