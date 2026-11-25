//! Pure logic for manager budget cards: which projects count, and how the
//! spent hours split per person for the stacked bar.

use std::collections::{HashMap, HashSet};

use crate::harvest::models::{ProjectAssignment, ProjectBudgetReport, TimeEntry};

/// A Harvest hour budget on a project the current user manages.
#[derive(Debug, Clone, PartialEq)]
pub struct ManagedBudget {
    pub project_id: i64,
    pub project_name: String,
    pub client_name: String,
    pub budget_hours: f64,
    pub spent_hours: f64,
    pub remaining_hours: f64,
    pub is_monthly: bool,
    pub budget_by: String,
}

/// One bar segment; `user_id == None` is the "Others" bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub user_id: Option<i64>,
    pub label: String,
    pub hours: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonSplit {
    pub segments: Vec<Segment>,
    /// True when the raw split differs from the target total by more than 5 %.
    pub approximate: bool,
}

/// Hour budgets (`budget_by` project/task/person with a non-null budget) of
/// projects where the user is an active project manager.
pub fn managed_hour_budgets(
    assignments: &[ProjectAssignment],
    reports: &[ProjectBudgetReport],
) -> Vec<ManagedBudget> {
    reports
        .iter()
        .filter_map(|r| {
            let budget = r.budget?;
            if !matches!(r.budget_by.as_str(), "project" | "task" | "person") {
                return None;
            }
            let assignment = assignments
                .iter()
                .find(|a| a.is_active && a.is_project_manager && a.project.id == r.project_id)?;
            Some(ManagedBudget {
                project_id: r.project_id,
                project_name: r.project_name.clone(),
                client_name: r
                    .client_name
                    .clone()
                    .unwrap_or_else(|| assignment.client.name.clone()),
                budget_hours: budget,
                spent_hours: r.budget_spent,
                remaining_hours: r.budget_remaining.unwrap_or(budget - r.budget_spent),
                is_monthly: r.budget_is_monthly,
                budget_by: r.budget_by.clone(),
            })
        })
        .collect()
}

