//! Streamable HTTP transport entry point (for the MCP Inspector web UI).
//!
//! Serves the WDE MCP server at http://127.0.0.1:3001/mcp

use std::{path::PathBuf, sync::Arc};

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use wde_mcp_server::WdeServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./skills"));

    let root_for_factory = root.clone();
    let service: StreamableHttpService<WdeServer, LocalSessionManager> = StreamableHttpService::new(
        move || WdeServer::from_root(&root_for_factory).map_err(std::io::Error::other),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", service);
    let addr = "127.0.0.1:3001";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(
        "WDE MCP server (HTTP) listening on http://{addr}/mcp  (root: {})",
        root.display()
    );
    // Graceful shutdown on SIGTERM/SIGINT.
    let shutdown = async {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
        tracing::info!("shutdown signal received, draining connections");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}
