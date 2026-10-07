//! Streamable HTTP transport and server startup.

use std::{path::PathBuf, sync::Arc};

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

use crate::{
    downloads::{Downloads, download},
    tools::PkbManager,
    transfers::TransferQuota,
    uploads::resource_upload,
};

pub(crate) async fn run(
    root: PathBuf,
    tmp_path: PathBuf,
    addr: &str,
    quota: u64,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let quota = Arc::new(TransferQuota::new(quota)?);
    let downloads = Arc::new(Downloads::new(quota.clone()));
    let manager = PkbManager::new(root, tmp_path, downloads.clone(), quota);
    let uploads = manager.uploads.clone();
    let service = StreamableHttpService::new(
        move || Ok(manager.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().disable_allowed_hosts(),
    );

    let download_routes = axum::Router::new()
        .route("/downloads/{token}", axum::routing::get(download))
        .with_state(downloads);

    let upload_routes = axum::Router::new()
        .route("/uploads/{token}", axum::routing::put(resource_upload))
        .with_state(uploads);

    let router = axum::Router::new()
        .merge(download_routes)
        .merge(upload_routes)
        .nest_service("/mcp", service);

    println!("MCP server listening on http://{addr}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
