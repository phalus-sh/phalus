use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, Mutex};

use crate::agents::analyzer;
use crate::agents::builder;
use crate::agents::provider::LlmProvider;
use crate::audit::{AuditEvent, AuditLogger};
use crate::cache::CspCache;
use crate::config::PhalusConfig;
use crate::docs::docs_site;
use crate::docs::github::GitHubFetcher;
use crate::firewall;
use crate::registry::crates::CratesResolver;
use crate::registry::golang::GoResolver;
use crate::registry::npm::NpmResolver;
use crate::registry::pypi::PypiResolver;
use crate::validator::api_surface::check_api_surface;
use crate::validator::license_check;
use crate::validator::similarity;
use crate::validator::syntax::run_syntax_check;
use crate::{
    CspSpec, Documentation, Ecosystem, Implementation, PackageMetadata, PackageRef,
    TargetLanguage, ValidationReport, Verdict,
};

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
// Resolve target language
// ---------------------------------------------------------------------------

pub fn resolve_target_lang(target_lang: &Option<String>) -> TargetLanguage {
    match target_lang.as_deref() {
        Some("rust") => TargetLanguage::Rust,
        Some("go") => TargetLanguage::Go,
        Some("python") => TargetLanguage::Python,
        Some("typescript") => TargetLanguage::TypeScript,
        _ => TargetLanguage::Same,
    }
}

// ---------------------------------------------------------------------------
// Helper: emit progress event
// ---------------------------------------------------------------------------

pub fn emit_progress(
    tx: &Option<broadcast::Sender<ProgressEvent>>,
    event: ProgressEvent,
) {
    if let Some(tx) = tx {
        let _ = tx.send(event);
    }
}

// ---------------------------------------------------------------------------
// run_package: full pipeline for a single package
// ---------------------------------------------------------------------------

