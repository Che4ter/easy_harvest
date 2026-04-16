#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use easy_harvest::app;

fn main() -> iced::Result {
    // Install the freedesktop icon so app launchers (GNOME, KDE, etc.) show
    // the Easy Harvest icon.  No-op on non-Linux platforms.
    easy_harvest::autostart::install_icon();

    // Re-register the autostart entry with the current executable path so the
    // registry key stays correct if the binary is ever renamed or moved.
    #[cfg(target_os = "windows")]
    if easy_harvest::autostart::is_enabled() {
        let _ = easy_harvest::autostart::enable();
    }

    app::run()
}
