use std::path::Path;

use serde::{Deserialize, Serialize};

/// A saved entry template for quickly booking common tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryTemplate {
    /// Short display name shown on the quick-select chip (e.g. "Travel Luzern-Olten").
    pub label: String,
    pub project_id: i64,
    pub task_id: i64,
    /// Default hours string (e.g. "1:30").  Empty string = no default.
    pub hours: String,
    /// Pre-filled notes.
    pub notes: String,
}

/// Collection of entry templates, persisted to `<data_dir>/templates.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Templates {
    pub entries: Vec<EntryTemplate>,
}

impl Templates {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("templates.json");
        super::io::load_json(&path).unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&data_dir.join("templates.json"), &json)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_empty_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let t = Templates::load(dir.path());
        assert!(t.entries.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Templates::default();
        t.entries.push(EntryTemplate {
            label: "Travel Luzern-Olten".into(),
            project_id: 42,
            task_id: 7,
            hours: "1:30".into(),
            notes: "Return ticket".into(),
        });
        t.entries.push(EntryTemplate {
            label: "Weekly sync".into(),
            project_id: 10,
            task_id: 3,
            hours: String::new(),
            notes: String::new(),
        });
        t.save(dir.path()).unwrap();

        let loaded = Templates::load(dir.path());
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].label, "Travel Luzern-Olten");
        assert_eq!(loaded.entries[0].project_id, 42);
        assert_eq!(loaded.entries[0].hours, "1:30");
        assert_eq!(loaded.entries[1].label, "Weekly sync");
        assert!(loaded.entries[1].notes.is_empty());
    }

    #[test]
    fn save_overwrites_previous_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Templates::default();
        t.entries.push(EntryTemplate {
            label: "Old".into(), project_id: 1, task_id: 1,
            hours: String::new(), notes: String::new(),
        });
        t.save(dir.path()).unwrap();

        let mut t2 = Templates::default();
        t2.entries.push(EntryTemplate {
            label: "New".into(), project_id: 2, task_id: 2,
            hours: String::new(), notes: String::new(),
        });
        t2.save(dir.path()).unwrap();

        let loaded = Templates::load(dir.path());
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].label, "New");
    }
}
