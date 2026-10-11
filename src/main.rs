//! stdio transport entry point.

use std::path::PathBuf;

use rmcp::ServiceExt;
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

    let server = WdeServer::from_root(&root)
        .map_err(|e| format!("failed to load skills from {}: {}", root.display(), e))?;

    tracing::info!("WDE MCP server (stdio) starting (root: {})", root.display());

    let transport = rmcp::transport::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
