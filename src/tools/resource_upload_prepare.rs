//! Prepare a bounded, conditional HTTP upload; raw bytes never enter MCP arguments.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UploadPrepareParams {
    path: String,
    full_size: u64,
    sha256: String,
    if_hash: Option<String>,
    secs: Option<u64>,
}

#[tool_router(router = resource_upload_prepare_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Prepare an HTTP upload of a text or binary file. <path> is a literal PKB-root-relative target; paths outside the root are rejected and the parent directory must exist. <full_size> is the exact byte count of the new content; <sha256> is its 64-character lowercase hexadecimal SHA-256. Omit <if_hash> to create a new file only; provide the current target SHA-256 from stat to overwrite an existing regular file only. Optional <secs> defaults to 300, range 1 through 3600. Return only /uploads/<token> as text. Append it to the externally reachable MCP server URL with the trailing /mcp removed, preserving proxy prefixes. PUT raw file bytes to that URL; do not use multipart or base64. The HTTP handler verifies size and content hash, rechecks the target under the shared operation lock, and atomically commits automatically. Success returns JSON {after_hash, full_size}. A token permits one active upload and cannot write again after success. Failed transfers may retry before expiry; no resumable uploads. New requests after expiry fail; transfers already started have a 300-second receive timeout. If the response is lost, use stat to check whether the target matches the uploaded SHA-256."
    )]
    async fn resource_upload_prepare(
        &self,
        Parameters(UploadPrepareParams {
            path,
            full_size,
            sha256,
            if_hash,
            secs,
        }): Parameters<UploadPrepareParams>,
    ) -> Result<CallToolResult, String> {
        let subpath = self
            .uploads
            .prepare(&path, full_size, sha256, if_hash, secs.unwrap_or(300))
            .await
            .map_err(|error| error.message)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(subpath)]))
    }
}
