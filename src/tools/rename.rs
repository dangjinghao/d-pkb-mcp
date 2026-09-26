//! The `rename` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::hash::sha256_hex;

use super::PkbManager;
use crate::snapshot::snapshot_if_enabled;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RenameParams {
    src_path: String,
    dst_path: String,
    if_hash: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct RenameOutput {
    path: String,
}

#[tool_router(router = rename_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        name = "rename",
        description = "The configured temporary directory inside the PKB is reserved: direct access is denied. Do not remove or move its ancestors. \
        Move or rename the file or directory at <src_path> to <dst_path>, like `mv` with overwriting disabled. \
        If <dst_path> is an existing directory, place the source inside it using the source basename; \
        otherwise <dst_path> is the exact new path and its parent directory must already exist. \
        Report a tool error if the resulting destination already exists, the source is missing, \
        or a directory would be moved into itself or one of its descendants. \
        For a regular file, proceed only when <if_hash> equals the SHA-256 hex digest of the current \
        file content; otherwise leave the entry unchanged and report a tool error whose text is \
        `sha mismatch, current_sha: <sha256 hex>`. <if_hash> is required for regular-file rename and \
        ignored for directory rename. \
        Do not merge directories or overwrite existing entries. \
        Correct an accidental move by renaming the entry back. \
        Return the final root-relative path as structured content (`path`). \
        When snapshots are enabled, save the PKB state before the operation and after success. \
        If the operation fails without changing snapshotted disk content, discard its pre-operation snapshot; \
        otherwise retain it for recovery. A failed pre-operation snapshot prevents the change; a failed post-operation snapshot reports \
        that the change completed without its final snapshot. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject moving the PKB root and paths outside it.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<RenameOutput>()
    )]
    async fn rename(
        &self,
        Parameters(RenameParams {
            src_path,
            dst_path,
            if_hash,
        }): Parameters<RenameParams>,
    ) -> Result<CallToolResult, String> {
        let root = self.paths.root();
        let source = match self.paths.resolve_inside_root(&src_path) {
            Ok(path) => path,
            Err(error) => return Err(error),
        };
        if source == *self.paths.root() {
            return Err("Cannot move the PKB root".to_owned());
        }
        let dst = match self.paths.resolve_inside_root(&dst_path) {
            Ok(path) => path,
            Err(error) => return Err(error),
        };
        let _guard = self.mutex_lock.lock().await;
        if self.paths.contains_reserved_path(&source) {
            return Err(crate::paths::reserved_path_error(&source));
        }

        let source_metadata = match fs::metadata(&source).await {
            Ok(metadata) => metadata,
            Err(error) => return Err(error.to_string()),
        };

        let (destination, moved_into_dir) = match fs::metadata(&dst).await {
            Ok(metadata) if metadata.is_dir() => {
                let Some(name) = source.file_name() else {
                    return Err("Unsupported <src_path>".to_owned());
                };
                (dst.join(name), true)
            }
            _ => (dst, false),
        };

        if self.paths.is_path_reserved(&destination) {
            return Err(crate::paths::reserved_path_error(&destination));
        }
        if fs::metadata(&destination).await.is_ok() {
            return Err("Destination already exists".to_owned());
        }
        if source_metadata.is_dir() && destination.starts_with(&source) {
            return Err("Cannot move a directory into itself or one of its descendants".to_owned());
        }

        if source_metadata.is_file() {
            let Some(if_hash) = if_hash else {
                return Err("Invalid <if_hash>: required for regular-file rename".to_owned());
            };
            let content = match fs::read(&source).await {
                Ok(content) => content,
                Err(error) => return Err(error.to_string()),
            };
            let current_hash = sha256_hex(&content);
            if current_hash != if_hash {
                return Err(format!("sha mismatch, current_sha: {current_hash}"));
            }
        }

        let landed = destination.strip_prefix(root).unwrap_or(&destination);
        let before = snapshot_if_enabled(&format!("before rename: {src_path:?} -> {landed:?}"))
            .await
            .map_err(|error| {
                format!("Pre-operation snapshot failed; operation not performed: {error}")
            })?;
        match fs::rename(&source, &destination).await {
            Ok(()) => {
                snapshot_if_enabled(&format!("after rename: {src_path:?} -> {landed:?}"))
                    .await
                    .map_err(|error| {
                        format!("Rename completed, but post-operation snapshot failed: {error}")
                    })?;
                let path = landed.to_string_lossy().into_owned();
                let message = if moved_into_dir {
                    format!("Renamed {src_path} to {path}")
                } else {
                    format!("Renamed {src_path} to {dst_path}")
                };
                let output = RenameOutput { path };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(message)]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(crate::snapshot::operation_failed(before, error).await),
        }
    }
}
