//! The `create` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};

use crate::{hash::sha256_hex, staging::stage};

use super::PkbManager;
use crate::snapshot::snapshot_if_enabled;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateParams {
    file_path: String,
    content: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct CreateOutput {
    after_hash: String,
}

#[tool_router(router = create_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "The configured temporary directory inside the PKB is reserved: direct access is denied. \
        Create a new file at <file_path> with <content>. \
        The target must not already exist; an existing file or directory is an error and is left unchanged. \
        The parent directory must already exist; do not create parent directories automatically. \
        Return the SHA-256 hex digest of the created file as structured content (`after_hash`). \
        When snapshots are enabled, save the PKB state before the operation and after success. \
        A failed pre-operation snapshot prevents the change; a failed post-operation snapshot reports \
        that the change completed without its final snapshot. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<CreateOutput>()
    )]
    async fn create(
        &self,
        Parameters(CreateParams { file_path, content }): Parameters<CreateParams>,
    ) -> Result<CallToolResult, String> {
        let target = match self.paths.resolve_inside_root(&file_path) {
            Ok(path) => path,
            Err(error) => return Err(error),
        };
        let _guard = self.mutex_lock.lock().await;
        let temp = match stage(self.paths.tmp_path(), &target, content.as_bytes()).await {
            Ok(temp) => temp,
            Err(error) => return Err(error.to_string()),
        };
        let after_hash = sha256_hex(content.as_bytes());

        snapshot_if_enabled(&format!("before create: {file_path:?}"))
            .await
            .map_err(|error| {
                format!("Pre-operation snapshot failed; operation not performed: {error}")
            })?;
        match temp.persist_noclobber(&target) {
            Ok(_) => {
                snapshot_if_enabled(&format!("after create: {file_path:?}"))
                    .await
                    .map_err(|error| {
                        format!("Create completed, but post-operation snapshot failed: {error}")
                    })?;
                let output = CreateOutput { after_hash };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(format!(
                    "Created {file_path}"
                ))]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.error.to_string()),
        }
    }
}
