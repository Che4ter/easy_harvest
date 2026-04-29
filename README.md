# Easy Harvest

A desktop app for quickly booking and reviewing time entries in [Harvest](https://www.getharvest.com/), without opening the web UI.

## What it does

- **Day view** — see and edit time entries for any day; a live progress bar tracks how much of your recorded work time has been booked into Harvest, with an indicator showing overtime or unbooked hours
- **Work day tracker** — record start, break, and end times to track actual hours at work; past days can be edited at any time
- **Vacation** — log vacation entries in bulk across a date range (weekends and public holidays skipped automatically)
- **Stats** — year-to-date balance, overtime, holiday days used and remaining; manual overtime adjustments
- **Billable overview** — per-project billable breakdown for the year, filterable by month
- **Project budgets** — define hour budgets across one or more projects and track usage with a live progress bar
- **Entry templates** — save project+task+notes combinations for entries you book often (e.g. "Travel Luzern-Olten")
- **Settings** — work profile (weekly hours, percentage, vacation days), carryover values, Swiss public holidays, data folder, and launch-at-startup

The API token is stored in the system keyring (GNOME Keyring, KWallet, Windows Credential Manager, macOS Keychain), with a plain-file fallback on headless Linux.

## Platform

| OS | Status |
|---|---|
| Linux (X11 / Wayland with AppIndicator) | Full support, system tray icon via D-Bus SNI |
| Windows | Full support, system tray icon in the taskbar notification area |
| macOS | Supported — no system tray (closing the window exits the app) |

On GNOME Wayland, tray icons require the [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/) shell extension.

## Building

### Linux

Requires Rust (stable) and GTK 3 + D-Bus development headers for the tray:

```sh
# Fedora / RHEL
sudo dnf install gtk3-devel dbus-devel

# Ubuntu / Debian
sudo apt install libgtk-3-dev libdbus-1-dev
```

```sh
cargo run --release
```

### Windows

Requires Rust (stable). No extra system libraries needed.

```sh
cargo run --release
```

### macOS

Requires Rust (stable). No extra system libraries needed.

```sh
cargo run --release
```

To distribute as a proper `.app` bundle (so the app appears in Finder and Spotlight), install [`cargo-bundle`](https://github.com/burtonageo/cargo-bundle) and run:

```sh
cargo install cargo-bundle
cargo build --release   # generates icon PNGs in target/icons/ needed by cargo-bundle
cargo bundle --release
```

This produces `target/release/bundle/osx/Easy Harvest.app`. You can drag it to `/Applications` like any other Mac app.

> **Note:** Without code signing the app will be blocked by Gatekeeper on first launch. Right-click → Open to bypass it, or sign with an Apple Developer certificate.

**Launch at startup** is managed via a `LaunchAgent` plist written to `~/Library/LaunchAgents/com.easyharvest.plist`. The toggle in Settings handles this automatically.

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

> **Windows SmartScreen:** Because the binary is not code-signed, Windows may show a "Windows protected your PC" warning on first launch. Click **More info → Run anyway** to proceed. This is expected for unsigned open-source apps.

## Data

All state (settings, favorites, work day records, templates, overtime adjustments, project budgets) is stored as JSON in the data folder you choose during setup. Pointing this at a OneDrive or Dropbox folder syncs everything across devices.

Changing the data folder in Settings migrates all existing files to the new location automatically.

## Notes

- Swiss public holidays (central cantons) are used for working day calculations and vacation entry creation.
- Expected hours per day are calculated from weekly hours × work percentage.
- Harvest API entries that are locked or billed cannot be edited from the app.
