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
        description = "Restore the **file** at <path> from a process-local Git snapshot. \
        When <snapshot> is omitted or null, use the latest Git commit changing this path \
        to identify the recorded operation and restore its before state, including a previous undo. \
        If that change records external edits rather than an operation on this path, report: \
        \"Detected snapshotted external changes; use snapshot_list to select a snapshot for explicit restoration.\" \
        Do not search past that change for an older operation. Repeated undo calls toggle \
        between the two latest states; they do not walk backward through history. \
        With <snapshot>, restore the file's state in that exact commit; use a full commit ID \
        returned by snapshot_list. This restores a version, rather than reverting that commit's changes. \
        Restore deleted files and create missing parent directories as needed. If the file is absent \
        from the selected snapshot, remove the current file. Always restore only <path>. \
        History is path-based and does not follow renames. Do not restore directories or reverse renames. \
        Leave unrelated paths unchanged. Later external edits do not prevent restoration; \
        preserve the current file state in the pre-restoration snapshot so it can be recovered. \
        Save snapshots before and after restoration so the restoration itself can be undone. \
        If the pre-operation snapshot fails, make no changes; if the post-operation snapshot fails, \
        report that restoration completed but snapshot creation failed. \
        Report an error if no recovery record exists or <snapshot> is not a known session snapshot. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    fn undo(&self, Parameters(UndoParams { path }): Parameters<UndoParams>) -> String {
        "TODO".to_owned()
    }
}
