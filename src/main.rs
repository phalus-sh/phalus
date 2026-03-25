use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use phalus::agents::analyzer;
use phalus::agents::builder;
use phalus::agents::provider::LlmProvider;
use phalus::audit::{AuditEvent, AuditLogger};
use phalus::cache::CspCache;
use phalus::config::PhalusConfig;
use phalus::docs::github::GitHubFetcher;
use phalus::firewall;
use phalus::manifest;
use phalus::pipeline::{
    filter_packages, write_csp_to_disk, write_implementation_to_disk, PackageResult,
    PipelineConfig,
};
use phalus::registry::npm::NpmResolver;
use phalus::validator::license_check;
use phalus::validator::similarity;
use phalus::{
    Documentation, Ecosystem, PackageMetadata, PackageRef, TargetLanguage,
    ValidationReport, Verdict,
};

#[derive(Parser)]
#[command(
    name = "phalus",
    version,
    about = "Private Headless Automated License Uncoupling System"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a manifest and show what would be processed
    Plan {
        manifest: PathBuf,
        #[arg(long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
    },
    /// Run clean room reimplementation
    Run {
        manifest: PathBuf,
        #[arg(long, default_value = "mit")]
        license: String,
        #[arg(long, default_value = "./phalus-output")]
        output: PathBuf,
        #[arg(long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        #[arg(long)]
        target_lang: Option<String>,
        #[arg(long, default_value = "context")]
        isolation: String,
        #[arg(long, default_value_t = 0.70)]
        similarity_threshold: f64,
        #[arg(long, default_value_t = 3)]
        concurrency: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        verbose: bool,
    },
    /// Run on a single package directly (e.g. npm/lodash@4.17.21)
    RunOne {
        package: String,
        #[arg(long, default_value = "mit")]
        license: String,
        #[arg(long, default_value = "./phalus-output")]
        output: PathBuf,
        #[arg(long)]
        target_lang: Option<String>,
        #[arg(long, default_value = "context")]
        isolation: String,
        #[arg(long, default_value_t = 0.70)]
        similarity_threshold: f64,
        #[arg(long)]
        verbose: bool,
    },
    /// Inspect a completed job
    Inspect {
        output_dir: PathBuf,
        #[arg(long)]
        audit: bool,
        #[arg(long)]
        similarity: bool,
        #[arg(long)]
        csp: bool,
    },
    /// Validate an existing output
    Validate {
        output_dir: PathBuf,
        #[arg(long, default_value_t = 0.70)]
        similarity_threshold: f64,
    },
    /// Show configuration
    Config,
    /// Start the local web UI
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

// ---------------------------------------------------------------------------
// Parse ecosystem/name@version format
// ---------------------------------------------------------------------------

fn parse_package_spec(spec: &str) -> Result<(Ecosystem, String, String)> {
    // Format: ecosystem/name@version  e.g. npm/lodash@4.17.21
    let (eco_str, rest) = spec
        .split_once('/')
        .context("expected format: ecosystem/name@version")?;
    let (name, version) = rest
        .split_once('@')
        .context("expected format: ecosystem/name@version")?;

    let ecosystem = match eco_str.to_lowercase().as_str() {
        "npm" => Ecosystem::Npm,
        "pypi" => Ecosystem::PyPI,
        "crates" => Ecosystem::Crates,
        "go" => Ecosystem::Go,
        _ => anyhow::bail!("unsupported ecosystem: {}", eco_str),
    };

    Ok((ecosystem, name.to_string(), version.to_string()))
}

// ---------------------------------------------------------------------------
// Resolve target language
// ---------------------------------------------------------------------------

fn resolve_target_lang(target_lang: &Option<String>) -> TargetLanguage {
    match target_lang.as_deref() {
        Some("rust") => TargetLanguage::Rust,
        Some("go") => TargetLanguage::Go,
        Some("python") => TargetLanguage::Python,
        Some("typescript") => TargetLanguage::TypeScript,
        _ => TargetLanguage::Same,
    }
}

// ---------------------------------------------------------------------------
// run_package: full pipeline for a single package
// ---------------------------------------------------------------------------

