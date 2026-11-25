use std::path::Path;

use serde::{Deserialize, Serialize};

/// Identifies a card on the Projects page: a Harvest-managed project or a
/// manual budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum CardKey {
    Harvest(i64),
    Manual(u64),
}

/// Persisted card order and hidden set for the Projects page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTrackingLayout {
    #[serde(default)]
    pub order: Vec<CardKey>,
    #[serde(default)]
    pub hidden: Vec<CardKey>,
}

const FILE_NAME: &str = "project_tracking_layout.json";

impl ProjectTrackingLayout {
    pub fn load(data_dir: &Path) -> Self {
        super::io::try_load_json::<Self>(&data_dir.join(FILE_NAME)).unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        super::io::atomic_write(&data_dir.join(FILE_NAME), &json)
    }

    /// Present keys in stored order, followed by the remaining present keys
    /// sorted by name (case-insensitive).
    pub fn ordered(&self, present: &[(CardKey, String)]) -> Vec<CardKey> {
        let mut result: Vec<CardKey> = Vec::with_capacity(present.len());
        for key in &self.order {
            if present.iter().any(|(k, _)| k == key) && !result.contains(key) {
                result.push(*key);
            }
        }
        let mut rest: Vec<&(CardKey, String)> = present
            .iter()
            .filter(|(k, _)| !result.contains(k))
            .collect();
        rest.sort_by_key(|(_, name)| name.to_lowercase());
        result.extend(rest.into_iter().map(|(k, _)| *k));
        result
    }

    /// Drops keys not in `present` from `order` and `hidden`.
    pub fn prune(&mut self, present: &[CardKey]) {
        self.order.retain(|k| present.contains(k));
        self.hidden.retain(|k| present.contains(k));
    }

    /// Sets `order = current`, then moves the card at `from` to insertion slot
    /// `to` (0..=len, measured on the list before removal).
    pub fn move_card(&mut self, current: &[CardKey], from: usize, to: usize) {
        self.order = current.to_vec();
        if from >= self.order.len() || to > self.order.len() || to == from || to == from + 1 {
            return;
        }
        let key = self.order.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        self.order.insert(insert_at, key);
    }

    pub fn set_hidden(&mut self, key: CardKey, hidden: bool) {
        if hidden {
            if !self.hidden.contains(&key) {
                self.hidden.push(key);
            }
        } else {
            self.hidden.retain(|k| *k != key);
        }
    }

    pub fn is_hidden(&self, key: CardKey) -> bool {
        self.hidden.contains(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CardKey::{Harvest as H, Manual as M};

    fn named(items: &[(CardKey, &str)]) -> Vec<(CardKey, String)> {
        items.iter().map(|(k, n)| (*k, n.to_string())).collect()
    }

    #[test]
    fn new_cards_append_sorted() {
        let layout = ProjectTrackingLayout {
            order: vec![M(2), H(1)],
            hidden: vec![],
        };
        let present = named(&[
            (H(1), "zeta"),
            (M(2), "Alpha"),
            (H(3), "beta"),
            (M(4), "Apple"),
        ]);
        assert_eq!(layout.ordered(&present), vec![M(2), H(1), M(4), H(3)]);
    }

    #[test]
    fn stale_keys_ignored_in_ordered() {
        let layout = ProjectTrackingLayout {
            order: vec![H(9), M(2), H(1)],
            hidden: vec![],
        };
        let present = named(&[(H(1), "a"), (M(2), "b")]);
        assert_eq!(layout.ordered(&present), vec![M(2), H(1)]);
    }

    #[test]
    fn prune_removes_stale() {
        let mut layout = ProjectTrackingLayout {
            order: vec![H(9), M(2), H(1)],
            hidden: vec![H(9), M(2)],
        };
        layout.prune(&[M(2), H(1)]);
        assert_eq!(layout.order, vec![M(2), H(1)]);
        assert_eq!(layout.hidden, vec![M(2)]);
    }

    #[test]
    fn move_card_down() {
        let mut layout = ProjectTrackingLayout::default();
        layout.move_card(&[H(1), H(2), H(3)], 0, 3);
        assert_eq!(layout.order, vec![H(2), H(3), H(1)]);
    }

    #[test]
    fn move_card_up() {
        let mut layout = ProjectTrackingLayout::default();
        layout.move_card(&[H(1), H(2), H(3)], 2, 0);
        assert_eq!(layout.order, vec![H(3), H(1), H(2)]);
    }

    #[test]
    fn move_card_noop_same_slot() {
        let mut layout = ProjectTrackingLayout::default();
        layout.move_card(&[H(1), H(2), H(3)], 1, 1);
        assert_eq!(layout.order, vec![H(1), H(2), H(3)]);
        layout.move_card(&[H(1), H(2), H(3)], 1, 2);
        assert_eq!(layout.order, vec![H(1), H(2), H(3)]);
    }

    #[test]
    fn hide_unhide_roundtrip() {
        let mut layout = ProjectTrackingLayout::default();
        assert!(!layout.is_hidden(H(1)));
        layout.set_hidden(H(1), true);
        layout.set_hidden(H(1), true);
        assert_eq!(layout.hidden, vec![H(1)]);
        assert!(layout.is_hidden(H(1)));
        assert!(!layout.is_hidden(M(1)));
        layout.set_hidden(H(1), false);
        assert!(!layout.is_hidden(H(1)));
        assert!(layout.hidden.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectTrackingLayout {
            order: vec![M(2), H(1)],
            hidden: vec![H(1)],
        };
        layout.save(dir.path()).unwrap();
        assert_eq!(ProjectTrackingLayout::load(dir.path()), layout);
    }

    #[test]
    fn corrupt_file_loads_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "{not json").unwrap();
        assert_eq!(
            ProjectTrackingLayout::load(dir.path()),
            ProjectTrackingLayout::default()
        );
    }
}
