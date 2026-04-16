use std::path::{Path, PathBuf};

use std::collections::HashMap;
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Year-indexed carryover
// ---------------------------------------------------------------------------

/// Carryover values for a specific year (carried *into* that year).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YearCarryover {
    /// Vacation days carried into this year (absolute, already adjusted for %).
    #[serde(default)]
    pub holiday_days: f64,
    /// Overtime hours carried into this year (positive = banked, negative = deficit).
    #[serde(default)]
    pub overtime_hours: f64,
}

// ---------------------------------------------------------------------------
// Public holidays
// ---------------------------------------------------------------------------

/// A public/national holiday that reduces expected working hours.
///
/// The credit is resolved at calculation time from `half_day` and the caller's
/// `expected_hours_per_day`, so it adapts automatically when work percentage
/// changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicHoliday {
    pub date: NaiveDate,
    pub name: String,
    /// If true, only half a working day's credit (e.g. "Tag der Arbeit ab 12:00").
    #[serde(default)]
    pub half_day: bool,
}

impl PublicHoliday {
    /// Resolve the credit hours based on the daily target.
    pub fn credit_hours(&self, expected_hours_per_day: f64) -> f64 {
        if self.half_day {
            expected_hours_per_day / 2.0
        } else {
            expected_hours_per_day
        }
    }
}

/// Easter Sunday for the given year (Anonymous Gregorian algorithm / Computus).
fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        .unwrap_or_else(|| panic!("easter algorithm produced invalid date for year {year}: month={month} day={day}"))
}

/// Generate Swiss cantonal public holidays for the given year.
///
/// Based on a typical central-Swiss canton (Luzern-style) plus common
/// company-observed days (Ostermontag, Pfingstmontag, Heiligabend,
/// Stephanstag, Silvester).
pub fn swiss_public_holidays(year: i32) -> Vec<PublicHoliday> {
    let easter = easter_sunday(year);
    let d = |m, day| {
        NaiveDate::from_ymd_opt(year, m, day)
            .unwrap_or_else(|| panic!("fixed holiday date invalid for year {year}: month={m} day={day}"))
    };
    vec![
        PublicHoliday { date: d(1, 1),               name: "Neujahr".into(),           half_day: false },
        PublicHoliday { date: easter - Duration::days(2), name: "Karfreitag".into(),    half_day: false },
        PublicHoliday { date: easter + Duration::days(1), name: "Ostermontag".into(),   half_day: false },
        PublicHoliday { date: d(5, 1),               name: "Tag der Arbeit".into(),     half_day: true  },
        PublicHoliday { date: easter + Duration::days(39), name: "Auffahrt".into(),     half_day: false },
        PublicHoliday { date: easter + Duration::days(50), name: "Pfingstmontag".into(),half_day: false },
        PublicHoliday { date: easter + Duration::days(60), name: "Fronleichnam".into(), half_day: false },
        PublicHoliday { date: d(8, 1),               name: "Bundesfeiertag".into(),     half_day: false },
        PublicHoliday { date: d(8, 15),              name: "Mariä Himmelfahrt".into(),  half_day: false },
        PublicHoliday { date: d(11, 1),              name: "Allerheiligen".into(),       half_day: false },
        PublicHoliday { date: d(12, 24),             name: "Heiligabend".into(),         half_day: true  },
        PublicHoliday { date: d(12, 25),             name: "Weihnachtstag".into(),       half_day: false },
        PublicHoliday { date: d(12, 26),             name: "Stephanstag".into(),         half_day: false },
        PublicHoliday { date: d(12, 31),             name: "Silvester".into(),           half_day: true  },
    ]
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub account_id: String,

    /// Runtime-only — always set by `load()`, never serialised.
    #[serde(skip)]
    pub data_dir: PathBuf,

    /// Total contracted weekly hours before applying the work percentage.
    /// e.g. 41.0 for a standard Swiss 41-hour week.
    #[serde(default = "default_total_weekly_hours")]
    pub total_weekly_hours: f64,

    /// Fraction of full-time employment (0.0–1.0). e.g. 0.8 = 80%.
    #[serde(default = "default_work_percentage")]
    pub work_percentage: f64,

    pub default_break_minutes: u32,

    /// Vacation days per year (same for full- and part-time; hours/day adjusts via work_percentage).
    #[serde(default = "default_holiday_days")]
    pub total_holiday_days_per_year: u32,

    /// Vacation days carried over from the previous year (positive = banked).
    /// Already adjusted for percentage — stored as absolute days.
    /// Keyed by the year the values are carried *into*.
    #[serde(default)]
    pub carryover: HashMap<i32, YearCarryover>,

    pub holiday_task_ids: Vec<i64>,

    /// Optional first day of employment.  If set and `year` matches, vacation
    /// entitlement is prorated by the fraction of the year actually worked.
    #[serde(default)]
    pub first_work_day: Option<NaiveDate>,

    /// Whether the app is registered to launch at login.
    /// Synced with the OS autostart state on every load — not persisted to JSON.
    #[serde(skip)]
    pub autostart: bool,
}

