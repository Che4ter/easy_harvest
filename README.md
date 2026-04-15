# Easy Harvest

A desktop app for quickly booking and reviewing time entries in [Harvest](https://www.getharvest.com/), without opening the web UI.

## What it does

- **Day view** — see and edit time entries for any day, with hours booked vs. expected and a live progress bar
- **Work day tracker** — track your actual working hours with start, break, and end times
- **Vacation** — log vacation entries in bulk across a date range (weekends and public holidays skipped automatically)
- **Stats** — year-to-date balance, overtime, holiday days used and remaining; manual overtime adjustments
- **Billable overview** — per-project billable breakdown for the year, filterable by month
- **Project budgets** — define hour budgets across one or more projects and track usage with a live progress bar
- **Entry templates** — save project+task+notes combinations for entries you book often (e.g. "Travel Luzern-Olten")
- **Settings** — work profile (weekly hours, percentage, vacation days), carryover values, Swiss public holidays, data folder, and launch-at-startup

The API token is stored in the system keyring (GNOME Keyring, KWallet, Windows Credential Manager), with a plain-file fallback on headless Linux.

## Platform

| OS | Status |
|---|---|
| Linux (X11 / Wayland with AppIndicator) | Full support, system tray icon via D-Bus SNI |
| Windows | Full support, system tray icon in the taskbar notification area |
| macOS | Untested |

On GNOME Wayland, tray icons require the [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/) shell extension.

## Building

Requires Rust (stable). No extra system libraries on Windows. On Linux, GTK 3 and D-Bus development headers are needed for the tray:

```sh
# Fedora / RHEL
sudo dnf install gtk3-devel dbus-devel

# Ubuntu / Debian
sudo apt install libgtk-3-dev libdbus-1-dev
```

Build and run:

```sh
cargo run --release
```

### Cross-compiling for Windows (from Linux)

Install the target and MinGW toolchain once:

```sh
rustup target add x86_64-pc-windows-gnu

# Fedora / RHEL
sudo dnf install mingw64-gcc

# Ubuntu / Debian
sudo apt install gcc-mingw-w64-x86-64
```

Then build:

```sh
cargo build --release --target x86_64-pc-windows-gnu
```

The binary is produced at `target/x86_64-pc-windows-gnu/release/easy_harvest.exe`.

The first run shows a wizard to set the data folder and connect to Harvest (Personal Access Token + Account ID from harvestapp.com → Settings → Developers).

## Data

All state (settings, favorites, work day records, templates) is stored as JSON in the data folder you choose during setup. Pointing this at a OneDrive or Dropbox folder syncs everything across devices.

## Notes

- Swiss public holidays (central cantons) are used for working day calculations and vacation entry creation.
- Expected hours per day are calculated from weekly hours × work percentage.
- Harvest API entries that are locked or billed cannot be edited from the app.