/// Splits entry hours per person: top `top_n` named (the current user is
/// always named if they have hours), the rest summed into "Others". With
/// `target_total`, segments are scaled to sum to it.
pub fn compute_person_split(
    entries: &[TimeEntry],
    counted_task_ids: Option<&HashSet<i64>>,
    target_total: Option<f64>,
    current_user_id: Option<i64>,
    top_n: usize,
) -> PersonSplit {
    let mut per_user: HashMap<i64, (Option<String>, f64)> = HashMap::new();
    for e in entries {
        if counted_task_ids.is_some_and(|ids| !ids.contains(&e.task.id)) {
            continue;
        }
        let slot = per_user.entry(e.user.id).or_insert((None, 0.0));
        if slot.0.is_none() {
            slot.0 = e.user.name.clone();
        }
        slot.1 += e.hours;
    }

    let mut users: Vec<(i64, Option<String>, f64)> =
        per_user.into_iter().map(|(id, (n, h))| (id, n, h)).collect();
    let by_hours = |a: &(i64, Option<String>, f64), b: &(i64, Option<String>, f64)| {
        b.2.total_cmp(&a.2).then(a.0.cmp(&b.0))
    };
    users.sort_by(by_hours);

    let raw: f64 = users.iter().map(|u| u.2).sum();
    let mut others = users.split_off(top_n.min(users.len()));
    if let Some(me) = current_user_id
        && top_n > 0
        && !users.iter().any(|u| u.0 == me)
        && let Some(pos) = others.iter().position(|u| u.0 == me)
    {
        let mine = others.remove(pos);
        let displaced = users.pop().expect("top_n > 0 and others non-empty");
        others.push(displaced);
        users.push(mine);
        users.sort_by(by_hours);
    }

    let mut segments: Vec<Segment> = users
        .into_iter()
        .map(|(id, name, hours)| Segment {
            user_id: Some(id),
            label: if Some(id) == current_user_id {
                "You".to_string()
            } else {
                name.unwrap_or_else(|| format!("User {id}"))
            },
            hours,
        })
        .collect();
    let others_hours: f64 = others.iter().map(|u| u.2).sum();
    if others_hours > 0.0 {
        segments.push(Segment { user_id: None, label: "Others".to_string(), hours: others_hours });
    }

    let mut approximate = false;
    if let Some(t) = target_total
        && raw > 0.0
    {
        let factor = t / raw;
        for s in &mut segments {
            s.hours *= factor;
        }
        approximate = t > 0.0 && (raw - t).abs() / t > 0.05;
    }
    PersonSplit { segments, approximate }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harvest::models::{ClientRef, ProjectRef, TaskRef, UserRef};

    fn assignment(project_id: i64, manager: bool) -> ProjectAssignment {
        ProjectAssignment {
            id: project_id,
            project: ProjectRef { id: project_id, name: format!("P{project_id}"), code: None },
            client: ClientRef { id: 1, name: "Fallback".into() },
            is_active: true,
            is_project_manager: manager,
            task_assignments: vec![],
        }
    }

    fn report(project_id: i64, budget_by: &str, budget: Option<f64>) -> ProjectBudgetReport {
        ProjectBudgetReport {
            project_id,
            project_name: format!("P{project_id}"),
            client_name: Some("Client".into()),
            budget_is_monthly: false,
            budget_by: budget_by.into(),
            is_active: true,
            budget,
            budget_spent: 30.0,
            budget_remaining: budget.map(|b| b - 30.0),
        }
    }

    fn entry(user_id: i64, task_id: i64, hours: f64) -> TimeEntry {
        TimeEntry {
            id: 1,
            spent_date: "2025-06-01".into(),
            hours,
            hours_without_timer: None,
            rounded_hours: None,
            notes: None,
            is_locked: false,
            is_running: false,
            is_billed: false,
            approval_status: None,
            billable: false,
            timer_started_at: None,
            project: ProjectRef { id: 1, name: "P".into(), code: None },
            task: TaskRef { id: task_id, name: "T".into() },
            client: ClientRef { id: 1, name: "C".into() },
            user: UserRef { id: user_id, name: Some(format!("U{user_id}")) },
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn six_users() -> Vec<TimeEntry> {
        [10.0, 9.0, 8.0, 7.0, 6.0, 5.0]
            .iter()
            .enumerate()
            .map(|(i, h)| entry(i as i64 + 1, 1, *h))
            .collect()
    }

    #[test]
    fn filters_to_managed_hour_budgets() {
        let assignments = vec![assignment(1, true), assignment(2, true), assignment(3, false)];
        let reports = vec![
            report(1, "project", Some(100.0)),
            report(2, "project_cost", Some(100.0)),
            report(3, "project", Some(100.0)),
        ];
        let out = managed_hour_budgets(&assignments, &reports);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].project_id, 1);
        assert_eq!(out[0].remaining_hours, 70.0);
        assert_eq!(out[0].client_name, "Client");
    }

    #[test]
    fn skips_null_budget() {
        let out = managed_hour_budgets(&[assignment(1, true)], &[report(1, "project", None)]);
        assert!(out.is_empty());
    }

    #[test]
    fn split_top_n_and_others() {
        let split = compute_person_split(&six_users(), None, None, None, 4);
        assert_eq!(split.segments.len(), 5);
        let last = split.segments.last().unwrap();
        assert_eq!(last.user_id, None);
        assert_eq!(last.hours, 11.0);
    }

    #[test]
    fn split_current_user_always_named() {
        let split = compute_person_split(&six_users(), None, None, Some(6), 4);
        assert_eq!(split.segments.len(), 5);
        assert!(split.segments.iter().any(|s| s.user_id == Some(6) && s.label == "You"));
        let others = split.segments.last().unwrap();
        assert_eq!(others.user_id, None);
        assert_eq!(others.hours, 7.0 + 6.0); // displaced user 4 plus user 5
    }

    #[test]
    fn split_scales_to_target() {
        let entries = vec![entry(1, 1, 60.0), entry(2, 1, 40.0)];
        let s = compute_person_split(&entries, None, Some(110.0), None, 4);
        assert!((s.segments.iter().map(|x| x.hours).sum::<f64>() - 110.0).abs() < 1e-9);
        assert!(s.approximate);
        let s = compute_person_split(&entries, None, Some(103.0), None, 4);
        assert!(!s.approximate);
    }

    #[test]
    fn split_empty_no_nan() {
        let s = compute_person_split(&[], None, Some(0.0), None, 4);
        assert!(s.segments.is_empty());
        assert!(!s.approximate);
    }

    #[test]
    fn split_filters_tasks() {
        let entries = vec![entry(1, 1, 5.0), entry(1, 2, 7.0)];
        let ids: HashSet<i64> = [1].into_iter().collect();
        let s = compute_person_split(&entries, Some(&ids), None, None, 4);
        assert_eq!(s.segments.len(), 1);
        assert_eq!(s.segments[0].hours, 5.0);
    }
}
