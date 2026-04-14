/// Returns `true` if an autostart entry for this app exists on the current platform.
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

/// Creates the platform-specific autostart entry pointing at the current executable.
/// Returns `Err` with a description if the operation failed.
pub fn enable() -> Result<(), String> {
    platform::enable()
}

/// Removes the platform-specific autostart entry.
/// Returns `Err` with a description if the operation failed.
pub fn disable() -> Result<(), String> {
    platform::disable()
}

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use std::path::PathBuf;

    fn desktop_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".config")
            })
            .join("autostart/easy_harvest.desktop")
    }

    pub fn is_enabled() -> bool {
        desktop_path().exists()
    }

    pub fn enable() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = desktop_path();
        std::fs::create_dir_all(path.parent().expect("desktop path always has a parent"))
            .map_err(|e| e.to_string())?;
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Easy Harvest\nExec=\"{}\"\nHidden=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        let path = desktop_path();
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "EasyHarvest";

    pub fn is_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .and_then(|key| key.get_value::<String, _>(VALUE_NAME))
            .is_ok()
    }

    pub fn enable() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe.to_string_lossy().into_owned();
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
            .and_then(|key| key.set_value(VALUE_NAME, &exe_str))
            .map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
            .and_then(|key| key.delete_value(VALUE_NAME))
            .or_else(|e| {
                // Treat "value not found" as success
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| e.to_string())
    }
}

// ── Unsupported platforms (macOS, etc.) ───────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod platform {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn enable() -> Result<(), String> {
        Ok(())
    }
    pub fn disable() -> Result<(), String> {
        Ok(())
    }
}
