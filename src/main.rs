use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use phalus::audit::{AuditEvent, AuditLogger};
use phalus::config::PhalusConfig;
use phalus::manifest;
use phalus::pipeline::{filter_packages, run_package, PipelineConfig};
use phalus::validator::license_check;
use phalus::validator::similarity;
use phalus::{Ecosystem, PackageRef, ValidationReport, Verdict};

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
    /// Build from an existing CSP (run Agent B only)
    Build {
        /// Path to a CSP manifest.json or CSP directory containing manifest.json
        csp: PathBuf,
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
// CLI command implementations
// ---------------------------------------------------------------------------

async fn cmd_plan(
    manifest_path: PathBuf,
    only: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> Result<()> {
    let parsed = manifest::parse_manifest(&manifest_path)?;

    let packages = filter_packages(&parsed.packages, only.as_deref(), exclude.as_deref());

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

    let packages = filter_packages(&parsed.packages, only.as_deref(), exclude.as_deref());

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
    if let Err(e) = audit.lock().await.log(AuditEvent::ManifestParsed {
        manifest_hash,
        package_count: packages.len(),
    }) {
        tracing::error!("audit log failure: {}", e);
    }

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
            run_package(&pkg, &config, &app_config, audit, None).await
        });
    }

    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(pkg_result) => {
                let status = if pkg_result.success { "OK" } else { "FAIL" };
                println!("  {} {}@{}", status, pkg_result.name, pkg_result.version);
                if let Some(err) = &pkg_result.error {
                    tracing::error!("    Error: {}", err);
                }
                results.push(pkg_result);
            }
            Err(e) => {
                tracing::error!("Task panicked: {}", e);
            }
        }
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    let failed = results.iter().filter(|r| !r.success).count();

    // Log job completed
    let audit_hash = audit.lock().await.finalize()?;
    if let Err(e) = audit.lock().await.log(AuditEvent::JobCompleted {
        packages_processed: results.len(),
        packages_failed: failed,
        total_elapsed_secs: elapsed,
        audit_log_hash: audit_hash,
    }) {
        tracing::error!("audit log failure: {}", e);
    }

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

async fn cmd_run_one(package_spec: String, config: PipelineConfig) -> Result<()> {
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

    let result = run_package(&pkg, &config, &app_config, audit, None).await;

    if result.success {
        println!("OK {}@{}", result.name, result.version);
    } else {
        tracing::error!("FAIL {}@{}", result.name, result.version);
        if let Some(err) = &result.error {
            tracing::error!("  Error: {}", err);
        }
        std::process::exit(1);
    }

    Ok(())
}

async fn cmd_build(csp_path: PathBuf, config: PipelineConfig) -> Result<()> {
    let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load()?);

    // Resolve the manifest.json path
    let manifest_path = if csp_path.is_dir() {
        csp_path.join("manifest.json")
    } else {
        csp_path
    };

    if !manifest_path.exists() {
        anyhow::bail!(
            "CSP manifest not found: {}. Expected a manifest.json file or a directory containing one.",
            manifest_path.display()
        );
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let csp: phalus::CspSpec = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse CSP manifest: {}", manifest_path.display()))?;

    println!(
        "Building {}@{} from CSP ({} documents)",
        csp.package_name,
        csp.package_version,
        csp.documents.len()
    );

    std::fs::create_dir_all(&config.output_dir)?;

    let audit_path = config.output_dir.join("audit.jsonl");
    let audit_logger = AuditLogger::new(audit_path)?;
    let audit = Arc::new(Mutex::new(audit_logger));

    // Firewall crossing (the CSP is already on disk, but we still log the event)
    let (csp, fw_event) = phalus::firewall::cross_firewall(csp, &config.isolation_mode).await;
    if let Err(e) = audit.lock().await.log(fw_event) {
        tracing::error!("audit log failure: {}", e);
    }

    // Run Agent B
    let target_lang = phalus::pipeline::resolve_target_lang(&config.target_lang);

    let implementation =
        phalus::pipeline::run_agent_b(&csp, &config.license, &target_lang, &app_config, &audit)
            .await?;

    // Write output
    phalus::pipeline::write_implementation_to_disk(&implementation, &config.output_dir)?;
    phalus::pipeline::write_csp_to_disk(&csp, &config.output_dir)?;

    println!("OK {}@{}", csp.package_name, csp.package_version);
    println!(
        "Output: {}",
        config.output_dir.join(&csp.package_name).display()
    );

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
                            if let Ok(csp) = serde_json::from_str::<phalus::CspSpec>(&content) {
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

            let sim = similarity::compute_similarity("", &all_code, &[], &[], similarity_threshold);

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
    let mut config = PhalusConfig::with_env_overrides(PhalusConfig::load()?);
    // Redact secrets before printing
    if !config.llm.agent_a_api_key.is_empty() {
        config.llm.agent_a_api_key = "***".into();
    }
    if !config.llm.agent_b_api_key.is_empty() {
        config.llm.agent_b_api_key = "***".into();
    }
    if !config.doc_fetcher.github_token.is_empty() {
        config.doc_fetcher.github_token = "***".into();
    }
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
            if verbose {
                tracing::info!("verbose mode enabled");
            }
            let config = PipelineConfig {
                license,
                output_dir: output,
                target_lang,
                isolation_mode: isolation,
                similarity_threshold,
                concurrency,
                dry_run,
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
            if verbose {
                tracing::info!("verbose mode enabled");
            }
            let config = PipelineConfig {
                license,
                output_dir: output,
                target_lang,
                isolation_mode: isolation,
                similarity_threshold,
                concurrency: 1,
                dry_run: false,
            };
            cmd_run_one(package, config).await
        }

        Commands::Build {
            csp,
            license,
            output,
            target_lang,
            isolation,
            similarity_threshold,
            verbose,
        } => {
            if verbose {
                tracing::info!("verbose mode enabled");
            }
            let config = PipelineConfig {
                license,
                output_dir: output,
                target_lang,
                isolation_mode: isolation,
                similarity_threshold,
                concurrency: 1,
                dry_run: false,
            };
            cmd_build(csp, config).await
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

        Commands::Serve { host, port } => phalus::web::start_server(&host, port).await,
    }
}
