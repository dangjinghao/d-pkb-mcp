//! The `undo` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UndoParams {
    path: String,
}

#[tool_router(router = undo_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Restore <path> to its state before its most recent recorded modification. \
        Use the recovery snapshot associated with that path's modification, not merely the newest \
        snapshot of the whole PKB. Restore a deleted file or directory even when <path> no longer exists. \
        For a rename, pass the resulting destination path to move the entry back to its recorded source; \
        fail if that source path is now occupied. For mkdir, remove only directories created by that \
        operation, and fail if they contain subsequently added entries. \
        Restore only paths involved in the recorded operation, leaving unrelated paths unchanged. \
        This is a snapshot recovery operation, not a POSIX command. \
        Report a tool error if no recovery record is available. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    fn undo(&self, Parameters(UndoParams { path }): Parameters<UndoParams>) -> String {
        "TODO".to_owned()
    }
}
