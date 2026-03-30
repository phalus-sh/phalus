/// Core scanning logic: walk a directory tree, collect packages from manifests
/// and SBOM files, resolve licenses via registries, and return a `ScanResult`.
use std::path::Path;

use anyhow::{Context, Result};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::license;
use crate::manifest;
use crate::registry::{
    crates::CratesResolver, golang::GoResolver, npm::NpmResolver, pypi::PypiResolver,
};
use crate::sbom;
use crate::{Ecosystem, LicenseClass, ScanResult, ScannedPackage};

/// Options for a scan run.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Maximum concurrent registry lookups.
    pub concurrency: usize,
    /// When true, skip registry lookups and rely only on manifest / SBOM data.
    pub offline: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            concurrency: 8,
            offline: false,
        }
    }
}

/// Walk `path` (directory or single manifest/SBOM file), collect all packages,
/// resolve their licenses, and return a `ScanResult`.
pub async fn run_scan(path: &Path, opts: ScanOptions) -> Result<ScanResult> {
    let abs_path = path
        .canonicalize()
        .with_context(|| format!("cannot access path: {}", path.display()))?;

    let mut manifest_files: Vec<String> = Vec::new();
    let mut sbom_files: Vec<String> = Vec::new();
    let mut package_refs: Vec<(crate::PackageRef, String /* source file */)> = Vec::new();
    let mut sbom_packages: Vec<ScannedPackage> = Vec::new();

    // ---------- Walk the directory ----------
    if abs_path.is_dir() {
        collect_files(
            &abs_path,
            &mut manifest_files,
            &mut sbom_files,
            &mut package_refs,
            &mut sbom_packages,
        )?;
    } else {
        // Single file — determine kind
        let file_name = abs_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if sbom::is_sbom_filename(file_name) {
            let content = std::fs::read_to_string(&abs_path)?;
            match sbom::parse_sbom(&content) {
                Ok(pkgs) => {
                    sbom_files.push(abs_path.display().to_string());
                    sbom_packages.extend(pkgs);
                }
                Err(e) => warn!("Could not parse SBOM {}: {}", abs_path.display(), e),
            }
        } else {
            match manifest::parse_manifest(&abs_path) {
                Ok(parsed) => {
                    manifest_files.push(abs_path.display().to_string());
                    for pkg in parsed.packages {
                        package_refs.push((pkg, abs_path.display().to_string()));
                    }
                }
                Err(e) => warn!("Could not parse manifest {}: {}", abs_path.display(), e),
            }
        }
    }

    // ---------- Resolve licenses from registries ----------
    let mut registry_packages: Vec<ScannedPackage> = if opts.offline {
        // Offline: use package refs as-is with unknown license
        package_refs
            .into_iter()
            .map(|(pkg, _src)| ScannedPackage {
                name: pkg.name,
                version: pkg.version_constraint,
                ecosystem: pkg.ecosystem,
                raw_license: None,
                spdx_license: None,
                classification: LicenseClass::Unknown,
                source: "manifest".to_string(),
            })
            .collect()
    } else {
        resolve_licenses(package_refs, opts.concurrency).await
    };

    // Combine manifest-resolved packages and SBOM packages
    registry_packages.extend(sbom_packages);

    Ok(ScanResult {
        id: Uuid::new_v4().to_string(),
        path: abs_path.display().to_string(),
        scanned_at: chrono::Utc::now(),
        packages: registry_packages,
        manifest_files,
        sbom_files,
    })
}

// ---------------------------------------------------------------------------
// Directory walker
// ---------------------------------------------------------------------------

