//! The `edit` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EditParams {
    file_path: String,
    old_str: String,
    new_str: String,
}

#[tool_router(router = edit_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Replace exactly one literal occurrence of <old_str> with <new_str> in <file_path>. \
        This is a literal string replacement, not a regular-expression substitution. \
        <old_str> must be nonempty and must match exactly once; otherwise leave the file unchanged \
        and report a tool error. Save a recovery snapshot before modifying the file; \
        if snapshot creation fails, leave the file unchanged and report a tool error. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    fn edit(
        &self,
        Parameters(EditParams {
            file_path,
            old_str,
            new_str,
        }): Parameters<EditParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
