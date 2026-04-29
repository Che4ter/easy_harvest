use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::harvest::models::ProjectAssignment;

// ---------------------------------------------------------------------------
// Stored data
// ---------------------------------------------------------------------------

/// Per project+task usage record.  Serialised into `favorites.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub project_id: i64,
    pub task_id: i64,
    /// Pinned entries always appear at the top of the project picker.
    pub is_pinned: bool,
    pub use_count: u32,
}

/// Usage frequency store and pin list, persisted at `<data_dir>/favorites.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Favorites {
    entries: Vec<UsageEntry>,
}

// ---------------------------------------------------------------------------
// Display-ready output
// ---------------------------------------------------------------------------

/// A fully resolved, display-ready entry for the project/task picker.
///
/// Produced by [`Favorites::sorted_options`], sorted so that:
/// 1. Pinned entries come first (within that group: by `use_count` desc, then alphabetical).
/// 2. Used-but-unpinned entries next (same sub-sort).
/// 3. Never-used entries last (alphabetical).
#[derive(Debug, Clone)]
pub struct ProjectOption {
    pub project_id: i64,
    pub task_id: i64,
    pub client_name: String,
    pub project_name: String,
    pub task_name: String,
    pub is_pinned: bool,
    pub use_count: u32,
    /// Pre-formatted for search: `"ClientName > ProjectName — TaskName"`.
    pub search_text: String,
}

impl ProjectOption {
    /// Multi-token case-insensitive match against the formatted search text.
    ///
    /// Splits `query` on whitespace; ALL tokens must appear somewhere in `search_text`.
    /// An empty query (or a query of only whitespace) matches everything.
    ///
    /// Example: "base service dev" matches
    /// "baseVISION AG > IR Retainer … Service Dev (SOC) — Service Development"
    pub fn matches_query(&self, query: &str) -> bool {
        if query.trim().is_empty() {
            return true;
        }
        let haystack = self.search_text.to_lowercase();
        query.split_whitespace().all(|token| haystack.contains(token.to_lowercase().as_str()))
    }
}

// ---------------------------------------------------------------------------
// Favorites impl
// ---------------------------------------------------------------------------

impl Favorites {
    fn find_mut(&mut self, project_id: i64, task_id: i64) -> Option<&mut UsageEntry> {
        self.entries
            .iter_mut()
            .find(|e| e.project_id == project_id && e.task_id == task_id)
    }

    /// Record a booking — increments `use_count` for the given project+task.
    /// Creates a new entry if none exists yet.
    pub fn record_use(&mut self, project_id: i64, task_id: i64) {
        if let Some(e) = self.find_mut(project_id, task_id) {
            e.use_count += 1;
        } else {
            self.entries.push(UsageEntry {
                project_id,
                task_id,
                is_pinned: false,
                use_count: 1,
            });
        }
    }

    /// Toggle the pinned state.  Creates a new pinned entry if none exists.
    pub fn toggle_pin(&mut self, project_id: i64, task_id: i64) {
        if let Some(e) = self.find_mut(project_id, task_id) {
            e.is_pinned = !e.is_pinned;
        } else {
            self.entries.push(UsageEntry {
                project_id,
                task_id,
                is_pinned: true,
                use_count: 0,
            });
        }
    }

