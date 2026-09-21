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
        Validate the request and save a recovery snapshot before moving; \
        if validation or snapshot creation fails, make no changes and report a tool error. \
        Record both the source and resulting destination paths; undo uses the resulting destination path. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject moving the PKB root, paths outside it, and access to internal snapshots."
    )]
    fn rename(
        &self,
        Parameters(RenameParams { src_path, dst_path }): Parameters<RenameParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
