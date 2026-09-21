//! The `search` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchParams {
    regex_pattern: String,
    path: String,
    limit: Option<usize>,
}

#[tool_router(router = search_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Search text-file contents under <path> recursively, like `grep -r -n`. \
        Interpret <regex_pattern> as a regular expression and return each matching line \
        with its file path, 1-based line number, and text. \
        <limit> defaults to DEFAULT_LIMIT matching lines in total when omitted or null, not files \
        or individual matches. Indicate whether results are truncated. \
        No matches is a successful empty result. Report invalid regular expressions and other failures \
        as tool errors. Relative paths are resolved from the PKB root. \
        Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and exclude internal snapshots."
    )]
    fn search(
        &self,
        Parameters(SearchParams {
            regex_pattern,
            path,
            limit,
        }): Parameters<SearchParams>,
    ) -> String {
        // TODO
        regex_pattern
    }
}