fn collect_files(
    dir: &Path,
    manifest_files: &mut Vec<String>,
    sbom_files: &mut Vec<String>,
    package_refs: &mut Vec<(crate::PackageRef, String)>,
    sbom_packages: &mut Vec<ScannedPackage>,
) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("cannot read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden directories and common non-source dirs
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with('.') || SKIP_DIRS.contains(&file_name) {
            continue;
        }

        if path.is_dir() {
            collect_files(
                &path,
                manifest_files,
                sbom_files,
                package_refs,
                sbom_packages,
            )?;
            continue;
        }

        // SBOM files take priority
        if sbom::is_sbom_filename(file_name) {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Cannot read {}: {}", path.display(), e);
                    continue;
                }
            };
            match sbom::parse_sbom(&content) {
                Ok(pkgs) => {
                    debug!("SBOM {}: {} packages", path.display(), pkgs.len());
                    sbom_files.push(path.display().to_string());
                    sbom_packages.extend(pkgs);
                }
                Err(e) => warn!("Cannot parse SBOM {}: {}", path.display(), e),
            }
            continue;
        }

        // Manifest files
        if is_manifest_filename(file_name) {
            match manifest::parse_manifest(&path) {
                Ok(parsed) => {
                    debug!(
                        "Manifest {}: {} packages",
                        path.display(),
                        parsed.packages.len()
                    );
                    manifest_files.push(path.display().to_string());
                    let src = path.display().to_string();
                    for pkg in parsed.packages {
                        package_refs.push((pkg, src.clone()));
                    }
                }
                Err(e) => warn!("Cannot parse manifest {}: {}", path.display(), e),
            }
        }
    }

    Ok(())
}

/// Filenames that identify a package manifest.
fn is_manifest_filename(name: &str) -> bool {
    matches!(
        name,
        "package.json" | "requirements.txt" | "Cargo.toml" | "go.mod"
    )
}

/// Top-level directories to skip entirely (not recursed into).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
    "target", // Rust build artifacts
    ".cargo",
    "vendor",
    "venv",
    ".venv",
    "env",
];

// ---------------------------------------------------------------------------
// Registry license resolution
// ---------------------------------------------------------------------------

async fn resolve_licenses(
    refs: Vec<(crate::PackageRef, String)>,
    concurrency: usize,
) -> Vec<ScannedPackage> {
    let sem = std::sync::Arc::new(Semaphore::new(concurrency));
    let mut set: JoinSet<ScannedPackage> = JoinSet::new();

    for (pkg, src) in refs {
        let permit = sem.clone().acquire_owned().await.unwrap();
        set.spawn(async move {
            let result = fetch_license(&pkg).await;
            drop(permit);
            let (raw_license, spdx_license, classification, source) = match result {
                Ok(Some(raw)) => {
                    let (spdx, class) = license::normalize_and_classify(&raw);
                    (Some(raw), Some(spdx), class, src)
                }
                Ok(None) => (None, None, LicenseClass::Unknown, src),
                Err(e) => {
                    debug!(
                        "Registry lookup failed for {}@{}: {}",
                        pkg.name, pkg.version_constraint, e
                    );
                    (None, None, LicenseClass::Unknown, src)
                }
            };
            ScannedPackage {
                name: pkg.name,
                version: pkg.version_constraint,
                ecosystem: pkg.ecosystem,
                raw_license,
                spdx_license,
                classification,
                source,
            }
        });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(pkg) => results.push(pkg),
            Err(e) => warn!("Task panicked: {}", e),
        }
    }
    results
}

