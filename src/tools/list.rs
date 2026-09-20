//! The `list` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListParams {
    dir_path: String,
    limit: Option<u32>,
}

#[tool_router(router = list_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "List the immediate children of <dir_path>, like `ls -l`, without recursion. \
        Return each entry's name, type, size, and modification time. \
        <limit> defaults to DEFAULT_LIMIT entries when omitted or null. Indicate whether results are truncated. \
        Relative paths are resolved from the PKB root, not a mutable working directory. \
        Paths are literal: no shell, tilde, environment-variable, or wildcard expansion. \
        Reject paths outside the PKB root and access to internal snapshots. \
        Return an empty result for an empty directory; report failures as tool errors."
    )]
    fn list(&self, Parameters(ListParams { dir_path, limit }): Parameters<ListParams>) -> String {
        // TODO
        dir_path
    }
}
