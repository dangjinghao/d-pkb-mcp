//! The `rename` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::fs;

use crate::paths::resolve_inside_root;

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RenameParams {
    src_path: String,
    dst_path: String,
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
        Do not merge directories or overwrite existing entries. \
        Do not create snapshot commits or an undo record for this operation. \
        Correct an accidental move by renaming the entry back. Later full-PKB snapshots may capture the move. \
        Snapshot history is path-based; the destination does not inherit the source's undo history. \
        To recover an older file version, move it back and consult its original path's history, \
        or use snapshot_list and an explicit undo snapshot to restore the original path. \
        Explicit restoration can recreate the original file without a placeholder and leaves \
        the destination untouched; it does not reverse the rename. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject moving the PKB root and paths outside it."
    )]
    async fn rename(
        &self,
        Parameters(RenameParams { src_path, dst_path }): Parameters<RenameParams>,
    ) -> String {
        let root = self.pkb_root.as_path();
        let Some(source) = resolve_inside_root(root, &src_path) else {
            return "Unsupported <src_path>".to_owned();
        };
        if source == *self.pkb_root {
            return "Cannot move the PKB root".to_owned();
        }
        let Some(dst) = resolve_inside_root(root, &dst_path) else {
            return "Unsupported <dst_path>".to_owned();
        };

        let source_metadata = match fs::metadata(&source).await {
            Ok(metadata) => metadata,
            Err(error) => return error.to_string(),
        };

        let (destination, moved_into_dir) = match fs::metadata(&dst).await {
            Ok(metadata) if metadata.is_dir() => {
                let Some(name) = source.file_name() else {
                    return "Unsupported <src_path>".to_owned();
                };
                (dst.join(name), true)
            }
            _ => (dst, false),
        };

        if fs::metadata(&destination).await.is_ok() {
            return "Destination already exists".to_owned();
        }
        if source_metadata.is_dir() && destination.starts_with(&source) {
            return "Cannot move a directory into itself or one of its descendants".to_owned();
        }

        //TODO: snapshot(path)
        match fs::rename(&source, &destination).await {
            Ok(()) => {
                let ret_str;
                if moved_into_dir {
                    let landed = destination.strip_prefix(root).unwrap_or(&destination);
                    ret_str = format!("Renamed {src_path} to {}", landed.display());
                } else {
                    ret_str = format!("Renamed {src_path} to {dst_path}");
                }

                //TODO: snapshot(path)
                ret_str
            }
            Err(error) => error.to_string(),
        }
    }
}