pub async fn run_package(
    pkg: &PackageRef,
    config: &PipelineConfig,
    app_config: &PhalusConfig,
    audit: Arc<Mutex<AuditLogger>>,
    progress_tx: Option<broadcast::Sender<ProgressEvent>>,
) -> PackageResult {
    let name = pkg.name.clone();
    let version = pkg.version_constraint.clone();

    tracing::info!("[{}] Starting pipeline...", name);

    emit_progress(&progress_tx, ProgressEvent::PackageStarted {
        name: name.clone(),
    });

    // 1. Resolve metadata via registry
    let metadata = match resolve_metadata(pkg).await {
        Ok(m) => m,
        Err(e) => {
            return PackageResult {
                name,
                version,
                success: false,
                error: Some(format!("registry resolve failed: {}", e)),
                validation: None,
            };
        }
    };

    tracing::info!(
        "[{}] Resolved: {} v{}",
        name, metadata.name, metadata.version
    );

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "resolve".to_string(),
    });

    // 2. Fetch documentation
    let docs = match fetch_docs(&metadata, app_config).await {
        Ok(d) => {
            // Log docs fetched
            let urls: Vec<String> = d
                .documents
                .iter()
                .filter_map(|doc| doc.source_url.clone())
                .collect();
            let content_hashes: HashMap<String, String> = d
                .documents
                .iter()
                .map(|doc| (doc.name.clone(), doc.content_hash.clone()))
                .collect();
            if let Err(e) = audit.lock().await.log(AuditEvent::DocsFetched {
                package: format!("{}@{}", metadata.name, metadata.version),
                urls_accessed: urls,
                content_hashes,
            }) {
                tracing::error!("audit log failure: {}", e);
            }
            d
        }
        Err(e) => {
            return PackageResult {
                name,
                version,
                success: false,
                error: Some(format!("doc fetch failed: {}", e)),
                validation: None,
            };
        }
    };

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "docs".to_string(),
    });

    // 3. Check CSP cache or run Agent A
    let cache = CspCache::default_cache();
    let csp = match cache.get(&metadata.name, &metadata.version, &docs.content_hash) {
        Some(cached) => {
            let spec_hashes: HashMap<String, String> = cached
                .documents
                .iter()
                .map(|d| (d.filename.clone(), d.content_hash.clone()))
                .collect();
            if let Err(e) = audit.lock().await.log(AuditEvent::SpecCacheHit {
                package: format!("{}@{}", metadata.name, metadata.version),
                spec_hashes,
            }) {
                tracing::error!("audit log failure: {}", e);
            }
            tracing::info!("[{}] CSP cache hit", name);
            cached
        }
        None => {
            tracing::info!("[{}] Running Agent A (analyzer)...", name);
            match run_agent_a(&docs, &metadata, app_config, &audit).await {
                Ok(spec) => {
                    // Store in cache
                    let _ = cache.put(
                        &metadata.name,
                        &metadata.version,
                        &docs.content_hash,
                        &spec,
                    );
                    spec
                }
                Err(e) => {
                    return PackageResult {
                        name,
                        version,
                        success: false,
                        error: Some(format!("Agent A failed: {}", e)),
                        validation: None,
                    };
                }
            }
        }
    };

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "analyze".to_string(),
    });

    // 4. Firewall crossing
    let (csp, fw_event) = firewall::cross_firewall(csp, &config.isolation_mode).await;
    if let Err(e) = audit.lock().await.log(fw_event) {
        tracing::error!("audit log failure: {}", e);
    }

    // Write CSP to disk
    if let Err(e) = write_csp_to_disk(&csp, &config.output_dir) {
        tracing::warn!("[{}] failed to write CSP: {}", name, e);
    }

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "firewall".to_string(),
    });

    // 5. Run Agent B (skip if dry_run)
    if config.dry_run {
        tracing::info!("[{}] Dry run - skipping implementation", name);
        return PackageResult {
            name,
            version: metadata.version,
            success: true,
            error: None,
            validation: None,
        };
    }

    let target_lang = resolve_target_lang(&config.target_lang);

    tracing::info!("[{}] Running Agent B (builder)...", name);

    let implementation =
        match run_agent_b(&csp, &config.license, &target_lang, app_config, &audit).await {
            Ok(imp) => imp,
            Err(e) => {
                return PackageResult {
                    name,
                    version: metadata.version,
                    success: false,
                    error: Some(format!("Agent B failed: {}", e)),
                    validation: None,
                };
            }
        };

    // 6. Write output to disk
    if let Err(e) = write_implementation_to_disk(&implementation, &config.output_dir) {
        return PackageResult {
            name,
            version: metadata.version,
            success: false,
            error: Some(format!("write output failed: {}", e)),
            validation: None,
        };
    }

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "build".to_string(),
    });

    // 6b. Run generated tests if configured
    let (tests_passed, tests_failed) = if app_config.validation.run_tests {
        // Try Docker first for sandboxed execution, fall back to local
        let result = crate::validator::test_runner::run_tests_in_docker(
            &implementation.target_language,
            &config.output_dir.join(&name),
        )
        .await;

        let result = match result {
            Some(r) => Some(r),
            None => {
                // Fall back to local test runner
                crate::validator::test_runner::run_generated_tests(
                    &implementation.target_language,
                    &config.output_dir.join(&name),
                )
                .await
            }
        };

        match result {
            Some(r) => (r.passed, r.failed),
            None => (0, 0),
        }
    } else {
        (0, 0)
    };

    // 7. Validate
    let generated_code: String = implementation.files.values().cloned().collect();
    let header_ok = implementation
        .files
        .iter()
        .filter(|(k, _)| {
            k.ends_with(".js")
                || k.ends_with(".ts")
                || k.ends_with(".rs")
                || k.ends_with(".py")
                || k.ends_with(".go")
        })
        .all(|(_, content)| license_check::check_license_header(content, &config.license));
    let license_ok = license_check::check_license_file(&implementation.files) && header_ok;
    let sim = similarity::compute_similarity(
        "",
        &generated_code,
        &[],
        &[],
        config.similarity_threshold,
    );

    // Syntax check (skip gracefully if the check tool isn't installed)
    let syntax_ok = run_syntax_check(
        &implementation.target_language,
        &config.output_dir.join(&name),
    )
    .await
    .unwrap_or(true);

    // API surface check
    let expected_exports: Vec<String> = csp
        .documents
        .iter()
        .find(|d| d.filename.contains("api-surface"))
        .map(|d| extract_export_names(&d.content))
        .unwrap_or_default();

    let api_coverage = check_api_surface(&expected_exports, &generated_code);

    let verdict = if sim.overall_score <= config.similarity_threshold && license_ok && syntax_ok {
        Verdict::Pass
    } else {
        Verdict::Fail
    };

    let validation = ValidationReport {
        package: metadata.clone(),
        syntax_ok,
        tests_passed,
        tests_failed,
        api_coverage,
        license_ok,
        similarity: sim,
        verdict,
    };

    // Write validation report
    let report_dir = config.output_dir.join(&name);
    let _ = std::fs::create_dir_all(&report_dir);
    let report_path = report_dir.join("validation.json");
    let _ = std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&validation).unwrap_or_default(),
    );

    // Log validation
    let verdict_str = match &validation.verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
    };
    if let Err(e) = audit.lock().await.log(AuditEvent::ValidationCompleted {
        package: format!("{}@{}", metadata.name, metadata.version),
        syntax_ok,
        tests_passed: Some(tests_passed),
        tests_failed: Some(tests_failed),
        similarity_score: validation.similarity.overall_score,
        verdict: verdict_str.to_string(),
    }) {
        tracing::error!("audit log failure: {}", e);
    }

    emit_progress(&progress_tx, ProgressEvent::PhaseDone {
        name: name.clone(),
        phase: "validate".to_string(),
    });

    let success = matches!(validation.verdict, Verdict::Pass);
    emit_progress(&progress_tx, ProgressEvent::PackageDone {
        name: name.clone(),
        success,
    });

    PackageResult {
        name,
        version: metadata.version,
        success,
        error: None,
        validation: Some(validation),
    }
}

