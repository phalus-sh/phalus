use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use std::path::PathBuf;

use crate::audit::AuditLogger;
use crate::config::PhalusConfig;
use crate::manifest::cargo::CargoParser;
use crate::manifest::gomod::GoModParser;
use crate::manifest::npm::NpmParser;
use crate::manifest::pypi::PypiParser;
use crate::pipeline::{PipelineConfig, ProgressEvent};
use crate::scan::{run_scan, ScanOptions};
use crate::{store, ParsedManifest};

// ---------------------------------------------------------------------------
// Job state tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct JobState {
    pub status: String,
    pub results: Vec<serde_json::Value>,
}

pub struct AppState {
    pub progress_tx: broadcast::Sender<ProgressEvent>,
    pub jobs: Mutex<HashMap<String, JobState>>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/manifest/parse", post(parse_manifest))
        .route("/api/health", get(health))
        .route("/api/jobs", post(create_job))
        .route("/api/jobs/{id}/stream", get(stream_job))
        .route("/api/jobs/{id}/download", get(download_job))
        .route("/api/packages/{name}/csp", get(get_package_csp))
        .route("/api/packages/{name}/audit", get(get_package_audit))
        .route("/api/packages/{name}/code", get(get_package_code))
        // Phase 1: license scanning endpoints
        .route("/api/scans", post(create_scan).get(list_scans))
        .route("/api/scans/{id}", get(get_scan))
        .route("/api/licenses", get(list_licenses))
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}

/// Try each manifest parser in order, requiring non-empty results.
fn try_parse_manifest(body: &str) -> Option<ParsedManifest> {
    if let Ok(manifest) = NpmParser::parse(body) {
        if !manifest.packages.is_empty() {
            return Some(manifest);
        }
    }
    if let Ok(manifest) = PypiParser::parse(body) {
        if !manifest.packages.is_empty() {
            return Some(manifest);
        }
    }
    if let Ok(manifest) = CargoParser::parse(body) {
        if !manifest.packages.is_empty() {
            return Some(manifest);
        }
    }
    if let Ok(manifest) = GoModParser::parse(body) {
        if !manifest.packages.is_empty() {
            return Some(manifest);
        }
    }
    None
}

async fn parse_manifest(State(_state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    match try_parse_manifest(&body) {
        Some(manifest) => Json(serde_json::to_value(&manifest).unwrap()).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "could not parse manifest"})),
        )
            .into_response(),
    }
}

async fn health() -> Json<serde_json::Value> {
    let output_dir = std::fs::canonicalize("./phalus-output")
        .unwrap_or_else(|_| PathBuf::from("./phalus-output"));
    Json(serde_json::json!({"status": "ok", "output_dir": output_dir.to_string_lossy()}))
}

// ---------------------------------------------------------------------------
// Job creation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    manifest_content: String,
    license: Option<String>,
    isolation: Option<String>,
    #[serde(default)]
    resume: bool,
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> impl IntoResponse {
    // Reject early if no LLM API keys are configured
    let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load().unwrap_or_default());
    let missing_a = app_config.llm.agent_a_api_key.is_empty();
    let missing_b = app_config.llm.agent_b_api_key.is_empty();
    if missing_a || missing_b {
        let missing: Vec<&str> = [
            if missing_a {
                Some("PHALUS_LLM__AGENT_A_API_KEY")
            } else {
                None
            },
            if missing_b {
                Some("PHALUS_LLM__AGENT_B_API_KEY")
            } else {
                None
            },
        ]
        .into_iter()
        .flatten()
        .collect();
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Missing LLM API key(s): {}. Set env vars or configure in ~/.phalus/config.toml", missing.join(", "))
            })),
        )
            .into_response();
    }

    let job_id = uuid::Uuid::new_v4().to_string();

    // Parse the manifest
    let parsed = match try_parse_manifest(&req.manifest_content) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "could not parse manifest"})),
            )
                .into_response();
        }
    };

    // Register job
    {
        let mut jobs = state.jobs.lock().await;
        jobs.insert(
            job_id.clone(),
            JobState {
                status: "running".to_string(),
                results: Vec::new(),
            },
        );
    }

    let tx = state.progress_tx.clone();
    let job_id_clone = job_id.clone();
    let license = req.license.unwrap_or_else(|| "mit".to_string());
    let isolation = req.isolation.unwrap_or_else(|| "context".to_string());
    let resume = req.resume;

    // Spawn background task to process packages via the real pipeline
    let state_jobs = Arc::clone(&state) as Arc<AppState>;
    tokio::spawn(async move {
        tracing::info!(
            "Job {} starting with {} packages",
            job_id_clone,
            parsed.packages.len()
        );
        let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load().unwrap_or_default());
        tracing::info!(
            "Job {} config loaded, agent_a_key set: {}",
            job_id_clone,
            !app_config.llm.agent_a_api_key.is_empty()
        );

        let pipeline_config = PipelineConfig {
            license,
            output_dir: PathBuf::from("./phalus-output"),
            target_lang: None,
            isolation_mode: isolation,
            similarity_threshold: 0.70,
            concurrency: 3,
            dry_run: false,
            resume,
        };

        std::fs::create_dir_all(&pipeline_config.output_dir).ok();

        let audit_path = pipeline_config.output_dir.join("audit.jsonl");
        let audit = Arc::new(tokio::sync::Mutex::new(
            AuditLogger::new(audit_path).unwrap(),
        ));

        let mut results = Vec::new();
        for pkg in &parsed.packages {
            let result = crate::pipeline::run_package(
                pkg,
                &pipeline_config,
                &app_config,
                audit.clone(),
                Some(tx.clone()),
            )
            .await;

            results.push(serde_json::to_value(&result).unwrap_or_default());

            // PackageDone is already emitted inside run_package
        }

        let failed = results
            .iter()
            .filter(|r| !r.get("success").and_then(|v| v.as_bool()).unwrap_or(true))
            .count();
        let _ = tx.send(ProgressEvent::JobDone {
            total: results.len(),
            failed,
        });

        tracing::info!(
            "Job {} completed: {} processed, {} failed",
            job_id_clone,
            results.len(),
            failed
        );

        // Update job state
        let mut jobs = state_jobs.jobs.lock().await;
        if let Some(job) = jobs.get_mut(&job_id_clone) {
            job.status = "completed".to_string();
            job.results = results;
        }
    });

    Json(serde_json::json!({"job_id": job_id})).into_response()
}

