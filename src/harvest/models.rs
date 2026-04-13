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
    pub hours: f64,
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
}

// --- Project user assignment (for listing projects assigned to current user) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAssignment {
    pub id: i64,
    pub project: ProjectRef,
    pub client: ClientRef,
    pub is_active: bool,
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
