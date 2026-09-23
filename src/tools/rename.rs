//! The `rename` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

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
    fn rename(
        &self,
        Parameters(RenameParams { src_path, dst_path }): Parameters<RenameParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
