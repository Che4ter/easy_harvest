use std::io::Write;
use std::path::Path;

/// Write `data` to `path` atomically: write to a temp file, fsync, then rename.
/// This prevents data loss if the process is killed mid-write.
pub fn atomic_write(path: &Path, data: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(data.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Try to load and parse JSON from `path`.
///
/// On parse failure (file exists but JSON is corrupt), backs up the corrupt file
/// to `<path>.corrupt` and logs a warning via `eprintln!`.  Returns `None` so
/// the caller can fall back to defaults.
pub fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None, // file doesn't exist — not an error
    };
    match serde_json::from_str(&contents) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!(
                "Warning: corrupt JSON in {}, backing up to .corrupt: {e}",
                path.display()
            );
            let backup = path.with_extension("json.corrupt");
            let _ = std::fs::copy(path, &backup);
            None
        }
    }
}

/// Like [`load_json`], but returns `None` silently on parse failure without
/// logging or backing up.  Use this for the *first* attempt in migration
/// scenarios where a different schema will be tried next.
pub fn try_load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Dummy {
        value: i32,
    }

    #[test]
    fn atomic_write_creates_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        atomic_write(&path, r#"{"value":42}"#).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, r#"{"value":42}"#);
    }

    #[test]
    fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        atomic_write(&path, r#"{"value":1}"#).unwrap();
        atomic_write(&path, r#"{"value":2}"#).unwrap();
        let d: Dummy = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(d.value, 2);
    }

    #[test]
    fn load_json_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result: Option<Dummy> = load_json(&dir.path().join("missing.json"));
        assert!(result.is_none());
    }

    #[test]
    fn load_json_parses_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        std::fs::write(&path, r#"{"value":7}"#).unwrap();
        let d: Dummy = load_json(&path).unwrap();
        assert_eq!(d.value, 7);
    }

    #[test]
    fn load_json_corrupt_returns_none_and_creates_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        std::fs::write(&path, b"this is not json").unwrap();

        let result: Option<Dummy> = load_json(&path);
        assert!(result.is_none());

        // The corrupt original must have been backed up.
        let backup = path.with_extension("json.corrupt");
        assert!(backup.exists(), "backup file should have been created");
        let backup_contents = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_contents, "this is not json");
    }

    #[test]
    fn try_load_json_corrupt_returns_none_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        std::fs::write(&path, b"not valid json at all").unwrap();

        let result: Option<Dummy> = try_load_json(&path);
        assert!(result.is_none());

        // try_load_json must NOT create a backup file.
        let backup = path.with_extension("json.corrupt");
        assert!(!backup.exists(), "try_load_json must not create a backup");
    }

    #[test]
    fn try_load_json_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result: Option<Dummy> = try_load_json(&dir.path().join("gone.json"));
        assert!(result.is_none());
    }
}
