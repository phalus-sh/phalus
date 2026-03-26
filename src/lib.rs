use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod agents;
pub mod audit;
pub mod cache;
pub mod config;
pub mod docs;
pub mod firewall;
pub mod manifest;
pub mod pipeline;
pub mod registry;
pub mod validator;
pub mod web;

// ---------------------------------------------------------------------------
// Ecosystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    PyPI,
    Crates,
    Go,
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ecosystem::Npm => write!(f, "npm"),
            Ecosystem::PyPI => write!(f, "pypi"),
            Ecosystem::Crates => write!(f, "crates"),
            Ecosystem::Go => write!(f, "go"),
        }
    }
}

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRef {
    pub name: String,
    pub version_constraint: String,
    pub ecosystem: Ecosystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedManifest {
    pub manifest_type: String,
    pub packages: Vec<PackageRef>,
}

// ---------------------------------------------------------------------------
// Registry / metadata types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
    pub description: Option<String>,
    pub license: Option<String>,
    pub repository_url: Option<String>,
    pub homepage_url: Option<String>,
    pub unpacked_size: Option<u64>,
    pub registry_url: String,
}

// ---------------------------------------------------------------------------
// Documentation types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEntry {
    pub name: String,
    pub content: String,
    pub source_url: Option<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Documentation {
    pub package: PackageMetadata,
    pub documents: Vec<DocEntry>,
    pub content_hash: String,
}

// ---------------------------------------------------------------------------
// CSP (Clean-Shot Package) types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CspDocument {
    pub filename: String,
    pub content: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CspSpec {
    pub package_name: String,
    pub package_version: String,
    pub documents: Vec<CspDocument>,
    pub generated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Implementation type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    pub package_name: String,
    pub files: HashMap<String, String>,
    pub target_language: String,
}

// ---------------------------------------------------------------------------
// Validation types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityReport {
    pub token_similarity: f64,
    pub name_overlap: f64,
    pub string_overlap: f64,
    pub structural_similarity: f64,
    pub overall_score: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub package: PackageMetadata,
    pub syntax_ok: bool,
    pub tests_passed: u32,
    pub tests_failed: u32,
    pub api_coverage: f64,
    pub license_ok: bool,
    pub similarity: SimilarityReport,
    pub verdict: Verdict,
}

// ---------------------------------------------------------------------------
// License template resolution
// ---------------------------------------------------------------------------

pub fn resolve_license_text(license_id: &str, year: &str, holder: &str) -> Option<String> {
    let template = match license_id {
        "mit" => include_str!("../licenses/mit.txt"),
        "apache-2.0" => include_str!("../licenses/apache-2.0.txt"),
        "bsd-2" => include_str!("../licenses/bsd-2.txt"),
        "bsd-3" => include_str!("../licenses/bsd-3.txt"),
        "isc" => include_str!("../licenses/isc.txt"),
        "unlicense" => include_str!("../licenses/unlicense.txt"),
        "cc0" => include_str!("../licenses/cc0.txt"),
        _ => return None,
    };
    Some(template.replace("{year}", year).replace("{holder}", holder))
}

// ---------------------------------------------------------------------------
// Mode / language enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IsolationMode {
    Context,
    Process,
    Container,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetLanguage {
    Same,
    Rust,
    Go,
    Python,
    TypeScript,
}

impl std::fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetLanguage::Same => write!(f, "same"),
            TargetLanguage::Rust => write!(f, "rust"),
            TargetLanguage::Go => write!(f, "go"),
            TargetLanguage::Python => write!(f, "python"),
            TargetLanguage::TypeScript => write!(f, "typescript"),
        }
    }
}
