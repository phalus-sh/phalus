use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{CspSpec, Implementation, PackageRef, ValidationReport};

// ---------------------------------------------------------------------------
// PipelineConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub license: String,
    pub output_dir: PathBuf,
    pub target_lang: Option<String>,
    pub isolation_mode: String,
    pub similarity_threshold: f64,
    pub concurrency: usize,
    pub dry_run: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            license: "mit".to_string(),
            output_dir: PathBuf::from("./phalus-output"),
            target_lang: None,
            isolation_mode: "context".to_string(),
            similarity_threshold: 0.70,
            concurrency: 3,
            dry_run: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PackageResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageResult {
    pub name: String,
    pub version: String,
    pub success: bool,
    pub error: Option<String>,
    pub validation: Option<ValidationReport>,
}

// ---------------------------------------------------------------------------
// ProgressEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressEvent {
    PackageStarted { name: String },
    PhaseDone { name: String, phase: String },
    PackageDone { name: String, success: bool },
    JobDone { total: usize, failed: usize },
}

// ---------------------------------------------------------------------------
// Package filtering
// ---------------------------------------------------------------------------

pub fn filter_packages(
    packages: &[PackageRef],
    only: Option<&[String]>,
    exclude: Option<&[String]>,
) -> Vec<PackageRef> {
    packages
        .iter()
        .filter(|p| {
            if let Some(only_list) = only {
                if !only_list.iter().any(|name| name == &p.name) {
                    return false;
                }
            }
            if let Some(exclude_list) = exclude {
                if exclude_list.iter().any(|name| name == &p.name) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Disk output helpers
// ---------------------------------------------------------------------------

/// Validate that `target` is contained within `base`, preventing path traversal.
fn validate_path_within(base: &Path, target: &Path) -> std::io::Result<()> {
    let canonical_base = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    let canonical_target = if target.exists() {
        std::fs::canonicalize(target)?
    } else {
        // For new files, canonicalize the parent
        let parent = target.parent().unwrap_or(base);
        let _ = std::fs::create_dir_all(parent);
        let canonical_parent =
            std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
        canonical_parent.join(target.file_name().unwrap_or_default())
    };
    if !canonical_target.starts_with(&canonical_base) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "path traversal detected: {} is outside {}",
                target.display(),
                base.display()
            ),
        ));
    }
    Ok(())
}

pub fn write_implementation_to_disk(imp: &Implementation, output_dir: &Path) -> Result<()> {
    let pkg_dir = output_dir.join(&imp.package_name);
    std::fs::create_dir_all(&pkg_dir)?;

    for (filename, content) in &imp.files {
        // Reject paths with ..
        if filename.contains("..") {
            tracing::warn!("skipping file with path traversal attempt: {}", filename);
            continue;
        }
        let file_path = pkg_dir.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        validate_path_within(&pkg_dir, &file_path)?;
        std::fs::write(&file_path, content)?;
    }

    Ok(())
}

pub fn write_csp_to_disk(csp: &CspSpec, output_dir: &Path) -> Result<()> {
    let csp_dir = output_dir
        .join(&csp.package_name)
        .join(".cleanroom")
        .join("csp");
    std::fs::create_dir_all(&csp_dir)?;

    for doc in &csp.documents {
        if doc.filename.contains("..") {
            tracing::warn!(
                "skipping CSP document with path traversal attempt: {}",
                doc.filename
            );
            continue;
        }
        let file_path = csp_dir.join(&doc.filename);
        validate_path_within(&csp_dir, &file_path)?;
        std::fs::write(&file_path, &doc.content)?;
    }

    // Also write manifest JSON
    let manifest_path = csp_dir.join("manifest.json");
    let manifest = serde_json::to_string_pretty(csp)?;
    std::fs::write(manifest_path, manifest)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ecosystem, PackageRef};

    #[test]
    fn test_pipeline_config_defaults() {
        let config = PipelineConfig::default();
        assert_eq!(config.concurrency, 3);
        assert_eq!(config.similarity_threshold, 0.70);
        assert_eq!(config.license, "mit");
    }

    #[test]
    fn test_filter_packages_only() {
        let packages = vec![
            PackageRef {
                name: "lodash".into(),
                version_constraint: "^4".into(),
                ecosystem: Ecosystem::Npm,
            },
            PackageRef {
                name: "express".into(),
                version_constraint: "^4".into(),
                ecosystem: Ecosystem::Npm,
            },
            PackageRef {
                name: "chalk".into(),
                version_constraint: "^5".into(),
                ecosystem: Ecosystem::Npm,
            },
        ];
        let filtered = filter_packages(
            &packages,
            Some(&["lodash".into(), "chalk".into()]),
            None,
        );
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_packages_exclude() {
        let packages = vec![
            PackageRef {
                name: "lodash".into(),
                version_constraint: "^4".into(),
                ecosystem: Ecosystem::Npm,
            },
            PackageRef {
                name: "express".into(),
                version_constraint: "^4".into(),
                ecosystem: Ecosystem::Npm,
            },
        ];
        let filtered = filter_packages(&packages, None, Some(&["express".into()]));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "lodash");
    }

    #[test]
    fn test_filter_packages_no_filters() {
        let packages = vec![
            PackageRef {
                name: "lodash".into(),
                version_constraint: "^4".into(),
                ecosystem: Ecosystem::Npm,
            },
        ];
        let filtered = filter_packages(&packages, None, None);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_write_implementation_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = std::collections::HashMap::new();
        files.insert("src/index.js".to_string(), "module.exports = {}".to_string());
        files.insert("package.json".to_string(), "{}".to_string());
        let imp = Implementation {
            package_name: "test-pkg".into(),
            files,
            target_language: "javascript".into(),
        };
        write_implementation_to_disk(&imp, dir.path()).unwrap();

        let index_path = dir.path().join("test-pkg").join("src/index.js");
        assert!(index_path.exists());
        let content = std::fs::read_to_string(index_path).unwrap();
        assert_eq!(content, "module.exports = {}");
    }

    #[test]
    fn test_write_csp_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let csp = CspSpec {
            package_name: "test-pkg".into(),
            package_version: "1.0.0".into(),
            documents: vec![crate::CspDocument {
                filename: "01-overview.md".into(),
                content: "# Overview".into(),
                content_hash: "abc".into(),
            }],
            generated_at: chrono::Utc::now(),
        };
        write_csp_to_disk(&csp, dir.path()).unwrap();

        let overview_path = dir
            .path()
            .join("test-pkg")
            .join(".cleanroom")
            .join("csp")
            .join("01-overview.md");
        assert!(overview_path.exists());
    }
}
