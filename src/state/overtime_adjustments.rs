use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvertimeAdjustment {
    pub id: u64,
    pub date: String,
    pub hours: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OvertimeAdjustmentStore {
    pub next_id: u64,
    pub years: HashMap<i32, Vec<OvertimeAdjustment>>,
}

impl OvertimeAdjustmentStore {
    pub fn adjustments_for(&self, year: i32) -> &[OvertimeAdjustment] {
        self.years.get(&year).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn adjustments_for_mut(&mut self, year: i32) -> &mut Vec<OvertimeAdjustment> {
        self.years.entry(year).or_default()
    }

    pub fn adjustments_total(&self, year: i32) -> f64 {
        self.adjustments_for(year).iter().map(|a| a.hours).sum()
    }

    pub fn load(data_dir: &Path) -> Self {
        super::io::load_json(&data_dir.join("overtime_adjustments.json")).unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(data_dir)?;
        let json =
            serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        super::io::atomic_write(&data_dir.join("overtime_adjustments.json"), &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let store = OvertimeAdjustmentStore::default();
        assert_eq!(store.next_id, 0);
        assert!(store.years.is_empty());
        assert!(store.adjustments_for(2026).is_empty());
        assert_eq!(store.adjustments_total(2026), 0.0);
    }

    #[test]
    fn adjustments_total_sums_correctly() {
        let mut store = OvertimeAdjustmentStore::default();
        let adjs = store.adjustments_for_mut(2026);
        adjs.push(OvertimeAdjustment {
            id: 1, date: "2026-03-01".into(), hours: 8.0, reason: "Bonus".into(),
        });
        adjs.push(OvertimeAdjustment {
            id: 2, date: "2026-06-01".into(), hours: -4.5, reason: "Payout".into(),
        });
        assert!((store.adjustments_total(2026) - 3.5).abs() < f64::EPSILON);
        assert_eq!(store.adjustments_total(2025), 0.0);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = OvertimeAdjustmentStore { next_id: 3, years: HashMap::new() };
        store.adjustments_for_mut(2026).push(OvertimeAdjustment {
            id: 1, date: "2026-01-15".into(), hours: -10.0, reason: "Hours payout".into(),
        });
        store.save(dir.path()).unwrap();

        let loaded = OvertimeAdjustmentStore::load(dir.path());
        assert_eq!(loaded.next_id, 3);
        assert_eq!(loaded.adjustments_for(2026).len(), 1);
        assert_eq!(loaded.adjustments_for(2026)[0].reason, "Hours payout");
    }
}
