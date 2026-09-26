//! The `mkdir` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
};
use tokio::fs;

use crate::paths::resolve_inside_root;

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
        Report failures as tool errors. Reject paths outside the PKB root.")]
    async fn mkdir(
        &self,
        Parameters(MkdirParams { path, parents }): Parameters<MkdirParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &path) else {
            return Err("Unsupported <path>".to_owned());
        };
        let _guard = self.mutex_lock.lock().await;
        let result = if parents.unwrap_or(false) {
            fs::create_dir_all(&target).await
        } else {
            fs::create_dir(&target).await
        };
        match result {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                "Created {path}"
            ))])),
            Err(error) => Err(error.to_string()),
        }
    }
}
