//! The `read` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReadParams {
    file_path: String,
    limit: Option<u32>,
    start: Option<u32>,
}

#[tool_router(router = read_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Read the text file at <file_path>, like `cat` with an optional line range. \
        <start> is a 1-based, inclusive line number and defaults to 1 when omitted or null. \
        <limit> defaults to DEFAULT_LIMIT lines when omitted or null. \
        Indicate whether results are truncated and provide the next line number when more content remains. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and access to internal snapshots. \
        Report failures as tool errors."
    )]
    fn read(
        &self,
        Parameters(ReadParams {
            file_path,
            limit,
            start,
        }): Parameters<ReadParams>,
    ) -> String {
        // TODO
        file_path
    }
}
