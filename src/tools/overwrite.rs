//! The `overwrite` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, paths::resolve_inside_root, staging::stage};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct OverwriteParams {
    file_path: String,
    content: String,
    if_hash: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct OverwriteOutput {
    after_hash: String,
}

#[tool_router(router = overwrite_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Overwrite the existing file at <file_path> with <content>, replacing its entire content. \
        The target must already exist; a missing path is an error and nothing is created. \
        Proceed only when <if_hash> equals the SHA-256 hex digest of the current file content; \
        otherwise leave the file unchanged and report a tool error whose text is \
        `sha mismatch, current_sha: <sha256 hex>`. \
        Return the SHA-256 hex digest of the overwritten file as structured content (`after_hash`). \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<OverwriteOutput>()
    )]
    async fn overwrite(
        &self,
        Parameters(OverwriteParams {
            file_path,
            content,
            if_hash,
        }): Parameters<OverwriteParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return Err("Unsupported <file_path>".to_owned());
        };
        let _guard = self.mutex_lock.lock().await;
        let current = match fs::read_to_string(&target).await {
            Ok(current) => current,
            Err(error) => return Err(error.to_string()),
        };
        let current_hash = sha256_hex(current.as_bytes());
        if current_hash != if_hash {
            return Err(format!("sha mismatch, current_sha: {current_hash}"));
        }
        let after_hash = sha256_hex(content.as_bytes());
        let temp = match stage(self.tmp_path.as_path(), &target, content.as_bytes()).await {
            Ok(temp) => temp,
            Err(error) => return Err(error.to_string()),
        };

        match temp.persist(&target) {
            Ok(_) => {
                let output = OverwriteOutput { after_hash };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(format!(
                    "Overwrote {file_path}"
                ))]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.error.to_string()),
        }
    }
}