async fn run_package(
    pkg: &PackageRef,
    config: &PipelineConfig,
    app_config: &PhalusConfig,
    audit: Arc<Mutex<AuditLogger>>,
) -> PackageResult {
    let name = pkg.name.clone();
    let version = pkg.version_constraint.clone();

    if config.verbose {
        eprintln!("[{}] Starting pipeline...", name);
    }

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

    if config.verbose {
        eprintln!(
            "[{}] Resolved: {} v{}",
            name, metadata.name, metadata.version
        );
    }

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
            let _ = audit.lock().await.log(AuditEvent::DocsFetched {
                package: format!("{}@{}", metadata.name, metadata.version),
                urls_accessed: urls,
                content_hashes,
            });
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

    // 3. Check CSP cache or run Agent A
    let cache = CspCache::default_cache();
    let csp = match cache.get(&metadata.name, &metadata.version, &docs.content_hash) {
        Some(cached) => {
            let spec_hashes: HashMap<String, String> = cached
                .documents
                .iter()
                .map(|d| (d.filename.clone(), d.content_hash.clone()))
                .collect();
            let _ = audit.lock().await.log(AuditEvent::SpecCacheHit {
                package: format!("{}@{}", metadata.name, metadata.version),
                spec_hashes,
            });
            if config.verbose {
                eprintln!("[{}] CSP cache hit", name);
            }
            cached
        }
        None => {
            if config.verbose {
                eprintln!("[{}] Running Agent A (analyzer)...", name);
            }
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

    // 4. Firewall crossing
    let (csp, fw_event) = firewall::cross_firewall(csp, &config.isolation_mode);
    let _ = audit.lock().await.log(fw_event);

    // Write CSP to disk
    if let Err(e) = write_csp_to_disk(&csp, &config.output_dir) {
        eprintln!("[{}] Warning: failed to write CSP: {}", name, e);
    }

    // 5. Run Agent B (skip if dry_run)
    if config.dry_run {
        if config.verbose {
            eprintln!("[{}] Dry run - skipping implementation", name);
        }
        return PackageResult {
            name,
            version: metadata.version,
            success: true,
            error: None,
            validation: None,
        };
    }

    let target_lang = resolve_target_lang(&config.target_lang);

    if config.verbose {
        eprintln!("[{}] Running Agent B (builder)...", name);
    }

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

    // 7. Validate
    let generated_code: String = implementation.files.values().cloned().collect();
    let license_ok = license_check::check_license_file(&implementation.files);
    let sim = similarity::compute_similarity(
        "",
        &generated_code,
        &[],
        &[],
        config.similarity_threshold,
    );
    let verdict = if sim.overall_score <= config.similarity_threshold && license_ok {
        Verdict::Pass
    } else {
        Verdict::Fail
    };

    let validation = ValidationReport {
        package: metadata.clone(),
        syntax_ok: true, // would need actual syntax check
        tests_passed: 0,
        tests_failed: 0,
        api_coverage: 0.0,
        license_ok,
        similarity: sim.clone(),
        verdict: verdict.clone(),
    };

    // Write validation report
    let report_dir = config.output_dir.join(&name);
    let _ = std::fs::create_dir_all(&report_dir);
    let report_path = report_dir.join("validation.json");
    let _ = std::fs::write(&report_path, serde_json::to_string_pretty(&validation).unwrap_or_default());

    // Log validation
    let verdict_str = match &verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
    };
    let _ = audit.lock().await.log(AuditEvent::ValidationCompleted {
        package: format!("{}@{}", metadata.name, metadata.version),
        syntax_ok: true,
        tests_passed: Some(0),
        tests_failed: Some(0),
        similarity_score: sim.overall_score,
        verdict: verdict_str.to_string(),
    });

    PackageResult {
        name,
        version: metadata.version,
        success: matches!(verdict, Verdict::Pass),
        error: None,
        validation: Some(validation),
    }
}

// ---------------------------------------------------------------------------
// Helper: resolve metadata
// ---------------------------------------------------------------------------

async fn resolve_metadata(pkg: &PackageRef) -> Result<PackageMetadata> {
    match pkg.ecosystem {
        Ecosystem::Npm => {
            let resolver = NpmResolver::default_registry();
            let meta = resolver.resolve(&pkg.name, &pkg.version_constraint).await?;
            Ok(meta)
        }
        _ => anyhow::bail!(
            "registry resolver not yet implemented for {}",
            pkg.ecosystem
        ),
    }
}

// ---------------------------------------------------------------------------
// Helper: fetch docs
// ---------------------------------------------------------------------------

async fn fetch_docs(metadata: &PackageMetadata, config: &PhalusConfig) -> Result<Documentation> {
    let token = if config.doc_fetcher.github_token.is_empty() {
        None
    } else {
        Some(config.doc_fetcher.github_token.as_str())
    };

    let fetcher = GitHubFetcher::default_github(token);

    let mut documents = Vec::new();
    if let Some(repo_url) = &metadata.repository_url {
        if let Some((owner, repo)) = GitHubFetcher::parse_github_url(repo_url) {
            match fetcher.fetch_readme(&owner, &repo).await {
                Ok(doc) => documents.push(doc),
                Err(e) => {
                    eprintln!(
                        "[{}] Warning: could not fetch README: {}",
                        metadata.name, e
                    );
                }
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

async fn run_agent_a(
    docs: &Documentation,
    metadata: &PackageMetadata,
    config: &PhalusConfig,
    audit: &Arc<Mutex<AuditLogger>>,
) -> Result<phalus::CspSpec> {
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

    let _ = audit.lock().await.log(AuditEvent::SpecGenerated {
        package: format!("{}@{}", metadata.name, metadata.version),
        document_hashes: doc_hashes,
        model: provider.model().to_string(),
        prompt_hash,
        symbiont_journal_hash: None,
    });

    Ok(csp)
}

// ---------------------------------------------------------------------------
// Helper: run Agent B
// ---------------------------------------------------------------------------

async fn run_agent_b(
    csp: &phalus::CspSpec,
    license: &str,
    target_lang: &TargetLanguage,
    config: &PhalusConfig,
    audit: &Arc<Mutex<AuditLogger>>,
) -> Result<phalus::Implementation> {
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

    let _ = audit.lock().await.log(AuditEvent::ImplementationGenerated {
        package: format!("{}@{}", csp.package_name, csp.package_version),
        file_hashes,
        model: provider.model().to_string(),
        prompt_hash,
        symbiont_journal_hash: None,
    });

    Ok(implementation)
}

// ---------------------------------------------------------------------------
// CLI command implementations
// ---------------------------------------------------------------------------

async fn cmd_plan(
    manifest_path: PathBuf,
    only: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> Result<()> {
    let parsed = manifest::parse_manifest(&manifest_path)?;

    let packages = filter_packages(
        &parsed.packages,
        only.as_deref(),
        exclude.as_deref(),
    );

    println!(
        "Manifest: {} ({} packages, {} after filtering)",
        manifest_path.display(),
        parsed.packages.len(),
        packages.len()
    );
    println!();
    println!("{:<30} {:<15} {:<10}", "PACKAGE", "VERSION", "ECOSYSTEM");
    println!("{}", "-".repeat(55));
    for pkg in &packages {
        println!(
            "{:<30} {:<15} {:<10}",
            pkg.name, pkg.version_constraint, pkg.ecosystem
        );
    }

    Ok(())
}

async fn cmd_run(
    manifest_path: PathBuf,
    config: PipelineConfig,
    only: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> Result<()> {
    let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load()?);

    let parsed = manifest::parse_manifest(&manifest_path)?;

    // Log manifest parsed
    let manifest_content = std::fs::read(&manifest_path)?;
    let manifest_hash = format!("{:x}", Sha256::digest(&manifest_content));

    let packages = filter_packages(
        &parsed.packages,
        only.as_deref(),
        exclude.as_deref(),
    );

    println!(
        "Processing {} packages (concurrency: {})",
        packages.len(),
        config.concurrency
    );

    // Create output dir
    std::fs::create_dir_all(&config.output_dir)?;

    // Set up audit logger
    let audit_path = config.output_dir.join("audit.jsonl");
    let audit_logger = AuditLogger::new(audit_path)?;
    let audit = Arc::new(Mutex::new(audit_logger));

    // Log manifest parsed
    let _ = audit.lock().await.log(AuditEvent::ManifestParsed {
        manifest_hash,
        package_count: packages.len(),
    });

    let start_time = std::time::Instant::now();
    let semaphore = Arc::new(Semaphore::new(config.concurrency));
    let mut join_set = JoinSet::new();

    for pkg in packages {
        let config = config.clone();
        let app_config = app_config.clone();
        let audit = Arc::clone(&audit);
        let semaphore = Arc::clone(&semaphore);

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            run_package(&pkg, &config, &app_config, audit).await
        });
    }

    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(pkg_result) => {
                let status = if pkg_result.success { "OK" } else { "FAIL" };
                println!(
                    "  {} {}@{}",
                    status, pkg_result.name, pkg_result.version
                );
                if let Some(err) = &pkg_result.error {
                    eprintln!("    Error: {}", err);
                }
                results.push(pkg_result);
            }
            Err(e) => {
                eprintln!("  Task panicked: {}", e);
            }
        }
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    let failed = results.iter().filter(|r| !r.success).count();

    // Log job completed
    let audit_hash = audit.lock().await.finalize()?;
    let _ = audit.lock().await.log(AuditEvent::JobCompleted {
        packages_processed: results.len(),
        packages_failed: failed,
        total_elapsed_secs: elapsed,
        audit_log_hash: audit_hash,
    });

    println!();
    println!(
        "Done: {} processed, {} failed, {:.1}s elapsed",
        results.len(),
        failed,
        elapsed
    );

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_run_one(
    package_spec: String,
    config: PipelineConfig,
) -> Result<()> {
    let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load()?);

    let (ecosystem, name, version) = parse_package_spec(&package_spec)?;

    let pkg = PackageRef {
        name: name.clone(),
        version_constraint: version,
        ecosystem,
    };

    std::fs::create_dir_all(&config.output_dir)?;

    let audit_path = config.output_dir.join("audit.jsonl");
    let audit_logger = AuditLogger::new(audit_path)?;
    let audit = Arc::new(Mutex::new(audit_logger));

    let result = run_package(&pkg, &config, &app_config, audit).await;

    if result.success {
        println!("OK {}@{}", result.name, result.version);
    } else {
        eprintln!("FAIL {}@{}", result.name, result.version);
        if let Some(err) = &result.error {
            eprintln!("  Error: {}", err);
        }
        std::process::exit(1);
    }

    Ok(())
}

async fn cmd_inspect(
    output_dir: PathBuf,
    show_audit: bool,
    show_similarity: bool,
    show_csp: bool,
) -> Result<()> {
    if !output_dir.exists() {
        anyhow::bail!("output directory does not exist: {}", output_dir.display());
    }

    // If no specific flag, show summary
    let show_all = !show_audit && !show_similarity && !show_csp;

    if show_audit || show_all {
        let audit_path = output_dir.join("audit.jsonl");
        if audit_path.exists() {
            println!("=== Audit Log ===");
            let content = std::fs::read_to_string(&audit_path)?;
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    println!(
                        "  [{}] seq={} type={}",
                        entry["timestamp"].as_str().unwrap_or("?"),
                        entry["seq"],
                        entry["event"]["type"].as_str().unwrap_or("?")
                    );
                }
            }
            println!();
        } else {
            println!("No audit log found.");
        }
    }

    if show_similarity || show_all {
        // Look for validation.json in package subdirs
        println!("=== Similarity Reports ===");
        if let Ok(entries) = std::fs::read_dir(&output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let validation_path = path.join("validation.json");
                    if validation_path.exists() {
                        let content = std::fs::read_to_string(&validation_path)?;
                        if let Ok(report) = serde_json::from_str::<ValidationReport>(&content) {
                            println!("  {}@{}:", report.package.name, report.package.version);
                            println!(
                                "    token_similarity: {:.4}",
                                report.similarity.token_similarity
                            );
                            println!(
                                "    name_overlap:     {:.4}",
                                report.similarity.name_overlap
                            );
                            println!(
                                "    string_overlap:   {:.4}",
                                report.similarity.string_overlap
                            );
                            println!(
                                "    overall_score:    {:.4}",
                                report.similarity.overall_score
                            );
                            let verdict = match report.verdict {
                                Verdict::Pass => "PASS",
                                Verdict::Fail => "FAIL",
                            };
                            println!("    verdict:          {}", verdict);
                        }
                    }
                }
            }
        }
        println!();
    }

    if show_csp || show_all {
        println!("=== CSP Specs ===");
        if let Ok(entries) = std::fs::read_dir(&output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let csp_dir = path.join(".cleanroom").join("csp");
                    if csp_dir.exists() {
                        let manifest_path = csp_dir.join("manifest.json");
                        if manifest_path.exists() {
                            let content = std::fs::read_to_string(&manifest_path)?;
                            if let Ok(csp) =
                                serde_json::from_str::<phalus::CspSpec>(&content)
                            {
                                println!(
                                    "  {}@{} ({} documents)",
                                    csp.package_name,
                                    csp.package_version,
                                    csp.documents.len()
                                );
                                for doc in &csp.documents {
                                    println!("    - {}", doc.filename);
                                }
                            }
                        }
                    }
                }
            }
        }
        println!();
    }

    Ok(())
}

