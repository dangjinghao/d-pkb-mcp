//! The `mkdir` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct MkdirParams {
    path: String,
    parents: Option<bool>,
}

#[tool_router(router = mkdir_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(description = "Create a directory at <path>, like `mkdir`. \
        <parents> defaults to false when omitted or null: the parent directory must exist \
        and an existing target is an error. With <parents>=true, behave like `mkdir -p`: \
        create missing parent directories and succeed if <path> is already a directory. \
        An existing file in place of a required directory is an error. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root.")]
    fn mkdir(&self, Parameters(MkdirParams { path, parents }): Parameters<MkdirParams>) -> String {
        "TODO".to_owned()
    }
}
