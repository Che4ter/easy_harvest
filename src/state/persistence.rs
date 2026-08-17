use std::{collections::BTreeMap, path::Path};

use chrono::NaiveDate;

use super::work_day::WorkDay;

/// Persistent store for all [`WorkDay`] records in one calendar month.
///
/// Stored at `<data_dir>/work_days/YYYY-MM.json` as a JSON object mapping
/// ISO date strings to [`WorkDay`] values.
///
/// # Read-modify-write
///
/// To safely update a day (including across OneDrive-synced laptops) always
/// use the read-modify-write pattern:
///
/// ```ignore
/// let mut store = WorkDayStore::load(&data_dir, 2026, 4);
/// let mut today = store.get_or_default(date);
/// today.start(now);
/// store.set(today);
/// store.save(&data_dir)?;
/// ```
#[derive(Debug)]
pub struct WorkDayStore {
    pub year: i32,
    pub month: u32,
    days: BTreeMap<NaiveDate, WorkDay>,
    /// Set when the last load hit a real read error (not just "file
    /// missing") — e.g. an OneDrive sharing-violation lock on a file that
    /// exists and is intact. `save` refuses to write while this is set, so
    /// a transient read glitch can never silently overwrite this month's
    /// real data with an empty store.
    load_failed: bool,
}

impl WorkDayStore {
    fn path(data_dir: &Path, year: i32, month: u32) -> std::path::PathBuf {
        data_dir
            .join("work_days")
            .join(format!("{year}-{month:02}.json"))
    }

    /// Load the month's store from disk.  Returns an empty store when the
    /// file doesn't exist yet or is corrupt, so the caller never needs to
    /// handle a cold-start case specially. A real read error (as opposed to
    /// "file missing") instead marks the store so [`Self::save`] refuses to
    /// write, rather than risk overwriting real data with this incomplete
    /// in-memory copy.
    pub fn load(data_dir: &Path, year: i32, month: u32) -> Self {
        let path = Self::path(data_dir, year, month);
        let (days, load_failed) = match super::io::load_json_checked(&path) {
            Ok(days) => (days.unwrap_or_default(), false),
            Err(e) => {
                eprintln!(
                    "Warning: failed to read {}: {e} — refusing to save this month until it can be read again",
                    path.display()
                );
                (BTreeMap::new(), true)
            }
        };
        Self { year, month, days, load_failed }
    }

    /// Persist the store to disk (creates parent directories as needed).
    ///
    /// Returns an error without writing if the load that produced this
    /// store failed to read the existing file (see [`Self::load`]) —
    /// writing here would permanently replace the real, unread file with
    /// this store's incomplete in-memory contents.
    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        if self.load_failed {
            return Err(std::io::Error::other(format!(
                "not saving {}-{:02}: the existing file could not be read earlier in this session",
                self.year, self.month
            )));
        }
        let path = Self::path(data_dir, self.year, self.month);
        std::fs::create_dir_all(path.parent().expect("path has parent"))?;
        let json = serde_json::to_string_pretty(&self.days)
            .map_err(std::io::Error::other)?;
        super::io::atomic_write(&path, &json)
    }

    /// Return the record for `date`, or `None` if no entry exists yet.
    pub fn get(&self, date: NaiveDate) -> Option<&WorkDay> {
        self.days.get(&date)
    }

    /// Return the record for `date`, or a fresh [`WorkDay`] if absent.
    pub fn get_or_default(&self, date: NaiveDate) -> WorkDay {
        self.days
            .get(&date)
            .cloned()
            .unwrap_or_else(|| WorkDay::new(date))
    }

    /// Insert or replace the record for `day.date`.
    pub fn set(&mut self, day: WorkDay) {
        self.days.insert(day.date, day);
    }

    /// All stored days in ascending date order.
    pub fn all_days(&self) -> impl Iterator<Item = &WorkDay> {
        self.days.values()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, d).unwrap()
    }

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn test_get_or_default_missing() {
        let store = WorkDayStore::load(std::path::Path::new("/nonexistent"), 2026, 4);
        let day = store.get_or_default(date(10));
        assert_eq!(day.date, date(10));
        assert!(day.start_time.is_none());
    }

    #[test]
    fn test_set_and_get() {
        let mut store = WorkDayStore::load(std::path::Path::new("/nonexistent"), 2026, 4);
        let mut day = store.get_or_default(date(10));
        day.start(t(9, 0));
        store.set(day);

        let retrieved = store.get(date(10)).unwrap();
        assert_eq!(retrieved.start_time, Some(t(9, 0)));
    }

    #[test]
    fn test_set_overwrites_existing() {
        let mut store = WorkDayStore::load(std::path::Path::new("/nonexistent"), 2026, 4);
        let mut day = store.get_or_default(date(10));
        day.start(t(9, 0));
        store.set(day);

        let mut updated = store.get_or_default(date(10));
        updated.end(t(17, 0));
        store.set(updated);

        let final_day = store.get(date(10)).unwrap();
        assert_eq!(final_day.end_time, Some(t(17, 0)));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut store = WorkDayStore::load(dir.path(), 2026, 4);

        let mut day1 = store.get_or_default(date(7));
        day1.start(t(9, 0));
        day1.end(t(17, 0));
        store.set(day1);

        let mut day2 = store.get_or_default(date(8));
        day2.start(t(8, 30));
        store.set(day2);

        store.save(dir.path()).expect("save failed");

        let loaded = WorkDayStore::load(dir.path(), 2026, 4);
        let d7 = loaded.get(date(7)).unwrap();
        assert_eq!(d7.start_time, Some(t(9, 0)));
        assert_eq!(d7.end_time, Some(t(17, 0)));

        let d8 = loaded.get(date(8)).unwrap();
        assert_eq!(d8.start_time, Some(t(8, 30)));
        assert!(d8.end_time.is_none());
    }

    #[test]
    fn test_all_days_sorted() {
        let mut store = WorkDayStore::load(std::path::Path::new("/nonexistent"), 2026, 4);
        store.set(WorkDay::new(date(10)));
        store.set(WorkDay::new(date(7)));
        store.set(WorkDay::new(date(14)));

        let dates: Vec<NaiveDate> = store.all_days().map(|d| d.date).collect();
        assert_eq!(dates, vec![date(7), date(10), date(14)]);
    }
}
