//! stdio transport entry point.

use std::path::PathBuf;

use rmcp::ServiceExt;
use wde_mcp_server::WdeServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./skills"));

    let server = WdeServer::from_root(&root)
        .map_err(|e| format!("failed to load skills from {}: {}", root.display(), e))?;

    let transport = rmcp::transport::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