    /// Return all active project+task combos as sorted [`ProjectOption`]s.
    ///
    /// Only active project assignments and active task assignments are included.
    pub fn sorted_options(&self, assignments: &[ProjectAssignment]) -> Vec<ProjectOption> {
        let mut options: Vec<ProjectOption> = assignments
            .iter()
            .filter(|pa| pa.is_active)
            .flat_map(|pa| {
                pa.task_assignments
                    .iter()
                    .filter(|ta| ta.is_active)
                    .map(move |ta| {
                        let usage = self
                            .entries
                            .iter()
                            .find(|e| e.project_id == pa.project.id && e.task_id == ta.task.id);

                        let search_text = format!(
                            "{} > {} — {}",
                            pa.client.name, pa.project.name, ta.task.name
                        );

                        ProjectOption {
                            project_id: pa.project.id,
                            task_id: ta.task.id,
                            client_name: pa.client.name.clone(),
                            project_name: pa.project.name.clone(),
                            task_name: ta.task.name.clone(),
                            is_pinned: usage.is_some_and(|e| e.is_pinned),
                            use_count: usage.map_or(0, |e| e.use_count),
                            search_text,
                        }
                    })
            })
            .collect();

        options.sort_by(|a, b| {
            // Pinned before unpinned (descending bool)
            b.is_pinned
                .cmp(&a.is_pinned)
                // Higher use_count first
                .then_with(|| b.use_count.cmp(&a.use_count))
                // Alphabetical tiebreak
                .then_with(|| a.search_text.cmp(&b.search_text))
        });

        options
    }

    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("favorites.json");
        super::io::load_json(&path).unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&data_dir.join("favorites.json"), &json)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harvest::models::{
        ClientRef, ProjectAssignment, ProjectRef, ProjectTaskAssignment, TaskRef,
    };

    fn make_assignments() -> Vec<ProjectAssignment> {
        vec![
            ProjectAssignment {
                id: 1,
                project: ProjectRef { id: 10, name: "Website".into(), code: None },
                client: ClientRef { id: 1, name: "Acme".into() },
                is_active: true,
                task_assignments: vec![
                    ProjectTaskAssignment {
                        id: 100,
                        task: TaskRef { id: 200, name: "Backend".into() },
                        is_active: true,
                        billable: Some(true),
                    },
                    ProjectTaskAssignment {
                        id: 101,
                        task: TaskRef { id: 201, name: "Frontend".into() },
                        is_active: true,
                        billable: Some(true),
                    },
                ],
            },
            ProjectAssignment {
                id: 2,
                project: ProjectRef { id: 11, name: "App".into(), code: None },
                client: ClientRef { id: 2, name: "Beta Corp".into() },
                is_active: true,
                task_assignments: vec![ProjectTaskAssignment {
                    id: 102,
                    task: TaskRef { id: 202, name: "Dev".into() },
                    is_active: true,
                    billable: Some(true),
                }],
            },
        ]
    }

    #[test]
    fn test_record_use_creates_entry() {
        let mut fav = Favorites::default();
        fav.record_use(10, 200);
        assert_eq!(fav.entries[0].use_count, 1);
        assert_eq!(fav.entries[0].project_id, 10);
    }

    #[test]
    fn test_record_use_increments_count() {
        let mut fav = Favorites::default();
        fav.record_use(10, 200);
        fav.record_use(10, 200);
        fav.record_use(10, 200);
        assert_eq!(fav.entries[0].use_count, 3);
    }

    #[test]
    fn test_toggle_pin_creates_and_flips() {
        let mut fav = Favorites::default();
        fav.toggle_pin(10, 200);
        assert!(fav.entries[0].is_pinned);
        fav.toggle_pin(10, 200);
        assert!(!fav.entries[0].is_pinned);
    }

    #[test]
    fn test_sorted_options_pinned_first() {
        let assignments = make_assignments();
        let mut fav = Favorites::default();
        // Use "Dev" once; pin "Backend"
        fav.record_use(11, 202);
        fav.toggle_pin(10, 200);

        let opts = fav.sorted_options(&assignments);
        assert_eq!(opts[0].task_name, "Backend", "pinned should be first");
    }

    #[test]
    fn test_sorted_options_use_count_before_alphabetical() {
        let assignments = make_assignments();
        let mut fav = Favorites::default();
        // "Dev" used twice; "Frontend" used once — both unpinned
        fav.record_use(11, 202);
        fav.record_use(11, 202);
        fav.record_use(10, 201);

        let opts = fav.sorted_options(&assignments);
        // Dev (count=2) should come before Frontend (count=1)
        let dev_pos = opts.iter().position(|o| o.task_name == "Dev").unwrap();
        let fe_pos = opts.iter().position(|o| o.task_name == "Frontend").unwrap();
        assert!(dev_pos < fe_pos);
    }

    #[test]
    fn test_sorted_options_alphabetical_for_unused() {
        let assignments = make_assignments();
        let fav = Favorites::default();
        let opts = fav.sorted_options(&assignments);
        // All use_count=0 and none pinned → alphabetical by search_text
        let texts: Vec<&str> = opts.iter().map(|o| o.search_text.as_str()).collect();
        let mut sorted = texts.clone();
        sorted.sort();
        assert_eq!(texts, sorted);
    }

    #[test]
    fn test_matches_query_case_insensitive() {
        let opt = ProjectOption {
            project_id: 10,
            task_id: 200,
            client_name: "Acme".into(),
            project_name: "Website".into(),
            task_name: "Backend".into(),
            is_pinned: false,
            use_count: 0,
            search_text: "Acme > Website — Backend".into(),
        };
        assert!(opt.matches_query("acme"));
        assert!(opt.matches_query("BACKEND"));
        assert!(opt.matches_query(""));
        assert!(!opt.matches_query("xyz"));
    }

    #[test]
    fn test_inactive_assignments_excluded() {
        let mut assignments = make_assignments();
        assignments[0].is_active = false;

        let fav = Favorites::default();
        let opts = fav.sorted_options(&assignments);
        // Only the second project's tasks should appear
        assert!(opts.iter().all(|o| o.project_id == 11));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fav = Favorites::default();
        fav.record_use(10, 200);
        fav.record_use(10, 200);
        fav.toggle_pin(11, 202);
        fav.save(dir.path()).expect("save failed");

        let loaded = Favorites::load(dir.path());
        assert_eq!(loaded.entries.len(), 2);
        let backend = loaded.entries.iter().find(|e| e.task_id == 200).unwrap();
        assert_eq!(backend.use_count, 2);
        let dev = loaded.entries.iter().find(|e| e.task_id == 202).unwrap();
        assert!(dev.is_pinned);
    }

    #[test]
    fn test_load_returns_default_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fav = Favorites::load(dir.path());
        assert!(fav.entries.is_empty());
    }

    #[test]
    fn test_inactive_task_assignment_excluded() {
        // Project is active, but one of its task assignments is inactive.
        // Only the active task should appear in sorted_options.
        let mut assignments = make_assignments();
        assignments[0].task_assignments[1].is_active = false; // deactivate "Frontend"

        let fav = Favorites::default();
        let opts = fav.sorted_options(&assignments);
        assert!(
            opts.iter().all(|o| o.task_name != "Frontend"),
            "inactive task assignment must be excluded"
        );
        assert!(opts.iter().any(|o| o.task_name == "Backend"), "active task must still appear");
    }

    #[test]
    fn test_matches_query_multi_token_all_must_match() {
        let opt = ProjectOption {
            project_id: 10,
            task_id: 200,
            client_name: "Acme".into(),
            project_name: "Website".into(),
            task_name: "Backend".into(),
            is_pinned: false,
            use_count: 0,
            search_text: "Acme > Website — Backend".into(),
        };
        // Both tokens present → match.
        assert!(opt.matches_query("acme backend"));
        // Only one of two tokens present → no match.
        assert!(!opt.matches_query("acme xyz"));
        // Three tokens, all present → match.
        assert!(opt.matches_query("acme website backend"));
    }

    #[test]
    fn test_matches_query_partial_substring() {
        let opt = ProjectOption {
            project_id: 10,
            task_id: 200,
            client_name: "Acme".into(),
            project_name: "Website".into(),
            task_name: "Backend".into(),
            is_pinned: false,
            use_count: 0,
            search_text: "Acme > Website — Backend".into(),
        };
        // Partial / substring match is supported.
        assert!(opt.matches_query("acm"));
        assert!(opt.matches_query("ack")); // "Backend" contains "ack"
        assert!(!opt.matches_query("acmez")); // no match when substring not present
    }
}
