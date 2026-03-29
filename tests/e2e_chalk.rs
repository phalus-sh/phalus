//! End-to-end test: pipeline components for chalk
//! Tests manifest parsing, filtering, and output structure for chalk.

use chrono::Utc;
use phalus::manifest::parse_manifest;
use phalus::pipeline::{filter_packages, write_csp_to_disk, write_implementation_to_disk};
use phalus::{CspDocument, CspSpec, Implementation};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_chalk_manifest_parsing() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(
        &manifest_path,
        r#"{
        "name": "my-cli-app",
        "version": "2.0.0",
        "dependencies": {
            "chalk": "5.3.0",
            "commander": "11.0.0",
            "ora": "7.0.1"
        }
    }"#,
    )
    .unwrap();

    let parsed = parse_manifest(&manifest_path).unwrap();
    assert_eq!(parsed.packages.len(), 3);
    assert_eq!(parsed.manifest_type, "package.json");

    // Filter to just chalk
    let filtered = filter_packages(&parsed.packages, Some(&["chalk".into()]), None);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "chalk");
    assert_eq!(filtered[0].version_constraint, "5.3.0");
}

#[test]
fn test_chalk_exclude_filter() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(
        &manifest_path,
        r#"{
        "name": "my-cli-app",
        "version": "2.0.0",
        "dependencies": {
            "chalk": "5.3.0",
            "commander": "11.0.0",
            "ora": "7.0.1"
        }
    }"#,
    )
    .unwrap();

    let parsed = parse_manifest(&manifest_path).unwrap();

    // Exclude chalk, keep the rest
    let filtered = filter_packages(&parsed.packages, None, Some(&["chalk".into()]));
    assert_eq!(filtered.len(), 2);
    let names: Vec<&str> = filtered.iter().map(|p| p.name.as_str()).collect();
    assert!(!names.contains(&"chalk"));
    assert!(names.contains(&"commander"));
    assert!(names.contains(&"ora"));
}

#[test]
fn test_chalk_csp_output_structure() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("output");

    let csp = CspSpec {
        package_name: "chalk".into(),
        package_version: "5.3.0".into(),
        documents: vec![
            CspDocument {
                filename: "01-overview.md".into(),
                content: "chalk: terminal string styling done right".into(),
                content_hash: "ccc".into(),
            },
            CspDocument {
                filename: "02-api-surface.json".into(),
                content:
                    r#"{"chalk": {"red": "Chainable", "bold": "Chainable", "blue": "Chainable"}}"#
                        .into(),
                content_hash: "ddd".into(),
            },
            CspDocument {
                filename: "03-examples.md".into(),
                content: "console.log(chalk.red('Hello world!'));".into(),
                content_hash: "eee".into(),
            },
        ],
        generated_at: Utc::now(),
    };

    write_csp_to_disk(&csp, &output_dir).unwrap();

    assert!(output_dir
        .join("chalk/.cleanroom/csp/01-overview.md")
        .exists());
    assert!(output_dir
        .join("chalk/.cleanroom/csp/02-api-surface.json")
        .exists());
    assert!(output_dir
        .join("chalk/.cleanroom/csp/03-examples.md")
        .exists());
    assert!(output_dir
        .join("chalk/.cleanroom/csp/manifest.json")
        .exists());
}

#[test]
fn test_chalk_implementation_output_structure() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("output");

    let mut files = HashMap::new();
    files.insert(
        "index.js".into(),
        "export const chalk = { red: (s) => `\\x1b[31m${s}\\x1b[0m`, bold: (s) => `\\x1b[1m${s}\\x1b[0m` };".into(),
    );
    files.insert(
        "package.json".into(),
        r#"{"name": "chalk", "version": "5.3.0", "type": "module", "exports": "./index.js"}"#
            .into(),
    );
    files.insert("LICENSE".into(), "MIT License\n\nCopyright (c) ...".into());
    files.insert(
        "readme.md".into(),
        "# chalk\n\nTerminal string styling done right.".into(),
    );

    let imp = Implementation {
        package_name: "chalk".into(),
        files,
        target_language: "javascript".into(),
    };

    write_implementation_to_disk(&imp, &output_dir).unwrap();

    assert!(output_dir.join("chalk/index.js").exists());
    assert!(output_dir.join("chalk/package.json").exists());
    assert!(output_dir.join("chalk/LICENSE").exists());
    assert!(output_dir.join("chalk/readme.md").exists());

    let content = std::fs::read_to_string(output_dir.join("chalk/index.js")).unwrap();
    assert!(content.contains("chalk"));
}

#[test]
fn test_chalk_audit_trail() {
    let dir = TempDir::new().unwrap();
    let audit_path = dir.path().join("audit.jsonl");

    let mut logger = phalus::audit::AuditLogger::new(audit_path.clone()).unwrap();
    logger
        .log(phalus::audit::AuditEvent::ManifestParsed {
            manifest_hash: "chalk_manifest_hash".into(),
            package_count: 1,
        })
        .unwrap();
    logger
        .log(phalus::audit::AuditEvent::DocsFetched {
            package: "chalk@5.3.0".into(),
            urls_accessed: vec![
                "https://api.github.com/repos/chalk/chalk/readme".into(),
                "https://registry.npmjs.org/chalk".into(),
            ],
            content_hashes: [
                ("README.md".into(), "chalk_readme_hash".into()),
                ("npm_metadata".into(), "chalk_meta_hash".into()),
            ]
            .into(),
        })
        .unwrap();

    let hash = logger.finalize().unwrap();
    assert_eq!(hash.len(), 64);

    let content = std::fs::read_to_string(&audit_path).unwrap();
    let lines: Vec<&str> = content.trim().lines().collect();
    assert_eq!(lines.len(), 2);

    // Verify chalk appears in the audit log
    assert!(content.contains("chalk@5.3.0"));
}
