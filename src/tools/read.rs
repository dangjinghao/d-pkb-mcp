//! The `read` tool: input schema, description, and handler.

use std::{io, path::Path};

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, serde_json, tool, tool_router,
};
use tokio::fs;

use crate::{hash::sha256_hex, paths::resolve_inside_root, tools::DEFAULT_LIMIT};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReadParams {
    file_path: String,
    limit: Option<usize>,
    start: Option<usize>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct ReadOutput {
    hash: String,
    is_truncated: bool,
}

async fn read_file(path: &Path, start: usize, limit: usize) -> io::Result<(String, bool, String)> {
    if start == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "start must be at least 1",
        ));
    }
    let file_content = fs::read_to_string(path).await?;
    let hash = sha256_hex(file_content.as_bytes());

    let lines: Vec<&str> = file_content.split_inclusive('\n').collect();
    let begin = (start - 1).min(lines.len());
    let end = if limit == 0 {
        lines.len()
    } else {
        begin.saturating_add(limit).min(lines.len())
    };
    let content = lines[begin..end].concat();
    let is_truncated = limit > 0 && end < lines.len();

    Ok((content, is_truncated, hash))
}

#[tool_router(router = read_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Read the text file at <file_path>, like `cat` with an optional line range. \
        <start> is a 1-based, inclusive line number and defaults to 1 when omitted or null. \
        <limit> defaults to DEFAULT_LIMIT lines when omitted or null; 0 means no limit. \
        Return the selected text exactly as in the file, including line endings; for a full read it \
        equals the whole file, so its SHA-256 equals `hash`. \
        Indicate whether results are truncated and the range of lines shown when more content remains. \
        As structured content, return the SHA-256 hex digest of the entire file as `hash` \
        (independent of <start> and <limit>) and the truncation state as `is_truncated`. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root. \
        Report failures as tool errors.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ReadOutput>()
    )]
    async fn read(
        &self,
        Parameters(ReadParams {
            file_path,
            start,
            limit,
        }): Parameters<ReadParams>,
    ) -> Result<CallToolResult, String> {
        let Some(target) = resolve_inside_root(self.pkb_root.as_path(), &file_path) else {
            return Err("Unsupported <file_path>".to_owned());
        };
        let start = start.unwrap_or(1);
        let limit = limit.unwrap_or(DEFAULT_LIMIT);

        match read_file(&target, start, limit).await {
            Ok((content, is_truncated, hash)) => {
                let mut text = content.clone();
                if is_truncated {
                    text.push_str(&format!(
                        "[truncated: showing line {} ~ {} ]",
                        start,
                        start + limit - 1
                    ));
                }
                let output = ReadOutput { hash, is_truncated };
                let value = match serde_json::to_value(&output) {
                    Ok(value) => value,
                    Err(error) => return Err(error.to_string()),
                };
                let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(error) => Err(error.to_string()),
        }
    }
}
