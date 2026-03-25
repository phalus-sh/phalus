use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::manifest::cargo::CargoParser;
use crate::manifest::gomod::GoModParser;
use crate::manifest::npm::NpmParser;
use crate::manifest::pypi::PypiParser;
use crate::pipeline::ProgressEvent;

pub struct AppState {
    pub progress_tx: broadcast::Sender<ProgressEvent>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/manifest/parse", post(parse_manifest))
        .route("/api/health", get(health))
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}

async fn parse_manifest(
    State(_state): State<Arc<AppState>>,
    body: String,
) -> impl IntoResponse {
    // Try npm (package.json)
    if let Ok(manifest) = NpmParser::parse(&body) {
        return Json(serde_json::to_value(&manifest).unwrap()).into_response();
    }
    // Try pypi (requirements.txt)
    if let Ok(manifest) = PypiParser::parse(&body) {
        if !manifest.packages.is_empty() {
            return Json(serde_json::to_value(&manifest).unwrap()).into_response();
        }
    }
    // Try cargo (Cargo.toml)
    if let Ok(manifest) = CargoParser::parse(&body) {
        if !manifest.packages.is_empty() {
            return Json(serde_json::to_value(&manifest).unwrap()).into_response();
        }
    }
    // Try go.mod
    if let Ok(manifest) = GoModParser::parse(&body) {
        if !manifest.packages.is_empty() {
            return Json(serde_json::to_value(&manifest).unwrap()).into_response();
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "could not parse manifest"})),
    )
        .into_response()
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
