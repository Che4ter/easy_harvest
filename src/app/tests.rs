use super::*;
use super::tasks::{build_vacation_entries, compute_budget_summaries};
use crate::harvest::models::{ClientRef, ProjectRef, TaskRef, TimeEntry, UserRef};

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
