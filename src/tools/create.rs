//! The `create` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};

use crate::{hash::sha256_hex, paths::resolve_inside_root, staging::stage};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateParams {
    file_path: String,
    content: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct CreateOutput {
    after_hash: String,
}

#[tool_router(router = create_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Create a new file at <file_path> with <content>. \
        The target must not already exist; an existing file or directory is an error and is left unchanged. \
        The parent directory must already exist; do not create parent directories automatically. \
        Return the SHA-256 hex digest of the created file as structured content (`after_hash`). \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<CreateOutput>()
    )]
    async fn create(
        &self,
        Parameters(CreateParams { file_path, content }): Parameters<CreateParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return Err("Unsupported <file_path>".to_owned());
        };
        let temp = match stage(self.tmp_path.as_path(), &target, content.as_bytes()).await {
            Ok(temp) => temp,
            Err(error) => return Err(error.to_string()),
        };
        let after_hash = sha256_hex(content.as_bytes());

        //TODO: snapshot(path)
        match temp.persist_noclobber(&target) {
            Ok(_) => {
                //TODO: snapshot(path)
                let output = CreateOutput { after_hash };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(format!(
                    "Created {file_path}"
                ))]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.error.to_string()),
        }
    }
}
