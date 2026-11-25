use serde::{Deserialize, Serialize};

// --- Refs (embedded in time entries) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: i64,
    pub name: Option<String>,
}

// --- Time Entries ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: i64,
    pub spent_date: String,
    pub hours: f64,
    pub hours_without_timer: Option<f64>,
    pub rounded_hours: Option<f64>,
    pub notes: Option<String>,
    pub is_locked: bool,
    pub is_running: bool,
    pub is_billed: bool,
    /// "unsubmitted" | "submitted" | "approved" — None on accounts without approval workflows.
    #[serde(default)]
    pub approval_status: Option<String>,
    /// Whether this entry counts as billable (project/task is set to billable in Harvest).
    /// Defaults to false for old API responses that pre-date this field.
    #[serde(default)]
    pub billable: bool,
    pub timer_started_at: Option<String>,
    pub project: ProjectRef,
    pub task: TaskRef,
    pub client: ClientRef,
    pub user: UserRef,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TimeEntriesResponse {
    pub time_entries: Vec<TimeEntry>,
    pub total_pages: i64,
}

#[derive(Debug, Serialize)]
pub struct CreateTimeEntry {
    pub project_id: i64,
    pub task_id: i64,
    pub spent_date: String,
    /// `None` makes Harvest create the entry with a running timer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateTimeEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spent_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// --- User ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub weekly_capacity: Option<i64>,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UsersResponse {
    pub users: Vec<User>,
    pub total_pages: i64,
}

// --- Company ---

/// Subset of `GET /v2/company` — only the web address is used.
#[derive(Debug, Clone, Deserialize)]
pub struct Company {
    /// e.g. "https://acme.harvestapp.com"
    pub base_uri: String,
}

// --- Project user assignment (for listing projects assigned to current user) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAssignment {
    pub id: i64,
    pub project: ProjectRef,
    pub client: ClientRef,
    pub is_active: bool,
    #[serde(default)]
    pub is_project_manager: bool,
    pub task_assignments: Vec<ProjectTaskAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTaskAssignment {
    pub id: i64,
    pub task: TaskRef,
    pub is_active: bool,
    pub billable: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectAssignmentsResponse {
    pub project_assignments: Vec<ProjectAssignment>,
    pub total_pages: i64,
}

/// One row of `GET /reports/project_budget`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectBudgetReport {
    pub project_id: i64,
    pub project_name: String,
    pub client_name: Option<String>,
    pub budget_is_monthly: bool,
    pub budget_by: String,
    pub is_active: bool,
    pub budget: Option<f64>,
    pub budget_spent: f64,
    pub budget_remaining: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectBudgetReportResponse {
    pub results: Vec<ProjectBudgetReport>,
    pub total_pages: i64,
}

/// A task assignment of a project with its optional per-task budget
/// (`GET /projects/{id}/task_assignments`).
#[derive(Debug, Clone, Deserialize)]
pub struct TaskAssignmentBudget {
    pub task: TaskRef,
    pub budget: Option<f64>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskAssignmentsResponse {
    pub task_assignments: Vec<TaskAssignmentBudget>,
    pub total_pages: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"results":[{"project_id":1,"project_name":"Alpha","client_id":2,"client_name":"X","budget_is_monthly":false,"budget_by":"project","is_active":true,"budget":400.0,"budget_spent":312.0,"budget_remaining":88.0}],"per_page":2000,"total_pages":1}"#;

    #[test]
    fn budget_report_deserializes() {
        let r: ProjectBudgetReportResponse = serde_json::from_str(REPORT).unwrap();
        assert_eq!(r.results[0].budget, Some(400.0));
        assert_eq!(r.results[0].budget_by, "project");
    }

    #[test]
    fn budget_report_null_budget() {
        let json = REPORT
            .replace("\"budget\":400.0", "\"budget\":null")
            .replace("\"budget_remaining\":88.0", "\"budget_remaining\":null");
        let r: ProjectBudgetReportResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(r.results[0].budget, None);
        assert_eq!(r.results[0].budget_remaining, None);
    }

    #[test]
    fn assignment_manager_flag_defaults_false() {
        let base = r#"{"id":1,"project":{"id":1,"name":"P","code":null},"client":{"id":1,"name":"C"},"is_active":true,"task_assignments":[]"#;
        let without: ProjectAssignment = serde_json::from_str(&format!("{base}}}")).unwrap();
        assert!(!without.is_project_manager);
        let with: ProjectAssignment =
            serde_json::from_str(&format!("{base},\"is_project_manager\":true}}")).unwrap();
        assert!(with.is_project_manager);
    }

    fn create(hours: Option<f64>) -> serde_json::Value {
        serde_json::to_value(CreateTimeEntry {
            project_id: 1,
            task_id: 2,
            spent_date: "2026-10-05".into(),
            hours,
            notes: None,
        })
        .unwrap()
    }

    #[test]
    fn create_without_hours_omits_the_field_so_harvest_starts_a_timer() {
        assert!(create(None).get("hours").is_none());
        assert_eq!(create(Some(1.5))["hours"], 1.5);
    }
}
