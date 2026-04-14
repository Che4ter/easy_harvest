use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::harvest::models::ProjectAssignment;

/// Cached list of project assignments with a 24-hour TTL.
///
/// The cache lives at `<data_dir>/cache/project_assignments.json`.
///
/// # Typical usage
///
/// ```ignore
/// let assignments = match ProjectCache::load(&data_dir) {
///     Some(c) if c.is_valid() => c.assignments,
///     _ => {
///         let fresh = client.list_all_my_project_assignments().await?;
///         ProjectCache::new(fresh.clone()).save(&data_dir).ok();
///         fresh
///     }
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectCache {
    pub assignments: Vec<ProjectAssignment>,
    pub fetched_at: DateTime<Utc>,
}

impl ProjectCache {
    /// Create a new cache stamped with the current UTC time.
    pub fn new(assignments: Vec<ProjectAssignment>) -> Self {
        Self {
            assignments,
            fetched_at: Utc::now(),
        }
    }

    /// Returns `true` if the cache is younger than 24 hours.
    pub fn is_valid(&self) -> bool {
        Utc::now().signed_duration_since(self.fetched_at) < Duration::hours(24)
    }

    /// Load from disk.  Returns `None` if the file is missing or cannot be parsed.
    pub fn load(data_dir: &Path) -> Option<Self> {
        let path = data_dir.join("cache").join("project_assignments.json");
        super::io::load_json(&path)
    }

    /// Persist to `<data_dir>/cache/project_assignments.json`.
    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        let dir = data_dir.join("cache");
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&dir.join("project_assignments.json"), &json)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harvest::models::{ClientRef, ProjectAssignment, ProjectRef, ProjectTaskAssignment, TaskRef};

    fn dummy_assignments() -> Vec<ProjectAssignment> {
        vec![ProjectAssignment {
            id: 1,
            project: ProjectRef { id: 10, name: "Alpha".into(), code: None },
            client: ClientRef { id: 20, name: "Acme".into() },
            is_active: true,
            task_assignments: vec![ProjectTaskAssignment {
                id: 100,
                task: TaskRef { id: 200, name: "Dev".into() },
                is_active: true,
                billable: Some(true),
            }],
        }]
    }

    #[test]
    fn test_new_cache_is_valid() {
        let cache = ProjectCache::new(dummy_assignments());
        assert!(cache.is_valid());
    }

    #[test]
    fn test_stale_cache_is_invalid() {
        let mut cache = ProjectCache::new(dummy_assignments());
        // Push fetched_at 25 hours into the past.
        cache.fetched_at = Utc::now() - Duration::hours(25);
        assert!(!cache.is_valid());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = ProjectCache::new(dummy_assignments());
        cache.save(dir.path()).expect("save failed");

        let loaded = ProjectCache::load(dir.path()).expect("load returned None");
        assert_eq!(loaded.assignments.len(), 1);
        assert_eq!(loaded.assignments[0].project.name, "Alpha");
        assert!(loaded.is_valid());
    }

    #[test]
    fn test_load_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(ProjectCache::load(dir.path()).is_none());
    }
}
