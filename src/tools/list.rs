//! The `list` tool: input schema, description, and handler.

use crate::paths::resolve_inside_root;
use crate::tools::DEFAULT_LIMIT;

use super::PkbManager;

use chrono::{DateTime, Utc};
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use std::{os::unix::fs::MetadataExt, path::Path};
use tokio::{
    fs::{self, DirEntry},
    io::Result,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListParams {
    dir_path: Option<String>,
    limit: Option<usize>,
}

async fn read_dir(path: &Path, limit: usize) -> Result<(Vec<fs::DirEntry>, bool)> {
    let mut entries = fs::read_dir(path).await?;
    let mut items = Vec::new();
    if limit == 0 {
        while let Some(entry) = entries.next_entry().await? {
            items.push(entry);
        }
        return Ok((items, false));
    }
    while items.len() < limit {
        match entries.next_entry().await? {
            Some(e) => items.push(e),
            None => break,
        }
    }
    let truncated = entries.next_entry().await?.is_some();
    Ok((items, truncated))
}

async fn entry_to_string(entry: &fs::DirEntry) -> String {
    let name = entry
        .file_name()
        .into_string()
        .unwrap_or_else(|os_str| os_str.to_string_lossy().into_owned());
    let metadata = entry.metadata().await;
    let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
    let mtime = metadata
        .as_ref()
        .ok()
        .and_then(|m| DateTime::<Utc>::from_timestamp(m.mtime(), 0))
        .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "?".to_owned());
    let is_dir = metadata.as_ref().map(|meta| meta.is_dir()).unwrap_or(false);

    format!(
        "{}\t{}\t{}\t{}",
        if is_dir { "d" } else { "-" },
        size,
        mtime,
        name
    )
}

async fn ll_style_output(entries: &[DirEntry], truncated: bool) -> String {
    let mut lines = Vec::with_capacity(entries.len());

    for entry in entries {
        lines.push(entry_to_string(entry).await);
    }
    if truncated {
        lines.push(format!(
            "[truncated: showing first {} entries]",
            lines.len()
        ));
    }
    lines.join("\n")
}

#[tool_router(router = list_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "List the immediate children of <dir_path>, like `ls -al`, without recursion. \
        <dir_path> defaults to the PKB root when omitted or null. \
        Return each entry's name, type, size, and modification time. \
        <limit> defaults to DEFAULT_LIMIT entries when omitted or null; 0 means no limit. \
        Indicate whether results are truncated. \
        Relative paths are resolved from the PKB root, not a mutable working directory. \
        Paths are literal: no shell, tilde, environment-variable, or wildcard expansion. \
        Reject paths outside the PKB root. \
        Return an empty result for an empty directory; report failures as tool errors."
    )]
    async fn list(
        &self,
        Parameters(ListParams { dir_path, limit }): Parameters<ListParams>,
    ) -> String {
        if let Some(resolved_path) =
            resolve_inside_root(self.pkb_root.as_path(), dir_path.as_deref().unwrap_or(".")).await
        {
            match read_dir(&resolved_path, limit.unwrap_or(DEFAULT_LIMIT)).await {
                Ok(entries) => ll_style_output(&entries.0, entries.1).await,
                Err(e) => e.to_string(),
            }
        } else {
            "Unsupported <dir_path>".to_owned()
        }
    }
}
