pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn start_server(host: &str, port: u16) -> anyhow::Result<()> {
    let (tx, _) = tokio::sync::broadcast::channel(100);
    let state = Arc::new(routes::AppState {
        progress_tx: tx,
        jobs: Mutex::new(HashMap::new()),
    });
    let app = routes::router(state);

    let addr = format!("{}:{}", host, port);
    if host != "127.0.0.1" && host != "localhost" {
        tracing::warn!("Server bound to {} -- this is not localhost-only!", addr);
    }

    println!("PHALUS web UI running at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
