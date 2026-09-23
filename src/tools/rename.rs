//! The `rename` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, paths::resolve_inside_root};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RenameParams {
    src_path: String,
    dst_path: String,
    if_hash: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
enum RenameOutput {
    Renamed { path: String }, // it is not a good return
    HashMismatch { current_hash: String },
}

#[tool_router(router = rename_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        name = "rename",
        description = "Move or rename the file or directory at <src_path> to <dst_path>, like `mv` with overwriting disabled. \
        If <dst_path> is an existing directory, place the source inside it using the source basename; \
        otherwise <dst_path> is the exact new path and its parent directory must already exist. \
        Report a tool error if the resulting destination already exists, the source is missing, \
        or a directory would be moved into itself or one of its descendants. \
        For a regular file, proceed only when <if_hash> equals the SHA-256 hex digest of the current \
        file content; otherwise leave the entry unchanged and report a tool error with the current hash \
        as structured content (`current_hash`). <if_hash> is required for regular-file rename and \
        ignored for directory rename. \
        Do not merge directories or overwrite existing entries. \
        Correct an accidental move by renaming the entry back. Later full-PKB snapshots may capture the move. \
        Snapshot history is path-based; the destination does not inherit the source's undo history. \
        To recover an older file version, move it back and consult its original path's history, \
        or use snapshot_list and an explicit undo snapshot to restore the original path. \
        Explicit restoration can recreate the original file without a placeholder and leaves \
        the destination untouched; it does not reverse the rename. \
        Return the final root-relative path as structured content (`path`). \
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
        let root = self.pkb_root.as_path();
        let Some(source) = resolve_inside_root(root, &src_path) else {
            return Err("Unsupported <src_path>".to_owned());
        };
        if source == *self.pkb_root {
            return Err("Cannot move the PKB root".to_owned());
        }
        let Some(dst) = resolve_inside_root(root, &dst_path) else {
            return Err("Unsupported <dst_path>".to_owned());
        };

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
                let message = format!(
                    "Invalid <if_hash>: does not match the current file content (current hash: {current_hash})"
                );
                let output = RenameOutput::HashMismatch { current_hash };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
                result.structured_content = Some(value);
                return Ok(result);
            }
        }

        //TODO: snapshot(path)
        match fs::rename(&source, &destination).await {
            Ok(()) => {
                //TODO: snapshot(path)
                let landed = destination.strip_prefix(root).unwrap_or(&destination);
                let path = landed.to_string_lossy().into_owned();
                let message = if moved_into_dir {
                    format!("Renamed {src_path} to {path}")
                } else {
                    format!("Renamed {src_path} to {dst_path}")
                };
                let output = RenameOutput::Renamed { path };
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
