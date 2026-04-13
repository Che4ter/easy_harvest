# Easy Harvest

A desktop app for quickly booking and reviewing time entries in [Harvest](https://www.getharvest.com/), without opening the web UI.

## What it does

- **Day view** — see and edit time entries for any day, with hours booked vs. expected and a live progress bar
- **Work day tracker** — track your actual working hours with start, break, and end times
- **Vacation** — log vacation entries in bulk across a date range (weekends and public holidays skipped automatically)
- **Stats** — year-to-date balance, overtime, holiday days used and remaining
- **Billable overview** — per-project billable breakdown for the year, filterable by month
- **Entry templates** — save project+task+notes combinations for entries you book often (e.g. "Travel Luzern-Olten")
- **Settings** — work profile (weekly hours, percentage, vacation days), carryover values, Swiss public holidays, and data folder

The API token is stored in the system keyring (GNOME Keyring, KWallet, Windows Credential Manager).

## Platform

| OS | Status |
|---|---|
| Linux (X11 / Wayland with AppIndicator) | Full support, system tray icon via D-Bus SNI |
| Windows | Supported, no system tray (app closes on window close) |
| macOS | Untested |

On GNOME Wayland, tray icons require the [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/) shell extension.

## Building

Requires Rust (stable). No extra system libraries on Windows. On Linux, `dbus` development headers are needed for the tray:

```sh
# Fedora / RHEL
sudo dnf install dbus-devel

# Ubuntu / Debian
sudo apt install libdbus-1-dev
```

Build and run:

```sh
cargo run --release
```

The first run shows a wizard to set the data folder and connect to Harvest (Personal Access Token + Account ID from harvestapp.com → Settings → Developers).

## Data

All state (settings, favorites, work day records, templates) is stored as JSON in the data folder you choose during setup. Pointing this at a OneDrive or Dropbox folder syncs everything across devices.

## Notes

- Swiss public holidays (central cantons) are used for working day calculations and vacation entry creation.
- Expected hours per day are calculated from weekly hours × work percentage.
- Harvest API entries that are locked or billed cannot be edited from the app.
