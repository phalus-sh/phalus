/// Scan result persistence: save and load `ScanResult` as JSON files
/// under `~/.phalus/scans/`.
use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::debug;

use crate::ScanResult;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Returns `~/.phalus/scans/` (creates it if absent).
pub fn scans_dir() -> Result<PathBuf> {
    let base = dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".phalus")
        .join("scans");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("cannot create scans directory: {}", base.display()))?;
    Ok(base)
}

fn scan_path(id: &str) -> Result<PathBuf> {
    Ok(scans_dir()?.join(format!("{}.json", id)))
}

// ---------------------------------------------------------------------------
// Save / load
// ---------------------------------------------------------------------------

/// Persist a `ScanResult` to `~/.phalus/scans/{id}.json`.
pub fn save(result: &ScanResult) -> Result<PathBuf> {
    let path = scan_path(&result.id)?;
    let json = serde_json::to_string_pretty(result).context("failed to serialize scan result")?;
    std::fs::write(&path, json)
        .with_context(|| format!("failed to write scan result: {}", path.display()))?;
    debug!("Saved scan {} to {}", result.id, path.display());
    Ok(path)
}

/// Load a `ScanResult` by its ID.
pub fn load(id: &str) -> Result<ScanResult> {
    let path = scan_path(id)?;
    let json = std::fs::read_to_string(&path).with_context(|| format!("scan {} not found", id))?;
    serde_json::from_str(&json).with_context(|| format!("invalid scan data for {}", id))
}

/// List all stored scan results, sorted by `scanned_at` descending.
pub fn list_all() -> Result<Vec<ScanResult>> {
    let dir = scans_dir()?;
    let mut results: Vec<ScanResult> = Vec::new();

    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("cannot read scans directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<ScanResult>(&json) {
                Ok(r) => results.push(r),
                Err(e) => {
                    tracing::warn!("Skipping corrupt scan file {}: {}", path.display(), e);
                }
            },
            Err(e) => {
                tracing::warn!("Cannot read {}: {}", path.display(), e);
            }
        }
    }

    results.sort_by(|a, b| b.scanned_at.cmp(&a.scanned_at));
    Ok(results)
}

/// Delete a stored scan by ID. Returns `true` if it existed.
pub fn delete(id: &str) -> Result<bool> {
    let path = scan_path(id)?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("failed to delete scan {}", id))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Delete all stored scans. Returns the number deleted.
pub fn delete_all() -> Result<usize> {
    let dir = scans_dir()?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            std::fs::remove_file(entry.path())?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ecosystem, LicenseClass, ScanResult, ScannedPackage};
    use chrono::Utc;

    fn make_result(id: &str) -> ScanResult {
        ScanResult {
            id: id.to_string(),
            path: "/tmp/test".to_string(),
            scanned_at: Utc::now(),
            packages: vec![ScannedPackage {
                name: "lodash".to_string(),
                version: "4.17.21".to_string(),
                ecosystem: Ecosystem::Npm,
                raw_license: Some("MIT".to_string()),
                spdx_license: Some("MIT".to_string()),
                classification: LicenseClass::Permissive,
                source: "manifest".to_string(),
            }],
            manifest_files: vec!["/tmp/test/package.json".to_string()],
            sbom_files: vec![],
        }
    }

    #[test]
    fn round_trip_save_load() {
        let result = make_result("store-test-round-trip");
        save(&result).unwrap();
        let loaded = load("store-test-round-trip").unwrap();
        assert_eq!(loaded.id, result.id);
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages[0].spdx_license.as_deref(), Some("MIT"));
        // Cleanup
        delete("store-test-round-trip").unwrap();
    }

    #[test]
    fn load_missing_returns_err() {
        assert!(load("nonexistent-scan-id-xyz").is_err());
    }

    #[test]
    fn delete_existing() {
        let result = make_result("store-test-delete");
        save(&result).unwrap();
        assert!(delete("store-test-delete").unwrap());
        assert!(!delete("store-test-delete").unwrap());
    }
}