fn default_total_weekly_hours() -> f64 {
    41.0
}

fn default_work_percentage() -> f64 {
    1.0
}

fn default_holiday_days() -> u32 {
    25
}

impl Settings {
    /// Effective daily hours = (total_weekly_hours × work_percentage) / 5.
    pub fn expected_hours_per_day(&self) -> f64 {
        (self.total_weekly_hours * self.work_percentage) / 5.0
    }

    /// Effective weekly hours = total_weekly_hours × work_percentage.
    pub fn expected_hours_per_week(&self) -> f64 {
        self.total_weekly_hours * self.work_percentage
    }

    /// Total vacation days for the year, plus carryover.
    ///
    /// Part-time workers get the same number of vacation *days* as full-time;
    /// only the hours per day differ (handled by `expected_hours_per_day`).
    /// When `first_work_day` falls within `year`, entitlement is prorated by
    /// the fraction of the year actually worked.
    pub fn effective_holiday_days_for(&self, year: i32) -> f64 {
        let base = if let Some(fwd) = self.first_work_day {
            if fwd.year() == year {
                let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
                let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
                let days_in_year = year_end.signed_duration_since(year_start).num_days() + 1;
                let days_worked = (year_end.signed_duration_since(fwd).num_days() + 1)
                    .min(days_in_year)
                    .max(0);
                self.total_holiday_days_per_year as f64
                    * days_worked as f64
                    / days_in_year as f64
            } else {
                self.total_holiday_days_per_year as f64
            }
        } else {
            self.total_holiday_days_per_year as f64
        };
        base + self.carryover.get(&year).map(|c| c.holiday_days).unwrap_or(0.0)
    }

    /// Overtime carryover hours for the given year (0.0 if not set).
    pub fn overtime_carryover_for(&self, year: i32) -> f64 {
        self.carryover.get(&year).map(|c| c.overtime_hours).unwrap_or(0.0)
    }

    /// Check that numeric fields are within sane bounds.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.total_weekly_hours.is_nan() || self.work_percentage.is_nan() {
            return Err("numeric settings must not be NaN");
        }
        if !(0.0..=168.0).contains(&self.total_weekly_hours) {
            return Err("total_weekly_hours must be between 0 and 168");
        }
        if !(0.0..=1.0).contains(&self.work_percentage) {
            return Err("work_percentage must be between 0.0 and 1.0");
        }
        Ok(())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            data_dir: default_data_dir(),
            total_weekly_hours: default_total_weekly_hours(),
            work_percentage: default_work_percentage(),
            default_break_minutes: 60,
            total_holiday_days_per_year: default_holiday_days(),
            carryover: HashMap::new(),
            holiday_task_ids: Vec::new(),
            first_work_day: None,
            autostart: false,
        }
    }
}

fn default_data_dir() -> PathBuf {
    crate::state::bootstrap::default_data_dir()
}

impl Settings {
    pub fn settings_path(data_dir: &Path) -> PathBuf {
        data_dir.join("settings.json")
    }

