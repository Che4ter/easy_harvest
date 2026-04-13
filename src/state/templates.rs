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
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(data_dir.join("templates.json"), json)
    }
}
