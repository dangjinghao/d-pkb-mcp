//! The `write` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use crate::{paths::resolve_inside_root, staging::stage};

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

        let temp = match stage(self.tmp_path.as_path(), &target, content.as_bytes()).await {
            Ok(temp) => temp,
            Err(error) => return error.to_string(),
        };

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