    pub fn load(data_dir: &Path) -> Self {
        let path = Self::settings_path(data_dir);
        let mut settings: Self = super::io::load_json(&path)
            .and_then(|s: Self| s.validate().ok().map(|_| s))
            .unwrap_or_else(|| Self {
                data_dir: data_dir.to_path_buf(),
                ..Default::default()
            });
        settings.data_dir = data_dir.to_path_buf();
        settings.autostart = crate::autostart::is_enabled();
        settings
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.data_dir)?;
        let path = Self::settings_path(&self.data_dir);
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&path, &json)
    }

    /// Load the API token — tries OS keyring first, falls back to a plain file.
    pub fn load_token(data_dir: &Path) -> Option<String> {
        // Try OS keyring
        if let Some(token) = keyring::Entry::new("easy_harvest", "harvest_api_token")
            .ok()
            .and_then(|e| e.get_password().ok())
        {
            return Some(token);
        }
        // File fallback (keyring unavailable, e.g. on headless Linux)
        std::fs::read_to_string(Self::token_file_path(data_dir))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Save the API token — tries OS keyring first, falls back to a plaintext file
    /// only when keyring is unavailable (e.g. headless Linux).
    pub fn save_token(token: &str, data_dir: &Path) -> Result<(), std::io::Error> {
        let keyring_ok = keyring::Entry::new("easy_harvest", "harvest_api_token")
            .ok()
            .and_then(|e| e.set_password(token).ok())
            .is_some();
        if keyring_ok {
            // Keyring succeeded — remove any stale plaintext fallback file.
            let path = Self::token_file_path(data_dir);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        } else {
            std::fs::create_dir_all(data_dir)?;
            let path = Self::token_file_path(data_dir);
            std::fs::write(&path, token)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
        }
        Ok(())
    }

    pub fn token_file_path(data_dir: &Path) -> PathBuf {
        data_dir.join("harvest_token")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expected_hours() {
        let s = Settings {
            total_weekly_hours: 41.0,
            work_percentage: 0.8,
            ..Default::default()
        };
        assert!((s.expected_hours_per_day() - 6.56).abs() < 1e-9);
        assert!((s.expected_hours_per_week() - 32.8).abs() < 1e-9);
    }

    #[test]
    fn test_expected_hours_full_time() {
        let s = Settings {
            total_weekly_hours: 41.0,
            work_percentage: 1.0,
            ..Default::default()
        };
        assert!((s.expected_hours_per_day() - 8.2).abs() < 1e-9);
    }

    #[test]
    fn test_effective_holiday_days_full_time() {
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 1.0,
            ..Default::default()
        };
        assert_eq!(s.effective_holiday_days_for(2026), 25.0);
    }

    #[test]
    fn test_effective_holiday_days_part_time() {
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 0.8,
            ..Default::default()
        };
        // Part-time workers get the same number of vacation days.
        assert_eq!(s.effective_holiday_days_for(2026), 25.0);
    }

    #[test]
    fn test_effective_holiday_days_with_carryover() {
        let mut carryover = std::collections::HashMap::new();
        carryover.insert(2026, YearCarryover { holiday_days: 3.5, overtime_hours: 0.0 });
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 0.8,
            carryover,
            ..Default::default()
        };
        assert_eq!(s.effective_holiday_days_for(2026), 28.5);
    }

    #[test]
    fn test_effective_holiday_days_first_year_proration() {
        // Nov 15 is day 319 of 2025. Dec 31 is day 365.
        // days_worked = 365 - 319 + 1 = 47, days_in_year = 365.
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 0.8,
            first_work_day: Some(NaiveDate::from_ymd_opt(2025, 11, 15).unwrap()),
            ..Default::default()
        };
        let expected = 25.0 * 47.0 / 365.0;
        assert!((s.effective_holiday_days_for(2025) - expected).abs() < 1e-9);
    }

    #[test]
    fn test_effective_holiday_days_non_first_year_unaffected() {
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 0.8,
            first_work_day: Some(NaiveDate::from_ymd_opt(2025, 11, 15).unwrap()),
            ..Default::default()
        };
        // Non-first years get full entitlement regardless of work_percentage.
        assert_eq!(s.effective_holiday_days_for(2026), 25.0);
        assert_eq!(s.effective_holiday_days_for(2024), 25.0);
    }

    #[test]
    fn test_effective_holiday_days_first_work_day_jan1_no_proration() {
        let s = Settings {
            total_holiday_days_per_year: 25,
            work_percentage: 1.0,
            first_work_day: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            ..Default::default()
        };
        assert_eq!(s.effective_holiday_days_for(2026), 25.0);
    }

    #[test]
    fn test_public_holiday_credit_full_day() {
        let h = PublicHoliday {
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            name: "Neujahr".into(),
            half_day: false,
        };
        assert_eq!(h.credit_hours(8.2), 8.2);
        assert_eq!(h.credit_hours(6.56), 6.56);
    }

    #[test]
    fn test_public_holiday_credit_half_day() {
        let h = PublicHoliday {
            date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            name: "Tag der Arbeit".into(),
            half_day: true,
        };
        assert_eq!(h.credit_hours(8.2), 4.1);
        assert_eq!(h.credit_hours(6.56), 3.28);
    }

    #[test]
    fn test_swiss_public_holidays_2026() {
        let holidays = swiss_public_holidays(2026);
        assert_eq!(holidays.len(), 14);

        let by_name = |n: &str| holidays.iter().find(|h| h.name == n).unwrap();

        assert_eq!(by_name("Neujahr").date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(by_name("Karfreitag").date, NaiveDate::from_ymd_opt(2026, 4, 3).unwrap());
        assert_eq!(by_name("Ostermontag").date, NaiveDate::from_ymd_opt(2026, 4, 6).unwrap());
        assert_eq!(by_name("Tag der Arbeit").date, NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
        assert!(by_name("Tag der Arbeit").half_day);
        assert_eq!(by_name("Auffahrt").date, NaiveDate::from_ymd_opt(2026, 5, 14).unwrap());
        assert_eq!(by_name("Pfingstmontag").date, NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(by_name("Fronleichnam").date, NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        assert_eq!(by_name("Bundesfeiertag").date, NaiveDate::from_ymd_opt(2026, 8, 1).unwrap());
        assert_eq!(by_name("Mariä Himmelfahrt").date, NaiveDate::from_ymd_opt(2026, 8, 15).unwrap());
        assert_eq!(by_name("Allerheiligen").date, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(by_name("Heiligabend").date, NaiveDate::from_ymd_opt(2026, 12, 24).unwrap());
        assert!(by_name("Heiligabend").half_day);
        assert_eq!(by_name("Weihnachtstag").date, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
        assert_eq!(by_name("Stephanstag").date, NaiveDate::from_ymd_opt(2026, 12, 26).unwrap());
        assert_eq!(by_name("Silvester").date, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        assert!(by_name("Silvester").half_day);
    }

    #[test]
    fn test_swiss_public_holidays_2025_matches_company_calendar() {
        // Validated against official Feiertagskalender 2025 PDF.
        let holidays = swiss_public_holidays(2025);
        let by_name = |n: &str| holidays.iter().find(|h| h.name == n).unwrap();

        assert_eq!(by_name("Neujahr").date, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(by_name("Karfreitag").date, NaiveDate::from_ymd_opt(2025, 4, 18).unwrap());
        assert_eq!(by_name("Ostermontag").date, NaiveDate::from_ymd_opt(2025, 4, 21).unwrap());
        assert_eq!(by_name("Tag der Arbeit").date, NaiveDate::from_ymd_opt(2025, 5, 1).unwrap());
        assert_eq!(by_name("Auffahrt").date, NaiveDate::from_ymd_opt(2025, 5, 29).unwrap());
        assert_eq!(by_name("Pfingstmontag").date, NaiveDate::from_ymd_opt(2025, 6, 9).unwrap());
        assert_eq!(by_name("Fronleichnam").date, NaiveDate::from_ymd_opt(2025, 6, 19).unwrap());
        assert_eq!(by_name("Bundesfeiertag").date, NaiveDate::from_ymd_opt(2025, 8, 1).unwrap());
        assert_eq!(by_name("Mariä Himmelfahrt").date, NaiveDate::from_ymd_opt(2025, 8, 15).unwrap());
        assert_eq!(by_name("Allerheiligen").date, NaiveDate::from_ymd_opt(2025, 11, 1).unwrap());
        assert_eq!(by_name("Heiligabend").date, NaiveDate::from_ymd_opt(2025, 12, 24).unwrap());
        assert_eq!(by_name("Weihnachtstag").date, NaiveDate::from_ymd_opt(2025, 12, 25).unwrap());
        assert_eq!(by_name("Stephanstag").date, NaiveDate::from_ymd_opt(2025, 12, 26).unwrap());
        assert_eq!(by_name("Silvester").date, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

    #[test]
    fn test_swiss_public_holidays_2027() {
        // Easter 2027 is March 28 — an early Easter, good edge-case coverage.
        let holidays = swiss_public_holidays(2027);
        assert_eq!(holidays.len(), 14);

        let by_name = |n: &str| holidays.iter().find(|h| h.name == n).unwrap();

        assert_eq!(by_name("Neujahr").date,          NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        assert_eq!(by_name("Karfreitag").date,       NaiveDate::from_ymd_opt(2027, 3, 26).unwrap());
        assert_eq!(by_name("Ostermontag").date,      NaiveDate::from_ymd_opt(2027, 3, 29).unwrap());
        assert_eq!(by_name("Tag der Arbeit").date,   NaiveDate::from_ymd_opt(2027, 5, 1).unwrap());
        assert!(by_name("Tag der Arbeit").half_day);
        assert_eq!(by_name("Auffahrt").date,         NaiveDate::from_ymd_opt(2027, 5, 6).unwrap());
        assert_eq!(by_name("Pfingstmontag").date,    NaiveDate::from_ymd_opt(2027, 5, 17).unwrap());
        assert_eq!(by_name("Fronleichnam").date,     NaiveDate::from_ymd_opt(2027, 5, 27).unwrap());
        assert_eq!(by_name("Bundesfeiertag").date,   NaiveDate::from_ymd_opt(2027, 8, 1).unwrap());
        assert_eq!(by_name("Mariä Himmelfahrt").date,NaiveDate::from_ymd_opt(2027, 8, 15).unwrap());
        assert_eq!(by_name("Allerheiligen").date,    NaiveDate::from_ymd_opt(2027, 11, 1).unwrap());
        assert_eq!(by_name("Heiligabend").date,      NaiveDate::from_ymd_opt(2027, 12, 24).unwrap());
        assert!(by_name("Heiligabend").half_day);
        assert_eq!(by_name("Weihnachtstag").date,    NaiveDate::from_ymd_opt(2027, 12, 25).unwrap());
        assert_eq!(by_name("Stephanstag").date,      NaiveDate::from_ymd_opt(2027, 12, 26).unwrap());
        assert_eq!(by_name("Silvester").date,        NaiveDate::from_ymd_opt(2027, 12, 31).unwrap());
        assert!(by_name("Silvester").half_day);
    }

    #[test]
    fn test_easter_known_years() {
        // Dates verified against https://www.timeanddate.com/calendar/catholic-easter.html
        let known = [
            (2020,  4, 12),
            (2021,  4,  4),
            (2022,  4, 17),
            (2023,  4,  9),
            (2024,  3, 31),
            (2025,  4, 20),
            (2026,  4,  5),
            (2027,  3, 28),
            (2028,  4, 16),
            (2029,  4,  1),
            (2030,  4, 21),
            (2031,  4, 13),
            (2032,  3, 28),
            (2033,  4, 17),
            (2034,  4,  9),
            (2035,  3, 25),
        ];
        for (year, month, day) in known {
            assert_eq!(
                easter_sunday(year),
                NaiveDate::from_ymd_opt(year, month, day).unwrap(),
                "Easter {year}",
            );
        }
    }

    #[test]
    fn test_swiss_public_holidays_structural_invariants() {
        use chrono::Duration;

        // Run structural checks for every year 2020–2035.
        for year in 2020..=2035 {
            let holidays = swiss_public_holidays(year);
            let by_name = |n: &str| {
                holidays.iter().find(|h| h.name == n)
                    .unwrap_or_else(|| panic!("{n} missing for {year}"))
            };

            // Always 14 holidays.
            assert_eq!(holidays.len(), 14, "wrong count for {year}");

            // All dates must fall within the correct year.
            for h in &holidays {
                assert_eq!(h.date.year(), year, "{} date year wrong for {year}", h.name);
            }

            // Fixed-date holidays are always the same calendar date.
            assert_eq!(by_name("Neujahr").date.month(),          1);
            assert_eq!(by_name("Neujahr").date.day(),            1);
            assert_eq!(by_name("Tag der Arbeit").date.month(),   5);
            assert_eq!(by_name("Tag der Arbeit").date.day(),     1);
            assert_eq!(by_name("Bundesfeiertag").date.month(),   8);
            assert_eq!(by_name("Bundesfeiertag").date.day(),     1);
            assert_eq!(by_name("Mariä Himmelfahrt").date.month(),8);
            assert_eq!(by_name("Mariä Himmelfahrt").date.day(), 15);
            assert_eq!(by_name("Allerheiligen").date.month(),   11);
            assert_eq!(by_name("Allerheiligen").date.day(),      1);
            assert_eq!(by_name("Heiligabend").date.month(),     12);
            assert_eq!(by_name("Heiligabend").date.day(),       24);
            assert_eq!(by_name("Weihnachtstag").date.month(),   12);
            assert_eq!(by_name("Weihnachtstag").date.day(),     25);
            assert_eq!(by_name("Stephanstag").date.month(),     12);
            assert_eq!(by_name("Stephanstag").date.day(),       26);
            assert_eq!(by_name("Silvester").date.month(),       12);
            assert_eq!(by_name("Silvester").date.day(),         31);

            // Easter-derived holidays have the correct offset from Easter Sunday.
            let easter = easter_sunday(year);
            assert_eq!(by_name("Karfreitag").date,   easter - Duration::days(2));
            assert_eq!(by_name("Ostermontag").date,  easter + Duration::days(1));
            assert_eq!(by_name("Auffahrt").date,     easter + Duration::days(39));
            assert_eq!(by_name("Pfingstmontag").date,easter + Duration::days(50));
            assert_eq!(by_name("Fronleichnam").date, easter + Duration::days(60));

            // Half-day flags.
            assert!( by_name("Tag der Arbeit").half_day,  "Tag der Arbeit should be half-day for {year}");
            assert!( by_name("Heiligabend").half_day,     "Heiligabend should be half-day for {year}");
            assert!( by_name("Silvester").half_day,       "Silvester should be half-day for {year}");
            assert!(!by_name("Karfreitag").half_day,      "Karfreitag should be full-day for {year}");
            assert!(!by_name("Ostermontag").half_day,     "Ostermontag should be full-day for {year}");
            assert!(!by_name("Weihnachtstag").half_day,   "Weihnachtstag should be full-day for {year}");

            // Chronological ordering of Easter cluster.
            assert!(by_name("Karfreitag").date < by_name("Ostermontag").date);
            assert!(by_name("Ostermontag").date < by_name("Auffahrt").date);
            assert!(by_name("Auffahrt").date < by_name("Pfingstmontag").date);
            assert!(by_name("Pfingstmontag").date < by_name("Fronleichnam").date);
        }
    }

    #[test]
    fn test_validate_ok() {
        let s = Settings::default();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_percentage() {
        let s = Settings {
            work_percentage: 1.5,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn test_validate_negative_hours() {
        let s = Settings {
            total_weekly_hours: -1.0,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn test_validate_nan() {
        let s = Settings {
            work_percentage: f64::NAN,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut carryover = std::collections::HashMap::new();
        carryover.insert(2026, YearCarryover { holiday_days: 2.0, overtime_hours: 5.5 });
        let settings = Settings {
            account_id: "12345".into(),
            data_dir: dir.path().to_path_buf(),
            total_weekly_hours: 41.0,
            work_percentage: 0.8,
            carryover,
            ..Default::default()
        };
        settings.save().expect("save failed");

        let loaded = Settings::load(dir.path());
        assert_eq!(loaded.account_id, "12345");
        assert_eq!(loaded.work_percentage, 0.8);
        assert_eq!(loaded.overtime_carryover_for(2026), 5.5);
        assert_eq!(loaded.effective_holiday_days_for(2026), 25.0 + 2.0);
        assert_eq!(loaded.data_dir, dir.path());
    }

    #[test]
    fn test_load_missing_new_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path()).unwrap();
        let json = r#"{
            "account_id": "999",
            "default_break_minutes": 60,
            "total_holiday_days_per_year": 25,
            "holiday_task_ids": []
        }"#;
        std::fs::write(dir.path().join("settings.json"), json).unwrap();

        let loaded = Settings::load(dir.path());
        assert_eq!(loaded.account_id, "999");
        assert_eq!(loaded.total_weekly_hours, 41.0);
        assert_eq!(loaded.work_percentage, 1.0);
        assert!(loaded.carryover.is_empty());
    }
}
