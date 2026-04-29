use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Returns the default data directory.
///
/// On Windows we try the OneDrive folder first so that files are
/// automatically synced without any extra setup.  On every other
/// platform we fall back to the OS data directory.
pub fn default_data_dir() -> PathBuf {
    if cfg!(target_os = "windows")
        && let Some(home) = dirs::home_dir() {
            let onedrive = home.join("OneDrive").join("EasyHarvest");
            if onedrive.parent().is_some_and(|p| p.exists()) {
                return onedrive;
            }
        }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("easy_harvest")
}

/// Tiny config stored in the OS *config* dir (not the data dir) so that
/// the data dir itself can be changed without losing the pointer to it.
///
/// Location:
///  - Linux   `~/.config/easy_harvest/bootstrap.json`
///  - Windows `%APPDATA%\easy_harvest\bootstrap.json`
///  - macOS   `~/Library/Application Support/easy_harvest/bootstrap.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub data_dir: PathBuf,
}

impl BootstrapConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("easy_harvest")
            .join("bootstrap.json")
    }

    /// Load from disk.  Falls back to `default_data_dir()` if the file
    /// does not exist or cannot be parsed.
    pub fn load() -> Self {
        let path = Self::config_path();
        super::io::load_json(&path).unwrap_or_else(|| Self { data_dir: default_data_dir() })
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&path, &json)
    }
}