async fn fetch_license(pkg: &crate::PackageRef) -> Result<Option<String>> {
    let meta = match &pkg.ecosystem {
        Ecosystem::Npm => {
            NpmResolver::default_registry()
                .resolve(&pkg.name, &pkg.version_constraint)
                .await?
        }
        Ecosystem::PyPI => {
            PypiResolver::default_registry()
                .resolve(&pkg.name, &pkg.version_constraint)
                .await?
        }
        Ecosystem::Crates => {
            CratesResolver::default_registry()
                .resolve(&pkg.name, &pkg.version_constraint)
                .await?
        }
        Ecosystem::Go => {
            GoResolver::default_registry()
                .resolve(&pkg.name, &pkg.version_constraint)
                .await?
        }
    };
    Ok(meta.license)
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

/// Format a `ScanResult` as a human-readable text report.
pub fn format_report(result: &ScanResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("Scan ID:  {}\n", result.id));
    out.push_str(&format!("Path:     {}\n", result.path));
    out.push_str(&format!(
        "Scanned:  {}\n",
        result.scanned_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));
    out.push_str(&format!(
        "Manifests found: {}\n",
        result.manifest_files.len()
    ));
    out.push_str(&format!("SBOMs found:     {}\n\n", result.sbom_files.len()));

    if result.packages.is_empty() {
        out.push_str("No packages found.\n");
        return out;
    }

    out.push_str(&format!(
        "{:<40} {:<15} {:<12} {:<20} {}\n",
        "Package", "Version", "Ecosystem", "Classification", "License (SPDX)"
    ));
    out.push_str(&"-".repeat(100));
    out.push('\n');

    let mut sorted = result.packages.clone();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for pkg in &sorted {
        let license_str = pkg
            .spdx_license
            .as_deref()
            .or(pkg.raw_license.as_deref())
            .unwrap_or("unknown");
        out.push_str(&format!(
            "{:<40} {:<15} {:<12} {:<20} {}\n",
            truncate(&pkg.name, 39),
            truncate(&pkg.version, 14),
            format!("{}", pkg.ecosystem),
            format!("{}", pkg.classification),
            license_str,
        ));
    }

    // Summary by classification
    let mut counts = std::collections::HashMap::new();
    for pkg in &result.packages {
        *counts.entry(pkg.classification.clone()).or_insert(0usize) += 1;
    }
    out.push('\n');
    out.push_str("Summary:\n");
    for (class, count) in &counts {
        out.push_str(&format!("  {:20} {}\n", format!("{}:", class), count));
    }
    out.push_str(&format!("  {:20} {}\n", "Total:", result.packages.len()));

    out
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[tokio::test]
    async fn scan_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let result = run_scan(
            tmp.path(),
            ScanOptions {
                concurrency: 1,
                offline: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.packages.len(), 0);
        assert_eq!(result.manifest_files.len(), 0);
    }

    #[tokio::test]
    async fn scan_package_json_offline() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"dependencies":{"lodash":"^4.17.21","express":"~4.18.0"}}"#,
        );
        let result = run_scan(
            tmp.path(),
            ScanOptions {
                concurrency: 1,
                offline: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.packages.len(), 2);
        assert_eq!(result.manifest_files.len(), 1);
        // Offline: licenses are unknown
        for pkg in &result.packages {
            assert_eq!(pkg.classification, LicenseClass::Unknown);
        }
    }

    #[tokio::test]
    async fn scan_sbom_file() {
        let tmp = TempDir::new().unwrap();
        let bom = r#"{
            "bomFormat": "CycloneDX",
            "components": [
                {"name":"lodash","version":"4.17.21","purl":"pkg:npm/lodash@4.17.21",
                 "licenses":[{"license":{"id":"MIT"}}]}
            ]
        }"#;
        write_file(tmp.path(), "bom.json", bom);
        let result = run_scan(
            tmp.path(),
            ScanOptions {
                concurrency: 1,
                offline: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.sbom_files.len(), 1);
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].spdx_license.as_deref(), Some("MIT"));
        assert_eq!(result.packages[0].classification, LicenseClass::Permissive);
    }

    #[tokio::test]
    async fn skip_node_modules() {
        let tmp = TempDir::new().unwrap();
        let nm = tmp.path().join("node_modules").join("some-lib");
        fs::create_dir_all(&nm).unwrap();
        write_file(
            &nm,
            "package.json",
            r#"{"dependencies":{"lodash":"^4.0.0"}}"#,
        );
        // Root has no manifest — only the nested one inside node_modules
        let result = run_scan(
            tmp.path(),
            ScanOptions {
                concurrency: 1,
                offline: true,
            },
        )
        .await
        .unwrap();
        // node_modules is skipped, so nothing should be found
        assert_eq!(result.packages.len(), 0);
    }

    #[test]
    fn format_report_non_empty() {
        use chrono::Utc;
        let result = ScanResult {
            id: "test-id".to_string(),
            path: "/tmp/foo".to_string(),
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
            manifest_files: vec!["/tmp/foo/package.json".to_string()],
            sbom_files: vec![],
        };
        let report = format_report(&result);
        assert!(report.contains("lodash"));
        assert!(report.contains("MIT"));
        assert!(report.contains("permissive"));
    }
}
