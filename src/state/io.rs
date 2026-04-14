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