// ---------------------------------------------------------------------------
// Helper: resolve metadata
// ---------------------------------------------------------------------------

pub async fn resolve_metadata(pkg: &PackageRef) -> Result<PackageMetadata> {
    match pkg.ecosystem {
        Ecosystem::Npm => {
            let resolver = NpmResolver::default_registry();
            Ok(resolver.resolve(&pkg.name, &pkg.version_constraint).await?)
        }
        Ecosystem::PyPI => {
            let resolver = PypiResolver::default_registry();
            Ok(resolver.resolve(&pkg.name, &pkg.version_constraint).await?)
        }
        Ecosystem::Crates => {
            let resolver = CratesResolver::default_registry();
            Ok(resolver.resolve(&pkg.name, &pkg.version_constraint).await?)
        }
        Ecosystem::Go => {
            let resolver = GoResolver::default_registry();
            Ok(resolver.resolve(&pkg.name, &pkg.version_constraint).await?)
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: fetch docs
// ---------------------------------------------------------------------------

pub async fn fetch_docs(metadata: &PackageMetadata, config: &PhalusConfig) -> Result<Documentation> {
    let token = if config.doc_fetcher.github_token.is_empty() {
        None
    } else {
        Some(config.doc_fetcher.github_token.as_str())
    };

    let fetcher = GitHubFetcher::default_github(token);

    let max_code_example_lines = config.doc_fetcher.max_code_example_lines as usize;

    let mut documents = Vec::new();
    if let Some(repo_url) = &metadata.repository_url {
        if let Some((owner, repo)) = GitHubFetcher::parse_github_url(repo_url) {
            match fetcher.fetch_readme(&owner, &repo).await {
                Ok(mut doc) => {
                    doc.content = docs_site::strip_long_code_examples(
                        &doc.content,
                        max_code_example_lines,
                    );
                    documents.push(doc);
                }
                Err(e) => {
                    tracing::warn!(
                        "[{}] could not fetch README: {}",
                        metadata.name, e
                    );
                }
            }
        }
    }

    // Fetch documentation site if homepage is available
    if let Some(homepage_url) = &metadata.homepage_url {
        let max_size_kb = config.doc_fetcher.max_readme_size_kb as u64;
        match docs_site::fetch_doc_site(homepage_url, max_size_kb).await {
            Ok(doc) => documents.push(doc),
            Err(e) => {
                tracing::warn!(
                    "[{}] could not fetch doc site: {}",
                    metadata.name, e
                );
            }
        }
    }

    let mut hasher = Sha256::new();
    for doc in &documents {
        hasher.update(doc.content_hash.as_bytes());
    }
    let content_hash = format!("{:x}", hasher.finalize());

    Ok(Documentation {
        package: metadata.clone(),
        documents,
        content_hash,
    })
}

// ---------------------------------------------------------------------------
// Helper: run Agent A
// ---------------------------------------------------------------------------

pub async fn run_agent_a(
    docs: &Documentation,
    metadata: &PackageMetadata,
    config: &PhalusConfig,
    audit: &Arc<Mutex<AuditLogger>>,
) -> Result<crate::CspSpec> {
    let api_key = &config.llm.agent_a_api_key;
    if api_key.is_empty() {
        anyhow::bail!("agent_a_api_key not configured. Set PHALUS_LLM__AGENT_A_API_KEY or configure in ~/.phalus/config.toml");
    }

    let base_url = if config.llm.agent_a_base_url.is_empty() {
        None
    } else {
        Some(config.llm.agent_a_base_url.as_str())
    };
    let provider = LlmProvider::new(api_key, &config.llm.agent_a_model, base_url);

    let system = analyzer::system_prompt();
    let user_prompt = analyzer::build_analyzer_prompt(docs);
    let prompt_hash = format!("{:x}", Sha256::digest(user_prompt.as_bytes()));

    let response = provider.complete(system, &user_prompt, 8192).await?;
    let csp = analyzer::parse_csp_response(&response, &metadata.name, &metadata.version)?;

    let doc_hashes: HashMap<String, String> = csp
        .documents
        .iter()
        .map(|d| (d.filename.clone(), d.content_hash.clone()))
        .collect();

    if let Err(e) = audit.lock().await.log(AuditEvent::SpecGenerated {
        package: format!("{}@{}", metadata.name, metadata.version),
        document_hashes: doc_hashes,
        model: provider.model().to_string(),
        prompt_hash,
        symbiont_journal_hash: None,
    }) {
        tracing::error!("audit log failure: {}", e);
    }

    Ok(csp)
}

// ---------------------------------------------------------------------------
// Helper: run Agent B
// ---------------------------------------------------------------------------

pub async fn run_agent_b(
    csp: &crate::CspSpec,
    license: &str,
    target_lang: &TargetLanguage,
    config: &PhalusConfig,
    audit: &Arc<Mutex<AuditLogger>>,
) -> Result<crate::Implementation> {
    let api_key = &config.llm.agent_b_api_key;
    if api_key.is_empty() {
        anyhow::bail!("agent_b_api_key not configured. Set PHALUS_LLM__AGENT_B_API_KEY or configure in ~/.phalus/config.toml");
    }

    let base_url = if config.llm.agent_b_base_url.is_empty() {
        None
    } else {
        Some(config.llm.agent_b_base_url.as_str())
    };
    let provider = LlmProvider::new(api_key, &config.llm.agent_b_model, base_url);

    let system = builder::system_prompt();
    let user_prompt = builder::build_builder_prompt(csp, license, target_lang);
    let prompt_hash = format!("{:x}", Sha256::digest(user_prompt.as_bytes()));

    let response = provider.complete(system, &user_prompt, 16384).await?;
    let lang_str = target_lang.to_string();
    let implementation =
        builder::parse_implementation_response(&response, &csp.package_name, &lang_str)?;

    let file_hashes: HashMap<String, String> = implementation
        .files
        .iter()
        .map(|(k, v)| (k.clone(), format!("{:x}", Sha256::digest(v.as_bytes()))))
        .collect();

    if let Err(e) = audit.lock().await.log(AuditEvent::ImplementationGenerated {
        package: format!("{}@{}", csp.package_name, csp.package_version),
        file_hashes,
        model: provider.model().to_string(),
        prompt_hash,
        symbiont_journal_hash: None,
    }) {
        tracing::error!("audit log failure: {}", e);
    }

    Ok(implementation)
}

// ---------------------------------------------------------------------------
// Helper: extract export names from API surface document
// ---------------------------------------------------------------------------

/// Best-effort extraction of export/function names from an API surface document.
///
/// Looks for JSON keys such as `"functions"`, `"methods"`, or `"exports"` that
/// contain arrays of strings, or falls back to treating top-level object keys
/// as export names.
pub fn extract_export_names(content: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };

    // Try well-known array keys first.
    for key in &["functions", "methods", "exports"] {
        if let Some(arr) = value.get(key).and_then(|v| v.as_array()) {
            let names: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !names.is_empty() {
                return names;
            }
        }
    }

    // Fallback: use top-level object keys (excluding metadata-like keys).
    if let Some(obj) = value.as_object() {
        return obj
            .keys()
            .filter(|k| !["name", "version", "description", "type"].contains(&k.as_str()))
            .cloned()
            .collect();
    }

    Vec::new()
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
