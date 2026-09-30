//! Temporary HTTP download subpaths backed by private, immutable copies.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
};

use super::PkbManager;
use crate::constants::{DEFAULT_LINK_TTL_SECS, MAX_LINK_TTL_SECS, MIN_LINK_TTL_SECS};
use crate::paths::resolve_inside_root;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DownloadLinkParams {
    path: String,
    secs: Option<u64>,
}

#[tool_router(router = download_link_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Create a temporary HTTP download subpath for the regular file at <path>. Paths are literal and relative to the PKB root; reject paths outside it. Optional <secs> defaults to 300 and must be between 1 and 3600. Return only /downloads/<token> as text, without a scheme or host. Append this subpath to the MCP server's externally reachable base URL (the endpoint URL with its trailing /mcp removed), preserving any reverse-proxy prefix. Download using HTTP GET to receive raw bytes. The link serves a private copy unaffected by later source changes. Expiry starts after copying; new requests after expiry fail, but active downloads may finish. Links are reusable until expiry. HTTP Range is not supported."
    )]
    async fn download_link(
        &self,
        Parameters(DownloadLinkParams { path, secs }): Parameters<DownloadLinkParams>,
    ) -> Result<CallToolResult, String> {
        let secs = secs.unwrap_or(DEFAULT_LINK_TTL_SECS);
        if !(MIN_LINK_TTL_SECS..=MAX_LINK_TTL_SECS).contains(&secs) {
            return Err(format!(
                "secs must be between {MIN_LINK_TTL_SECS} and {MAX_LINK_TTL_SECS}"
            ));
        }
        let target = resolve_inside_root(&self.pkb_root, &path).ok_or("Unsupported <path>")?;
        let _guard = self.mutex_lock.lock().await;
        let subpath = self
            .downloads
            .prepare(&target, &self.tmp_path, secs)
            .await
            .map_err(|error| error.to_string())?;
        Ok(CallToolResult::success(vec![ContentBlock::text(subpath)]))
    }
}