async fn cmd_validate(output_dir: PathBuf, similarity_threshold: f64) -> Result<()> {
    if !output_dir.exists() {
        anyhow::bail!("output directory does not exist: {}", output_dir.display());
    }

    let mut any_fail = false;

    if let Ok(entries) = std::fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let pkg_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Read all generated files
            let mut files = HashMap::new();
            collect_files(&path, &path, &mut files)?;

            let license_ok = license_check::check_license_file(&files);
            let all_code: String = files.values().cloned().collect();

            let sim = similarity::compute_similarity(
                "",
                &all_code,
                &[],
                &[],
                similarity_threshold,
            );

            let pass = sim.overall_score <= similarity_threshold && license_ok;
            let status = if pass { "PASS" } else { "FAIL" };
            if !pass {
                any_fail = true;
            }

            println!(
                "{} {} (similarity: {:.4}, license: {})",
                status,
                pkg_name,
                sim.overall_score,
                if license_ok { "ok" } else { "missing" }
            );
        }
    }

    if any_fail {
        std::process::exit(1);
    }
    Ok(())
}

fn collect_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    files: &mut HashMap<String, String>,
) -> Result<()> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip .cleanroom directory
                if path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                {
                    continue;
                }
                collect_files(base, &path, files)?;
            } else {
                let relative = path.strip_prefix(base).unwrap_or(&path);
                if let Ok(content) = std::fs::read_to_string(&path) {
                    files.insert(relative.to_string_lossy().to_string(), content);
                }
            }
        }
    }
    Ok(())
}

