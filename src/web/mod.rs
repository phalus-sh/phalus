pub mod routes;

use std::sync::Arc;

pub async fn start_server(host: &str, port: u16) -> anyhow::Result<()> {
    let (tx, _) = tokio::sync::broadcast::channel(100);
    let state = Arc::new(routes::AppState { progress_tx: tx });
    let app = routes::router(state);

    let addr = format!("{}:{}", host, port);
    if host != "127.0.0.1" && host != "localhost" {
        eprintln!("WARNING: Server bound to {} — this is not localhost-only!", addr);
    }

    println!("PHALUS web UI running at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
