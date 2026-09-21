//! The `remove` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RemoveParams {
    path: String,
    recursive: Option<bool>,
}

#[tool_router(router = remove_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(description = "Remove a file or an empty directory at <path>. \
        <recursive> defaults to false when omitted or null; a nonempty directory is then an error. \
        With <recursive>=true, remove a directory and its contents, like `rm -r`. \
        A missing path is an error; there is no force mode. \
        Remove symbolic links themselves without following their targets. \
        Validate the request and save a recovery snapshot before removal; \
        if validation or snapshot creation fails, make no changes and report a tool error. \
        Preserve a recovery record for the deleted path so undo can restore it. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject removing the PKB root, paths outside it, and access to internal snapshots.")]
    fn remove(
        &self,
        Parameters(RemoveParams { path, recursive }): Parameters<RemoveParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
