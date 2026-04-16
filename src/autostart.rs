/// Returns `true` if an autostart entry for this app exists on the current platform.
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

/// Creates the platform-specific autostart entry pointing at the current executable.
/// Returns `Err` with a description if the operation failed.
pub fn enable() -> Result<(), String> {
    platform::enable()
}

/// Installs the app icon to the freedesktop hicolor icon theme on Linux so that
/// `.desktop` files with `Icon=easy_harvest` resolve correctly.  No-op on
/// non-Linux platforms.
pub fn install_icon() {
    #[cfg(target_os = "linux")]
    platform::install_icon();
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
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Easy Harvest\n\
             Comment=Book and review Harvest time entries without opening the web UI\n\
             Exec=\"{}\"\n\
             Icon=easy_harvest\n\
             Categories=Office;ProjectManagement;\n\
             Keywords=harvest;time;tracking;\n\
             Hidden=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())
    }

    /// Write the embedded 128×128 PNG to the freedesktop hicolor icon theme so
    /// `Icon=easy_harvest` resolves in both the autostart `.desktop` and any
    /// manually-installed launcher entry.
    pub fn install_icon() {
        const ICON_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/easy_harvest_128.png"));

        let Some(data_home) = dirs::data_dir() else { return; };
        let icon_dir = data_home.join("icons/hicolor/128x128/apps");
        if std::fs::create_dir_all(&icon_dir).is_err() {
            return;
        }
        let icon_path = icon_dir.join("easy_harvest.png");
        let _ = std::fs::write(&icon_path, ICON_PNG);

        // Best-effort: update the icon cache so the icon is visible immediately.
        let _ = std::process::Command::new("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(data_home.join("icons/hicolor"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
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

// ── macOS ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;

    const LABEL: &str = "com.easyharvest";

    fn plist_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    }

    pub fn is_enabled() -> bool {
        plist_path().exists()
    }

    pub fn enable() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe.to_string_lossy();
        let path = plist_path();
        std::fs::create_dir_all(path.parent().expect("plist path always has a parent"))
            .map_err(|e| e.to_string())?;
        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key>\n\
             \t<string>{LABEL}</string>\n\
             \t<key>ProgramArguments</key>\n\
             \t<array>\n\
             \t\t<string>{exe_str}</string>\n\
             \t</array>\n\
             \t<key>RunAtLoad</key>\n\
             \t<true/>\n\
             </dict>\n\
             </plist>\n"
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())
    }

    pub fn disable() -> Result<(), String> {
        let path = plist_path();
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
}

// ── Unsupported platforms ─────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
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
