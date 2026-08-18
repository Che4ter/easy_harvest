use std::collections::HashMap;
use std::path::{Path, PathBuf};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::overtime_adjustments::OvertimeAdjustmentStore;
use super::settings::{effective_holiday_days, YearCarryover};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub harvest_user_id: i64,
    pub display_name: String,
    pub expected_hours_per_day: f64,
    pub total_holiday_days_per_year: u32,
    #[serde(default)]
    pub holiday_task_ids: Vec<i64>,
    #[serde(default)]
    pub first_work_day: Option<NaiveDate>,
    #[serde(default)]
    pub carryover: HashMap<i32, YearCarryover>,
    #[serde(default)]
    pub overtime_adjustments: OvertimeAdjustmentStore,
}

impl TeamMember {
    pub fn effective_holiday_days_for(&self, year: i32) -> f64 {
        effective_holiday_days(
            self.total_holiday_days_per_year,
            self.first_work_day,
            &self.carryover,
            self.expected_hours_per_day,
            year,
        )
    }

    pub fn overtime_carryover_for(&self, year: i32) -> f64 {
        self.carryover.get(&year).map(|c| c.overtime_hours).unwrap_or(0.0)
    }

    /// True when no carryover entry exists for `year` — callers use this to
    /// show a warning indicator, since stats still compute (falling back to
    /// 0.0 carryover) but may understate the member's true balance.
    pub fn missing_carryover_for(&self, year: i32) -> bool {
        !self.carryover.contains_key(&year)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamSettings {
    pub members: Vec<TeamMember>,
}

impl TeamSettings {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("team_settings.json")
    }

    pub fn load(data_dir: &Path) -> Self {
        super::io::load_json(&Self::path(data_dir)).unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        super::io::atomic_write(&Self::path(data_dir), &json)
    }

    pub fn member(&self, harvest_user_id: i64) -> Option<&TeamMember> {
        self.members.iter().find(|m| m.harvest_user_id == harvest_user_id)
    }

    pub fn member_mut(&mut self, harvest_user_id: i64) -> Option<&mut TeamMember> {
        self.members.iter_mut().find(|m| m.harvest_user_id == harvest_user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: i64) -> TeamMember {
        TeamMember {
            harvest_user_id: id,
            display_name: "Alex".into(),
            expected_hours_per_day: 8.2,
            total_holiday_days_per_year: 25,
            holiday_task_ids: vec![],
            first_work_day: None,
            carryover: HashMap::new(),
            overtime_adjustments: OvertimeAdjustmentStore::default(),
        }
    }

    #[test]
    fn missing_carryover_true_when_absent() {
        let m = member(1);
        assert!(m.missing_carryover_for(2026));
    }

    #[test]
    fn missing_carryover_false_when_present() {
        let mut m = member(1);
        m.carryover.insert(2026, YearCarryover::default());
        assert!(!m.missing_carryover_for(2026));
    }

    #[test]
    fn overtime_carryover_for_defaults_to_zero() {
        let m = member(1);
        assert_eq!(m.overtime_carryover_for(2026), 0.0);
    }

    #[test]
    fn member_and_member_mut_lookup_by_id() {
        let mut ts = TeamSettings { members: vec![member(1), member(2)] };
        assert_eq!(ts.member(2).unwrap().harvest_user_id, 2);
        assert!(ts.member(999).is_none());
        ts.member_mut(1).unwrap().display_name = "Renamed".into();
        assert_eq!(ts.member(1).unwrap().display_name, "Renamed");
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut ts = TeamSettings::default();
        let mut m = member(42);
        m.display_name = "Gian Luca's Report".into();
        m.carryover.insert(2026, YearCarryover { overtime_hours: 3.5, ..Default::default() });
        ts.members.push(m);
        ts.save(dir.path()).unwrap();

        let loaded = TeamSettings::load(dir.path());
        assert_eq!(loaded.members.len(), 1);
        assert_eq!(loaded.member(42).unwrap().display_name, "Gian Luca's Report");
        assert_eq!(loaded.member(42).unwrap().overtime_carryover_for(2026), 3.5);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let ts = TeamSettings::load(dir.path());
        assert!(ts.members.is_empty());
    }
}
