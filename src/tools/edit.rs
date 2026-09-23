//! The `edit` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::fs;

use crate::paths::resolve_inside_root;

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
    async fn edit(
        &self,
        Parameters(EditParams {
            file_path,
            old_str,
            new_str,
        }): Parameters<EditParams>,
    ) -> String {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return "Unsupported <file_path>".to_owned();
        };
        if old_str.is_empty() {
            return "Invalid <old_str>: must not be empty".to_owned();
        }
        let content = match fs::read_to_string(&target).await {
            Ok(content) => content,
            Err(error) => return error.to_string(),
        };
        let occurrences = content.matches(&old_str).count();
        if occurrences != 1 {
            return format!(
                "Invalid <old_str>: expected exactly one occurrence, found {occurrences}"
            );
        }
        let updated = content.replacen(&old_str, &new_str, 1);

        let metadata = match fs::metadata(&target).await {
            Ok(metadata) => metadata,
            Err(error) => return error.to_string(),
        };
        let stem = target
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("tmp");
        let prefix = format!("{stem}-");
        let suffix = target
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default();
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix).suffix(&suffix);
        let temp = match builder.tempfile_in(self.tmp_path.as_path()) {
            Ok(temp) => temp,
            Err(error) => return error.to_string(),
        };
        if let Err(error) = fs::write(temp.path(), &updated).await {
            return error.to_string();
        }
        if let Err(error) = fs::set_permissions(temp.path(), metadata.permissions()).await {
            return error.to_string();
        }

        //TODO: snapshot(path)
        match temp.persist(&target) {
            Ok(_) => {
                //TODO: snapshot(path)
                format!("Edited {file_path}")
            }
            Err(error) => error.error.to_string(),
        }
    }
}
