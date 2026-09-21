//! Streamable HTTP transport and server startup.

use std::{path::PathBuf, sync::Arc};

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

use crate::tools::PkbManager;

pub(crate) async fn run(root: PathBuf) -> anyhow::Result<()> {
    let manager = PkbManager::new(root);
    let service = StreamableHttpService::new(
        move || Ok(manager.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
    println!("MCP server listening on http://127.0.0.1:8000/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
