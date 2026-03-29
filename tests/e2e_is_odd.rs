//! End-to-end test: pipeline components for is-odd
//! Tests manifest parsing, filtering, and output structure for is-odd.

use chrono::Utc;
use phalus::manifest::parse_manifest;
use phalus::pipeline::{filter_packages, write_csp_to_disk, write_implementation_to_disk};
use phalus::{CspDocument, CspSpec, Implementation};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_is_odd_manifest_parsing() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(
        &manifest_path,
        r#"{
        "name": "my-app",
        "version": "1.0.0",
        "dependencies": {
            "is-odd": "3.0.1",
            "is-even": "1.0.0"
        }
    }"#,
    )
    .unwrap();

    let parsed = parse_manifest(&manifest_path).unwrap();
    assert_eq!(parsed.packages.len(), 2);
    assert_eq!(parsed.manifest_type, "package.json");

    // Filter to just is-odd
    let filtered = filter_packages(&parsed.packages, Some(&["is-odd".into()]), None);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "is-odd");
    assert_eq!(filtered[0].version_constraint, "3.0.1");
}

#[test]
fn test_is_odd_exclude_filter() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(
        &manifest_path,
        r#"{
        "name": "my-app",
        "version": "1.0.0",
        "dependencies": {
            "is-odd": "3.0.1",
            "is-even": "1.0.0",
            "is-number": "7.0.0"
        }
    }"#,
    )
    .unwrap();

    let parsed = parse_manifest(&manifest_path).unwrap();

    // Exclude is-even, keep is-odd and is-number
    let filtered = filter_packages(&parsed.packages, None, Some(&["is-even".into()]));
    assert_eq!(filtered.len(), 2);
    let names: Vec<&str> = filtered.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"is-odd"));
    assert!(names.contains(&"is-number"));
    assert!(!names.contains(&"is-even"));
}

#[test]
fn test_is_odd_csp_output_structure() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("output");

    let csp = CspSpec {
        package_name: "is-odd".into(),
        package_version: "3.0.1".into(),
        documents: vec![
            CspDocument {
                filename: "01-overview.md".into(),
                content: "is-odd: returns true if the given number is odd".into(),
                content_hash: "aaa".into(),
            },
            CspDocument {
                filename: "02-api-surface.json".into(),
                content: r#"{"isOdd": "function(n: number): boolean"}"#.into(),
                content_hash: "bbb".into(),
            },
        ],
        generated_at: Utc::now(),
    };

    write_csp_to_disk(&csp, &output_dir).unwrap();

    assert!(output_dir
        .join("is-odd/.cleanroom/csp/01-overview.md")
        .exists());
    assert!(output_dir
        .join("is-odd/.cleanroom/csp/02-api-surface.json")
        .exists());
    assert!(output_dir
        .join("is-odd/.cleanroom/csp/manifest.json")
        .exists());
}

#[test]
fn test_is_odd_implementation_output_structure() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("output");

    let mut files = HashMap::new();
    files.insert(
        "index.js".into(),
        "module.exports = function isOdd(n) { return n % 2 !== 0; };".into(),
    );
    files.insert(
        "package.json".into(),
        r#"{"name": "is-odd", "version": "3.0.1", "main": "index.js"}"#.into(),
    );
    files.insert("LICENSE".into(), "MIT License\n\nCopyright (c) ...".into());

    let imp = Implementation {
        package_name: "is-odd".into(),
        files,
        target_language: "javascript".into(),
    };

    write_implementation_to_disk(&imp, &output_dir).unwrap();

    assert!(output_dir.join("is-odd/index.js").exists());
    assert!(output_dir.join("is-odd/package.json").exists());
    assert!(output_dir.join("is-odd/LICENSE").exists());

    let content = std::fs::read_to_string(output_dir.join("is-odd/index.js")).unwrap();
    assert!(content.contains("isOdd"));
}
