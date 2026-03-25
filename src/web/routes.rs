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
use crate::ParsedManifest;

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

async fn parse_manifest(
    State(_state): State<Arc<AppState>>,
    body: String,
) -> impl IntoResponse {
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
    Json(serde_json::json!({"status": "ok"}))
}

// ---------------------------------------------------------------------------
// Job creation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    manifest_content: String,
    license: Option<String>,
    isolation: Option<String>,
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> impl IntoResponse {
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

    // Spawn background task to process packages via the real pipeline
    let state_jobs = Arc::clone(&state) as Arc<AppState>;
    tokio::spawn(async move {
        let app_config = PhalusConfig::with_env_overrides(PhalusConfig::load().unwrap_or_default());

        let pipeline_config = PipelineConfig {
            license,
            output_dir: PathBuf::from("./phalus-output"),
            target_lang: None,
            isolation_mode: isolation,
            similarity_threshold: 0.70,
            concurrency: 3,
            dry_run: false,
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
            .filter(|r| {
                !r.get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
            })
            .count();
        let _ = tx.send(ProgressEvent::JobDone {
            total: results.len(),
            failed,
        });

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
