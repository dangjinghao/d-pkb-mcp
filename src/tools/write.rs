//! The `write` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WriteParams {
    file_path: String,
    content: String,
}

#[tool_router(router = write_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Write <content> to <file_path>, like shell output redirection (`>`). \
        Create the file if it does not exist, or overwrite its entire content if it does. \
        The parent directory must already exist; do not create parent directories automatically. \
        Validate the request and save a recovery snapshot before modifying the file. \
        If validation or snapshot creation fails, leave the file unchanged and report a tool error. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    fn write(
        &self,
        Parameters(WriteParams { file_path, content }): Parameters<WriteParams>,
    ) -> String {
        "TODO".to_owned()
    }
}
