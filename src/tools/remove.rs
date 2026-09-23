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
        For regular-file removal, validate the request and save a Git snapshot before deletion, \
        then save another snapshot after deletion. If the pre-removal snapshot fails, make no changes. \
        If the post-removal snapshot fails, report that deletion completed but snapshot creation failed. \
        Record the file's root-relative path and its before/after snapshots so undo can restore it \
        using the original path even when the file no longer exists. \
        Directory removal, including recursive removal, and symbolic-link removal have no recovery guarantee. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject removing the PKB root and paths outside it.")]
    fn remove(
        &self,
        Parameters(RemoveParams { path, recursive }): Parameters<RemoveParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
