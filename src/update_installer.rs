use serde_json::Value;

// ── Update assets ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAssets {
    pub binary_url: String,
    pub checksum_url: String,
}

fn platform_asset_name() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        Some("easy_harvest-linux-x86_64")
    } else if cfg!(target_os = "windows") {
        Some("easy_harvest-windows-x86_64.exe")
    } else {
        None
    }
}

fn asset_url(assets: &[Value], name: &str) -> Option<String> {
    assets
        .iter()
        .find(|a| a.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|a| a.get("browser_download_url"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Matches a release's `assets[]` JSON against an explicit binary name,
/// rather than reading it from `platform_asset_name` itself, so it's
/// unit-testable for every platform regardless of which OS the tests
/// happen to run on.
fn find_update_assets_for(assets: &[Value], binary_name: &str) -> Option<UpdateAssets> {
    let checksum_name = format!("{binary_name}.sha256");
    Some(UpdateAssets {
        binary_url: asset_url(assets, binary_name)?,
        checksum_url: asset_url(assets, &checksum_name)?,
    })
}

/// Find this platform's binary + `.sha256` sidecar in a release's `assets[]`
/// array. Returns `None` on platforms with no self-update binary (macOS).
pub fn find_update_assets(assets: &[Value]) -> Option<UpdateAssets> {
    find_update_assets_for(assets, platform_asset_name()?)
}

// ── Checksum verification ────────────────────────────────────────────────────

pub fn verify_checksum(bytes: &[u8], expected_hex: &str) -> bool {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let actual = digest.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    actual.eq_ignore_ascii_case(expected_hex.trim())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_assets() -> Vec<Value> {
        vec![
            json!({"name": "easy_harvest-linux-x86_64", "browser_download_url": "https://example.com/linux-bin"}),
            json!({"name": "easy_harvest-linux-x86_64.sha256", "browser_download_url": "https://example.com/linux-bin.sha256"}),
            json!({"name": "easy_harvest-windows-x86_64.exe", "browser_download_url": "https://example.com/win-bin"}),
            json!({"name": "easy_harvest-windows-x86_64.exe.sha256", "browser_download_url": "https://example.com/win-bin.sha256"}),
            json!({"name": "easy_harvest-macos-aarch64.zip", "browser_download_url": "https://example.com/mac-zip"}),
        ]
    }

    #[test]
    fn finds_linux_binary_and_checksum() {
        let result = find_update_assets_for(&sample_assets(), "easy_harvest-linux-x86_64");
        assert_eq!(
            result,
            Some(UpdateAssets {
                binary_url: "https://example.com/linux-bin".into(),
                checksum_url: "https://example.com/linux-bin.sha256".into(),
            })
        );
    }

    #[test]
    fn finds_windows_binary_and_checksum() {
        let result = find_update_assets_for(&sample_assets(), "easy_harvest-windows-x86_64.exe");
        assert_eq!(
            result,
            Some(UpdateAssets {
                binary_url: "https://example.com/win-bin".into(),
                checksum_url: "https://example.com/win-bin.sha256".into(),
            })
        );
    }

    #[test]
    fn missing_checksum_sidecar_returns_none() {
        let assets = vec![json!({
            "name": "easy_harvest-linux-x86_64",
            "browser_download_url": "https://example.com/linux-bin"
        })];
        assert_eq!(find_update_assets_for(&assets, "easy_harvest-linux-x86_64"), None);
    }

    #[test]
    fn missing_binary_returns_none() {
        let assets = vec![json!({
            "name": "easy_harvest-linux-x86_64.sha256",
            "browser_download_url": "https://example.com/linux-bin.sha256"
        })];
        assert_eq!(find_update_assets_for(&assets, "easy_harvest-linux-x86_64"), None);
    }

    #[test]
    fn verify_checksum_accepts_matching_hash() {
        use sha2::{Digest, Sha256};
        let bytes = b"hello world";
        let digest = Sha256::digest(bytes);
        let expected = digest.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        assert!(verify_checksum(bytes, &expected));
    }

    #[test]
    fn verify_checksum_is_case_insensitive_and_trims_whitespace() {
        use sha2::{Digest, Sha256};
        let bytes = b"hello world";
        let digest = Sha256::digest(bytes);
        let hex = digest.iter().map(|b| format!("{:02X}", b)).collect::<String>();
        let expected_upper_with_newline = format!("{}\n", hex);
        assert!(verify_checksum(bytes, &expected_upper_with_newline));
    }

    #[test]
    fn verify_checksum_rejects_mismatch() {
        let bogus = "0".repeat(64);
        assert!(!verify_checksum(b"hello world", &bogus));
    }
}
