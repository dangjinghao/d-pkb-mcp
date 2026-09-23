//! The `remove` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::fs;

use crate::paths::resolve_inside_root;

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RemoveParams {
    path: String,
    recursive: Option<bool>,
}

#[tool_router(router = remove_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(description = "Remove a file or an empty directory at <path>. \
        <recursive> defaults to false when omitted or null; a nonempty directory is then an error. \
        With <recursive>=true, remove a directory and its contents, like `rm -r`. \
        A missing path is an error; there is no force mode. \
        The PKB must contain only regular files and directories; behavior is undefined if symbolic links are present. \
        For regular-file removal, validate the request and save a Git snapshot before deletion, \
        then save another snapshot after deletion. If the pre-removal snapshot fails, make no changes. \
        If the post-removal snapshot fails, report that deletion completed but snapshot creation failed. \
        Record the file's root-relative path and its before/after snapshots so undo can restore it \
        using the original path even when the file no longer exists. \
        Directory removal, including recursive removal, has no recovery guarantee. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject removing the PKB root and paths outside it.")]
    async fn remove(
        &self,
        Parameters(RemoveParams { path, recursive }): Parameters<RemoveParams>,
    ) -> String {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &path).await else {
            return "Unsupported <path>".to_owned();
        };
        if target == *self.pkb_root {
            return "Cannot remove the PKB root".to_owned();
        }
        let metadata = match fs::metadata(&target).await {
            Ok(metadata) => metadata,
            Err(error) => return error.to_string(),
        };

        //TODO: snapshort(path)
        let result = if metadata.is_dir() {
            if recursive.unwrap_or(false) {
                fs::remove_dir_all(&target).await
            } else {
                fs::remove_dir(&target).await
            }
        } else {
            fs::remove_file(&target).await
        };
        //TODO: snapshort(path)

        match result {
            Ok(()) => format!("Removed {path}"),
            Err(error) => error.to_string(),
        }
    }
}
