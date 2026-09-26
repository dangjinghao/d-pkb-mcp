//! The `undo` tool: input schema, description, and handler.

use std::io::ErrorKind;

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, snapshot, staging::stage};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UndoParams {
    path: String,
    if_hash: Option<String>,
    snapshot: Option<String>,
}

#[tool_router(router = undo_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "The configured temporary directory inside the PKB is reserved: direct access is denied. \
        Restore the file at <path> from a process-local Git snapshot. \
        For an existing file, <if_hash> is required and must match its current SHA-256 digest; \
        otherwise leave it unchanged and report a tool error. A hash mismatch reports \
        `sha mismatch, current_sha: <sha256 hex>`. For a missing path, <if_hash> is ignored. \
        When <snapshot> is omitted or null, find the latest recorded commit that changed \
        <path> and restore the path's state from its parent commit. Select the restore target \
        before taking the pre-restoration snapshot. \
        With <snapshot>, restore the state in that exact commit, using a full commit ID \
        returned by snapshot_list. \
        Restore only <path>, creating missing parent directories as needed. If the file \
        is absent from the selected state, remove the current file. Reject directories \
        and the PKB root. History follows paths, not renames. \
        Stage restored content in the temporary directory, then move it over the target. \
        Save snapshots before and after restoration, preserving the current disk state, \
        including external edits. A failed pre-restoration snapshot prevents changes; \
        a failed post-restoration snapshot reports that restoration completed without its final snapshot. \
        Report an error if snapshots are disabled, no previous state is available for default undo, \
        or <snapshot> is not a known session snapshot. Paths excluded from snapshots cannot be restored. \
        Paths are literal and relative to the PKB root. Reject paths outside it."
    )]
    async fn undo(
        &self,
        Parameters(UndoParams {
            path,
            if_hash,
            snapshot: selected,
        }): Parameters<UndoParams>,
    ) -> Result<CallToolResult, String> {
        let root = self.paths.root();
        let target = self.paths.resolve_inside_root(&path)?;
        if target == *self.paths.root() {
            return Err("Cannot restore the PKB root".to_owned());
        }
        let _guard = self.mutex_lock.lock().await;
        let exists = match fs::metadata(&target).await {
            Ok(metadata) if metadata.is_file() => true,
            Ok(_) => return Err("Cannot restore a directory".to_owned()),
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error.to_string()),
        };
        if exists {
            let expected = if_hash
                .ok_or_else(|| "Invalid <if_hash>: required for existing-file undo".to_owned())?;
            let content = fs::read(&target).await.map_err(|error| error.to_string())?;
            let current_hash = sha256_hex(&content);
            if current_hash != expected {
                return Err(format!("sha mismatch, current_sha: {current_hash}"));
            }
        }
        let relative = target
            .strip_prefix(root)
            .map_err(|error| error.to_string())?;
        let content = snapshot::restore_content(relative, selected.as_deref())
            .await
            .map_err(|error| error.to_string())?;
        let temp = match content {
            Some(content) => Some(
                stage(self.paths.tmp_path(), &target, &content)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            None => None,
        };
        snapshot::snapshot(&format!("before undo: {relative:?}"))
            .await
            .map_err(|error| {
                format!("Pre-restoration snapshot failed; operation not performed: {error}")
            })?;
        if let Some(temp) = temp {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            temp.persist(&target)
                .map_err(|error| error.error.to_string())?;
        } else if exists {
            fs::remove_file(&target)
                .await
                .map_err(|error| error.to_string())?;
        }
        snapshot::snapshot(&format!("after undo: {relative:?}"))
            .await
            .map_err(|error| {
                format!("Restoration completed, but post-restoration snapshot failed: {error}")
            })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Restored {path}"
        ))]))
    }
}