// ---------------------------------------------------------------------------
// SSE streaming
// ---------------------------------------------------------------------------

async fn stream_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Check job exists
    {
        let jobs = state.jobs.lock().await;
        if !jobs.contains_key(&id) {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "job not found"})),
            )
                .into_response();
        }
    }

    let rx = state.progress_tx.subscribe();
    Sse::new(make_event_stream(rx))
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn make_event_stream(
    mut rx: broadcast::Receiver<ProgressEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok(Event::default().data(data));

                    // Stop streaming after JobDone
                    if matches!(event, ProgressEvent::JobDone { .. }) {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ZIP download
// ---------------------------------------------------------------------------

async fn download_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Verify job exists and is completed
    {
        let jobs = state.jobs.lock().await;
        match jobs.get(&id) {
            Some(job) if job.status == "completed" => {}
            Some(_) => return (StatusCode::CONFLICT, "job still running").into_response(),
            None => return (StatusCode::NOT_FOUND, "job not found").into_response(),
        }
    }

    let output_dir = std::path::PathBuf::from("./phalus-output");
    if !output_dir.exists() {
        return (StatusCode::NOT_FOUND, "no output directory").into_response();
    }

    // Create ZIP in memory
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        add_dir_to_zip(&mut zip, &output_dir, &output_dir, &options).ok();
        zip.finish().ok();
    }

    let bytes = buf.into_inner();
    (
        StatusCode::OK,
        [
            ("content-type", "application/zip"),
            (
                "content-disposition",
                "attachment; filename=\"phalus-output.zip\"",
            ),
        ],
        bytes,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Package-level endpoints
// ---------------------------------------------------------------------------

async fn get_package_csp(Path(name): Path<String>) -> impl IntoResponse {
    let manifest_path = PathBuf::from("./phalus-output")
        .join(&name)
        .join(".cleanroom")
        .join("csp")
        .join("manifest.json");

    match tokio::fs::read_to_string(&manifest_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(value) => Json(value).into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "invalid CSP manifest JSON"})),
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CSP manifest not found"})),
        )
            .into_response(),
    }
}

async fn get_package_audit(Path(name): Path<String>) -> impl IntoResponse {
    let audit_path = PathBuf::from("./phalus-output").join("audit.jsonl");

    match tokio::fs::read_to_string(&audit_path).await {
        Ok(content) => {
            let matching: Vec<serde_json::Value> = content
                .lines()
                .filter_map(|line| {
                    let value: serde_json::Value = serde_json::from_str(line).ok()?;
                    // Check if the event's package field contains the package name
                    let event = value.get("event")?;
                    let package = event.get("package")?.as_str()?;
                    if package.contains(&name) {
                        Some(value)
                    } else {
                        None
                    }
                })
                .collect();
            Json(serde_json::json!(matching)).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "audit log not found"})),
        )
            .into_response(),
    }
}

