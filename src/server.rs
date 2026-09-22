//! Streamable HTTP transport and server startup.

use std::{path::PathBuf, sync::Arc};

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

use crate::tools::PkbManager;

pub(crate) async fn run(root: PathBuf, addr: &str) -> anyhow::Result<()> {
    let manager = PkbManager::new(root);
    let service = StreamableHttpService::new(
        move || Ok(manager.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("MCP server listening on http://{addr}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
