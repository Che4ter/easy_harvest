#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use easy_harvest::app;

fn main() -> iced::Result {
    app::run()
}
