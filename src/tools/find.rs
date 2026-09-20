//! The `find` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindParams {
    glob_pattern: String,
    path: String,
    limit: Option<u32>,
}

#[tool_router(router = find_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Recursively find files and directories under <path>, like `find` with `-name`. \
        Match <glob_pattern> against each entry's basename, not its full relative path. \
        Only <glob_pattern> is interpreted as a glob; <path> is literal. \
        <limit> defaults to DEFAULT_LIMIT matching entries when omitted or null. \
        Return matching paths and indicate whether results are truncated. \
        No matches is a successful empty result. Report failures as tool errors. \
        Relative paths are resolved from the PKB root, with no shell expansion. \
        Reject paths outside the PKB root and exclude internal snapshots."
    )]
    fn find(
        &self,
        Parameters(FindParams {
            glob_pattern,
            path,
            limit,
        }): Parameters<FindParams>,
    ) -> String {
        // TODO
        glob_pattern
    }
}
