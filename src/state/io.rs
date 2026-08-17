use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Write `data` to `path` atomically: write to a temp file, fsync, then rename.
/// This prevents data loss if the process is killed mid-write.
pub fn atomic_write(path: &Path, data: &str) -> std::io::Result<()> {
    // Give the temp file a counter-based unique suffix so two concurrent
    // writers targeting the same `path` (e.g. an auto-save racing a manual
    // sync, both as separate async tasks in this process) never share one
    // temp file and clobber each other's in-flight write.
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("{n}.tmp"));
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(data.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Try to load and parse JSON from `path`, treating any read error
/// (including a real I/O failure, not just a missing file) the same as
/// "no data yet". Use [`load_json_checked`] instead when the caller needs
/// to tell those two cases apart before doing a read-modify-write.
///
/// On parse failure (file exists but JSON is corrupt), backs up the corrupt file
/// to `<path>.corrupt` and logs a warning via `eprintln!`.  Returns `None` so
/// the caller can fall back to defaults.
pub fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    match load_json_checked(path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Warning: failed to read {}, treating as missing: {e}",
                path.display()
            );
            None
        }
    }
}

/// Load and parse JSON from `path`, distinguishing "file genuinely does not
/// exist" (`Ok(None)`) from a real read error such as a permission problem
/// or a sharing-violation lock (`Err`) — e.g. from OneDrive mid-sync on a
/// file that exists and is intact. A caller that then does a
/// read-modify-write (like [`super::persistence::WorkDayStore`]) must not
/// treat `Err` the same as "missing", or a transient lock would turn into
/// silently overwriting real data with an empty store on the next save.
///
/// Corrupt JSON is still treated as `Ok(None)` (after backing up to
/// `<path>.corrupt`), since that is a genuine "this file is unusable, fall
/// back to defaults" case rather than a transient read failure.
pub fn load_json_checked<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> std::io::Result<Option<T>> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    match serde_json::from_str(&contents) {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            eprintln!(
                "Warning: corrupt JSON in {}, backing up to .corrupt: {e}",
                path.display()
            );
            let backup = path.with_extension("json.corrupt");
            if let Err(backup_err) = std::fs::copy(path, &backup) {
                eprintln!(
                    "Warning: failed to back up corrupt file {} to {}: {backup_err}",
                    path.display(),
                    backup.display()
                );
            }
            Ok(None)
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