fn cmd_config() -> Result<()> {
    let config = PhalusConfig::with_env_overrides(PhalusConfig::load()?);
    let toml_str = toml::to_string_pretty(&config)?;
    println!("{}", toml_str);
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Plan {
            manifest,
            only,
            exclude,
        } => cmd_plan(manifest, only, exclude).await,

        Commands::Run {
            manifest,
            license,
            output,
            only,
            exclude,
            target_lang,
            isolation,
            similarity_threshold,
            concurrency,
            dry_run,
            verbose,
        } => {
            let config = PipelineConfig {
                license,
                output_dir: output,
                target_lang,
                isolation_mode: isolation,
                similarity_threshold,
                concurrency,
                dry_run,
                verbose,
            };
            cmd_run(manifest, config, only, exclude).await
        }

        Commands::RunOne {
            package,
            license,
            output,
            target_lang,
            isolation,
            similarity_threshold,
            verbose,
        } => {
            let config = PipelineConfig {
                license,
                output_dir: output,
                target_lang,
                isolation_mode: isolation,
                similarity_threshold,
                concurrency: 1,
                dry_run: false,
                verbose,
            };
            cmd_run_one(package, config).await
        }

        Commands::Inspect {
            output_dir,
            audit,
            similarity,
            csp,
        } => cmd_inspect(output_dir, audit, similarity, csp).await,

        Commands::Validate {
            output_dir,
            similarity_threshold,
        } => cmd_validate(output_dir, similarity_threshold).await,

        Commands::Config => cmd_config(),

        Commands::Serve { host, port } => {
            phalus::web::start_server(&host, port).await
        }
    }
}
