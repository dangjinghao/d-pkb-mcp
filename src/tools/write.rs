//! The `write` tool: input schema, description, and handler.

use std::{fs::Permissions, os::unix::fs::PermissionsExt};

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::fs;

use crate::paths::resolve_inside_root;

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
        Validate the request and save a recovery snapshot before modifying the file, \
        then save another snapshot after writing. \
        If validation or the pre-write snapshot fails, leave the file unchanged and report a tool error. \
        If the post-write snapshot fails, report that the write completed but snapshot creation failed. \
        The post-write snapshot records the file's on-disk state after the write, including any \
        third-party writes that raced with this operation, so undo restores the actual resulting state. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    async fn write(
        &self,
        Parameters(WriteParams { file_path, content }): Parameters<WriteParams>,
    ) -> String {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return "Unsupported <file_path>".to_owned();
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
        let existing = fs::metadata(&target)
            .await
            .ok()
            .filter(|metadata| metadata.is_file());
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix).suffix(&suffix);
        if existing.is_none() {
            builder.permissions(Permissions::from_mode(0o666));
        }
        let temp = match builder.tempfile_in(self.tmp_path.as_path()) {
            Ok(temp) => temp,
            Err(error) => return error.to_string(),
        };
        if let Err(error) = fs::write(temp.path(), &content).await {
            return error.to_string();
        }
        if let Some(metadata) = &existing {
            if let Err(error) = fs::set_permissions(temp.path(), metadata.permissions()).await {
                return error.to_string();
            }
        }

        //TODO: snapshot(path)
        match temp.persist(&target) {
            Ok(_) => {
                //TODO: snapshot(path)
                format!("Wrote {file_path}")
            }
            Err(error) => error.error.to_string(),
        }
    }
}
