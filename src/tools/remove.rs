//! The `remove` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, paths::resolve_inside_root};

use super::PkbManager;
use crate::snapshot::snapshot_if_enabled;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RemoveParams {
    path: String,
    recursive: Option<bool>,
    if_hash: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct RemoveOutput {
    path: String,
}

#[tool_router(router = remove_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(description = "Remove a file or an empty directory at <path>. \
        <recursive> defaults to false when omitted or null; a nonempty directory is then an error. \
        With <recursive>=true, remove a directory and its contents, like `rm -r`. \
        A missing path is an error; there is no force mode. \
        For a regular file, proceed only when <if_hash> equals the SHA-256 hex digest of the current \
        file content; otherwise leave the file unchanged and report a tool error whose text is \
        `sha mismatch, current_sha: <sha256 hex>`. <if_hash> is required for regular-file removal and \
        ignored for directory removal. \
        The PKB must contain only regular files and directories; behavior is undefined if symbolic links are present. \
        Return the removed root-relative path as structured content (`path`). \
        When snapshots are enabled, save the PKB state before the operation and after success. \
        A failed pre-operation snapshot prevents the change; a failed post-operation snapshot reports \
        that the change completed without its final snapshot. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject removing the PKB root and paths outside it.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<RemoveOutput>()
    )]
    async fn remove(
        &self,
        Parameters(RemoveParams {
            path,
            recursive,
            if_hash,
        }): Parameters<RemoveParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &path) else {
            return Err("Unsupported <path>".to_owned());
        };
        if target == *self.pkb_root {
            return Err("Cannot remove the PKB root".to_owned());
        }
        let _guard = self.mutex_lock.lock().await;
        let metadata = match fs::metadata(&target).await {
            Ok(metadata) => metadata,
            Err(error) => return Err(error.to_string()),
        };

        if metadata.is_file() {
            let Some(if_hash) = if_hash else {
                return Err("Invalid <if_hash>: required for regular-file removal".to_owned());
            };
            let content = match fs::read(&target).await {
                Ok(content) => content,
                Err(error) => return Err(error.to_string()),
            };
            let current_hash = sha256_hex(&content);
            if current_hash != if_hash {
                return Err(format!("sha mismatch, current_sha: {current_hash}"));
            }
        }

        snapshot_if_enabled(&format!("before remove: {path:?}"))
            .await
            .map_err(|error| {
                format!("Pre-operation snapshot failed; operation not performed: {error}")
            })?;
        let result = if metadata.is_dir() {
            if recursive.unwrap_or(false) {
                fs::remove_dir_all(&target).await
            } else {
                fs::remove_dir(&target).await
            }
        } else {
            fs::remove_file(&target).await
        };

        match result {
            Ok(()) => {
                snapshot_if_enabled(&format!("after remove: {path:?}"))
                    .await
                    .map_err(|error| {
                        format!("Remove completed, but post-operation snapshot failed: {error}")
                    })?;
                let message = format!("Removed {path}");
                let landed = target
                    .strip_prefix(self.pkb_root.as_path())
                    .unwrap_or(&target);
                let output = RemoveOutput {
                    path: landed.to_string_lossy().into_owned(),
                };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(message)]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.to_string()),
        }
    }
}
