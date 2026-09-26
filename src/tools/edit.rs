//! The `edit` tool: input schema, description, and handler.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, paths::resolve_inside_root, staging::stage};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EditParams {
    file_path: String,
    old_str: String,
    new_str: String,
    if_hash: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct EditOutput {
    after_hash: String,
    start_line: usize,
}

#[tool_router(router = edit_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Replace exactly one literal occurrence of <old_str> with <new_str> in <file_path>. \
        This is a literal string replacement, not a regular-expression substitution. \
        <old_str> must be nonempty and must match exactly once; otherwise leave the file unchanged \
        and report a tool error. \
        Proceed only when <if_hash> equals the SHA-256 hex digest of the current file content; \
        otherwise leave the file unchanged and report a tool error whose text is \
        `sha mismatch, current_sha: <sha256 hex>`. \
        Return the SHA-256 hex digest of the edited file as `after_hash` and the 1-based line \
        number where the matched <old_str> started as `start_line` in structured content. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<EditOutput>()
    )]
    async fn edit(
        &self,
        Parameters(EditParams {
            file_path,
            old_str,
            new_str,
            if_hash,
        }): Parameters<EditParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return Err("Unsupported <file_path>".to_owned());
        };
        let _guard = self.mutex_lock.lock().await;
        if old_str.is_empty() {
            return Err("Invalid <old_str>: must not be empty".to_owned());
        }
        let content = match fs::read_to_string(&target).await {
            Ok(content) => content,
            Err(error) => return Err(error.to_string()),
        };
        let current_hash = sha256_hex(content.as_bytes());
        if current_hash != if_hash {
            return Err(format!("sha mismatch, current_sha: {current_hash}"));
        }
        let mut occurrences = content.match_indices(&old_str);
        let Some((start, _)) = occurrences.next() else {
            return Err("Invalid <old_str>: expected exactly one occurrence, found 0".to_owned());
        };
        let extra = occurrences.count();
        if extra > 0 {
            return Err(format!(
                "Invalid <old_str>: expected exactly one occurrence, found {}",
                extra + 1
            ));
        }
        let start_line = content[..start].matches('\n').count() + 1;
        let updated = content.replacen(&old_str, &new_str, 1);
        let after_hash = sha256_hex(updated.as_bytes());

        let temp = match stage(self.tmp_path.as_path(), &target, updated.as_bytes()).await {
            Ok(temp) => temp,
            Err(error) => return Err(error.to_string()),
        };

        match temp.persist(&target) {
            Ok(_) => {
                let output = EditOutput {
                    after_hash,
                    start_line,
                };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(format!(
                    "Edited {file_path}"
                ))]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.error.to_string()),
        }
    }
}
