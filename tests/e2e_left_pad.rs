//! End-to-end test: pipeline components for left-pad
//! Tests manifest parsing, registry resolution (mocked), and output structure.

use phalus::manifest::parse_manifest;
use phalus::pipeline::{filter_packages, write_csp_to_disk, write_implementation_to_disk};
use phalus::{CspDocument, CspSpec, Implementation};
use chrono::Utc;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_manifest_to_pipeline_config() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(&manifest_path, r#"{
        "name": "test",
        "version": "1.0.0",
        "dependencies": {
            "left-pad": "1.1.3",
            "is-odd": "3.0.1"
        }
    }"#).unwrap();

    let parsed = parse_manifest(&manifest_path).unwrap();
    assert_eq!(parsed.packages.len(), 2);
    assert_eq!(parsed.manifest_type, "package.json");

    // Filter to just left-pad
    let filtered = filter_packages(&parsed.packages, Some(&["left-pad".into()]), None);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "left-pad");
}

#[test]
fn test_csp_and_implementation_output_structure() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("output");

    // Simulate CSP output
    let csp = CspSpec {
        package_name: "left-pad".into(),
        package_version: "1.1.3".into(),
        documents: vec![
            CspDocument { filename: "01-overview.md".into(), content: "Left-pad utility".into(), content_hash: "abc".into() },
            CspDocument { filename: "02-api-surface.json".into(), content: "{}".into(), content_hash: "def".into() },
        ],
        generated_at: Utc::now(),
    };

    write_csp_to_disk(&csp, &output_dir).unwrap();

    // Verify CSP structure
    assert!(output_dir.join("left-pad/.cleanroom/csp/01-overview.md").exists());
    assert!(output_dir.join("left-pad/.cleanroom/csp/02-api-surface.json").exists());

    // Simulate implementation output
    let mut files = HashMap::new();
    files.insert("src/index.js".into(), "module.exports = function leftPad(str, len, ch) {};".into());
    files.insert("package.json".into(), r#"{"name": "left-pad", "version": "1.1.3"}"#.into());
    files.insert("LICENSE".into(), "MIT License".into());

    let imp = Implementation {
        package_name: "left-pad".into(),
        files,
        target_language: "javascript".into(),
    };

    write_implementation_to_disk(&imp, &output_dir).unwrap();

    // Verify implementation structure
    assert!(output_dir.join("left-pad/src/index.js").exists());
    assert!(output_dir.join("left-pad/package.json").exists());
    assert!(output_dir.join("left-pad/LICENSE").exists());

    let content = std::fs::read_to_string(output_dir.join("left-pad/src/index.js")).unwrap();
    assert!(content.contains("leftPad"));
}

#[test]
fn test_audit_trail_creation() {
    let dir = TempDir::new().unwrap();
    let audit_path = dir.path().join("audit.jsonl");

    let mut logger = phalus::audit::AuditLogger::new(audit_path.clone()).unwrap();
    logger.log(phalus::audit::AuditEvent::ManifestParsed {
        manifest_hash: "test123".into(),
        package_count: 2,
    }).unwrap();
    logger.log(phalus::audit::AuditEvent::DocsFetched {
        package: "left-pad@1.1.3".into(),
        urls_accessed: vec!["https://api.github.com/repos/left-pad/left-pad/readme".into()],
        content_hashes: [("README.md".into(), "abc123".into())].into(),
    }).unwrap();

    let hash = logger.finalize().unwrap();
    assert_eq!(hash.len(), 64);

    let content = std::fs::read_to_string(&audit_path).unwrap();
    let lines: Vec<&str> = content.trim().lines().collect();
    assert_eq!(lines.len(), 2);
}
