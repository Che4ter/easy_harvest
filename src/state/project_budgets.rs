use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBudget {
    pub id: u64,
    pub name: String,
    pub budget_hours: f64,
    pub project_ids: Vec<i64>,
    #[serde(default)]
    pub task_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBudgetStore {
    pub next_id: u64,
    pub years: HashMap<i32, Vec<ProjectBudget>>,
}

impl Default for ProjectBudgetStore {
    fn default() -> Self {
        Self {
            next_id: 1,
            years: HashMap::new(),
        }
    }
}

impl ProjectBudgetStore {
    pub fn budgets_for(&self, year: i32) -> &[ProjectBudget] {
        self.years.get(&year).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn budgets_for_mut(&mut self, year: i32) -> &mut Vec<ProjectBudget> {
        self.years.entry(year).or_default()
    }

    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("project_budgets.json");
        // Try new year-keyed format first (silent failure — old format tried next).
        if let Some(store) = super::io::try_load_json::<Self>(&path) {
            return store;
        }
        // Try migrating from the old flat format.
        if let Some(old) = super::io::load_json::<LegacyProjectBudgets>(&path) {
            let current_year = chrono::Local::now().naive_local().year();
            let budgets: Vec<ProjectBudget> = old
                .budgets
                .into_iter()
                .map(|b| ProjectBudget {
                    id: b.id,
                    name: b.name,
                    budget_hours: b.budget_hours,
                    project_ids: b.project_ids,
                    task_ids: b.task_ids,
                })
                .collect();
            let mut years = HashMap::new();
            if !budgets.is_empty() {
                years.insert(current_year, budgets);
            }
            return Self {
                next_id: old.next_id,
                years,
            };
        }
        Self::default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json =
            serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        super::io::atomic_write(&data_dir.join("project_budgets.json"), &json)
    }
}

// ── Legacy format for backward-compatible migration ─────────────────────────

use chrono::Datelike;

#[derive(Deserialize)]
struct LegacyProjectBudget {
    id: u64,
    name: String,
    budget_hours: f64,
    project_ids: Vec<i64>,
    #[serde(default)]
    task_ids: Vec<i64>,
    // Ignored fields from the old format
    #[serde(default)]
    #[allow(dead_code)]
    start_date: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    created_at: Option<String>,
}

#[derive(Deserialize)]
struct LegacyProjectBudgets {
    next_id: u64,
    budgets: Vec<LegacyProjectBudget>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let store = ProjectBudgetStore::default();
        assert_eq!(store.next_id, 1);
        assert!(store.years.is_empty());
        assert!(store.budgets_for(2026).is_empty());
    }

    #[test]
    fn budgets_for_mut_creates_entry() {
        let mut store = ProjectBudgetStore::default();
        assert!(!store.years.contains_key(&2026));
        let budgets = store.budgets_for_mut(2026);
        budgets.push(ProjectBudget {
            id: 1,
            name: "Test".into(),
            budget_hours: 100.0,
            project_ids: vec![42],
            task_ids: vec![],
        });
        assert_eq!(store.budgets_for(2026).len(), 1);
        assert!(store.budgets_for(2025).is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProjectBudgetStore::default();
        store.budgets_for_mut(2025).push(ProjectBudget {
            id: 1,
            name: "Alpha".into(),
            budget_hours: 50.0,
            project_ids: vec![10],
            task_ids: vec![20, 30],
        });
        store.budgets_for_mut(2026).push(ProjectBudget {
            id: 2,
            name: "Beta".into(),
            budget_hours: 200.0,
            project_ids: vec![11, 12],
            task_ids: vec![],
        });
        store.next_id = 3;
        store.save(dir.path()).unwrap();

        let loaded = ProjectBudgetStore::load(dir.path());
        assert_eq!(loaded.next_id, 3);
        assert_eq!(loaded.budgets_for(2025).len(), 1);
        assert_eq!(loaded.budgets_for(2025)[0].name, "Alpha");
        assert_eq!(loaded.budgets_for(2026).len(), 1);
        assert_eq!(loaded.budgets_for(2026)[0].project_ids, vec![11, 12]);
    }

    #[test]
    fn migrate_legacy_format() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_json = serde_json::json!({
            "next_id": 5,
            "budgets": [
                {
                    "id": 1,
                    "name": "Old Budget",
                    "budget_hours": 100.0,
                    "project_ids": [42],
                    "task_ids": [],
                    "start_date": "2025-01-01",
                    "created_at": "2025-06-01T10:00:00"
                }
            ]
        });
        std::fs::write(
            dir.path().join("project_budgets.json"),
            serde_json::to_string_pretty(&legacy_json).unwrap(),
        )
        .unwrap();

        let loaded = ProjectBudgetStore::load(dir.path());
        assert_eq!(loaded.next_id, 5);
        let current_year = chrono::Local::now().naive_local().year();
        let budgets = loaded.budgets_for(current_year);
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].name, "Old Budget");
        assert_eq!(budgets[0].id, 1);
    }
}
