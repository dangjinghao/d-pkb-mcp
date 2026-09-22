//! The `read` tool: input schema, description, and handler.

use std::{io, path::Path};

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader},
};

use crate::{paths::resolve_inside_root, tools::DEFAULT_LIMIT};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReadParams {
    file_path: String,
    limit: Option<usize>,
    start: Option<usize>,
}

async fn read_file(path: &Path, start: usize, limit: usize) -> io::Result<Vec<String>> {
    if start == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "start must be at least 1",
        ));
    }
    let file = File::open(path).await?;
    let mut lines = BufReader::new(file).lines();
    let mut output = Vec::new();

    // skip first <start> lines
    for _ in 1..start {
        if lines.next_line().await?.is_none() {
            return Ok(output);
        }
    }
    if limit == 0 {
        while let Some(line) = lines.next_line().await? {
            output.push(line);
        }
    } else {
        while output.len() < limit {
            match lines.next_line().await? {
                Some(line) => output.push(line),
                None => break,
            }
        }
        if lines.next_line().await?.is_some() {
            output.push(format!(
                "[truncated: showing line {} ~ {} ]",
                start,
                start + limit - 1
            ));
        }
    }

    Ok(output)
}

#[tool_router(router = read_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Read the text file at <file_path>, like `cat` with an optional line range. \
        <start> is a 1-based, inclusive line number and defaults to 1 when omitted or null. \
        <limit> defaults to DEFAULT_LIMIT lines when omitted or null; 0 means no limit. \
        Indicate whether results are truncated and provide the next line number when more content remains. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root. \
        Report failures as tool errors."
    )]
    async fn read(
        &self,
        Parameters(ReadParams {
            file_path,
            start,
            limit,
        }): Parameters<ReadParams>,
    ) -> String {
        if let Some(resolved_path) = resolve_inside_root(self.pkb_root.as_path(), &file_path).await
        {
            match read_file(
                &resolved_path,
                start.unwrap_or(1),
                limit.unwrap_or(DEFAULT_LIMIT),
            )
            .await
            {
                Ok(lines) => lines.join("\n"),
                Err(e) => e.to_string(),
            }
        } else {
            "Unsupported <file_path>".to_owned()
        }
    }
}
