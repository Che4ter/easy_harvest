#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use easy_harvest::app;

fn main() -> iced::Result {
    // Install the freedesktop icon so app launchers (GNOME, KDE, etc.) show
    // the Easy Harvest icon.  No-op on non-Linux platforms.
    easy_harvest::autostart::install_icon();

    app::run()
}