async fn get_package_code(Path(name): Path<String>) -> impl IntoResponse {
    let pkg_dir = PathBuf::from("./phalus-output").join(&name);

    if !pkg_dir.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "package output not found"})),
        )
            .into_response();
    }

    let mut files: HashMap<String, String> = HashMap::new();
    if let Err(e) = collect_package_files(&pkg_dir, &pkg_dir, &mut files).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("failed to read files: {}", e)})),
        )
            .into_response();
    }

    Json(serde_json::json!(files)).into_response()
}

/// Recursively collect all files from a package directory, excluding `.cleanroom/`.
async fn collect_package_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    files: &mut HashMap<String, String>,
) -> std::io::Result<()> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().to_string();

        // Skip .cleanroom directory
        if rel_str.starts_with(".cleanroom") {
            continue;
        }

        if path.is_dir() {
            Box::pin(collect_package_files(base, &path, files)).await?;
        } else if let Ok(content) = tokio::fs::read_to_string(&path).await {
            files.insert(rel_str, content);
        }
    }
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<&mut std::io::Cursor<Vec<u8>>>,
    base: &std::path::Path,
    dir: &std::path::Path,
    options: &zip::write::SimpleFileOptions,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let name = rel.to_string_lossy().to_string();

        if path.is_dir() {
            add_dir_to_zip(zip, base, &path, options)?;
        } else if let Ok(content) = std::fs::read(&path) {
            zip.start_file(name, *options).ok();
            use std::io::Write;
            zip.write_all(&content).ok();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 1: License scanning endpoints
// ---------------------------------------------------------------------------

/// POST /api/scans — trigger a new license scan.
/// Body: `{ "path": "/absolute/or/relative/path", "offline": false }`
#[derive(Debug, Deserialize)]
struct CreateScanRequest {
    path: String,
    #[serde(default)]
    offline: bool,
    #[serde(default = "default_concurrency")]
    concurrency: usize,
}

fn default_concurrency() -> usize {
    8
}

async fn create_scan(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateScanRequest>,
) -> impl IntoResponse {
    let scan_path = std::path::PathBuf::from(&req.path);
    if !scan_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("path not found: {}", req.path)})),
        )
            .into_response();
    }

    let opts = ScanOptions {
        concurrency: req.concurrency,
        offline: req.offline,
    };

    match run_scan(&scan_path, opts).await {
        Ok(result) => {
            // Persist
            match store::save(&result) {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to persist scan result: {}", e);
                }
            }
            Json(serde_json::to_value(&result).unwrap_or_default()).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/scans — list all stored scan results (summary only).
async fn list_scans(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    match store::list_all() {
        Ok(results) => {
            let summaries: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "path": r.path,
                        "scanned_at": r.scanned_at,
                        "package_count": r.packages.len(),
                        "manifest_files": r.manifest_files.len(),
                        "sbom_files": r.sbom_files.len(),
                    })
                })
                .collect();
            Json(serde_json::json!({"scans": summaries})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/scans/{id} — get a specific scan result.
async fn get_scan(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store::load(&id) {
        Ok(result) => Json(serde_json::to_value(&result).unwrap_or_default()).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "scan not found"})),
        )
            .into_response(),
    }
}

/// GET /api/licenses — list all unique licenses found across all stored scans.
/// Query params: `?ecosystem=npm`, `?classification=permissive`
async fn list_licenses(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    match store::list_all() {
        Ok(results) => {
            use std::collections::HashMap;
            // Aggregate: license_id → { count, classification, ecosystems[] }
            let mut agg: HashMap<String, serde_json::Value> = HashMap::new();

            for scan in &results {
                for pkg in &scan.packages {
                    let key = pkg
                        .spdx_license
                        .clone()
                        .or_else(|| pkg.raw_license.clone())
                        .unwrap_or_else(|| "unknown".to_string());

                    let entry = agg.entry(key.clone()).or_insert_with(|| {
                        serde_json::json!({
                            "license": key,
                            "spdx_id": pkg.spdx_license,
                            "classification": pkg.classification,
                            "count": 0,
                            "ecosystems": [],
                        })
                    });

                    // Increment count
                    if let Some(c) = entry["count"].as_u64() {
                        entry["count"] = serde_json::json!(c + 1);
                    }

                    // Add ecosystem if not already present
                    let eco = format!("{}", pkg.ecosystem);
                    if let Some(arr) = entry["ecosystems"].as_array() {
                        if !arr.iter().any(|e| e.as_str() == Some(&eco)) {
                            let mut new_arr = arr.clone();
                            new_arr.push(serde_json::json!(eco));
                            entry["ecosystems"] = serde_json::json!(new_arr);
                        }
                    }
                }
            }

            let mut licenses: Vec<serde_json::Value> = agg.into_values().collect();
            licenses.sort_by(|a, b| {
                b["count"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["count"].as_u64().unwrap_or(0))
            });

            Json(serde_json::json!({"licenses": licenses})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
