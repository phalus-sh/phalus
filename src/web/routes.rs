use axum::{
    extract::State,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;

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
) -> Json<serde_json::Value> {
    // Try to parse the body as a manifest
    let _ = body;
    Json(serde_json::json!({"status": "ok", "message": "manifest parsing endpoint"}))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
