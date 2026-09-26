//! The `snapshot_list` tool: input schema, description, and handler.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use crate::snapshot;

use super::{DEFAULT_LIMIT, PkbManager};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SnapshotListParams {
    limit: Option<usize>,
}

#[tool_router(router = snapshot_list_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "List commits in the current process's snapshot repository, like `git log --oneline`. \
        Return one TSV line per commit: full commit ID followed by its one-line title, newest first \
        in commit-history order. Do not include commit message bodies. \
        <limit> defaults to DEFAULT_LIMIT when omitted or null; 0 means no limit. \
        Append [truncated: showing first N snapshots] if more commits exist. \
        Return an empty result if no commits exist, or error text if snapshots are not initialized."
    )]
    async fn snapshot_list(
        &self,
        Parameters(SnapshotListParams { limit }): Parameters<SnapshotListParams>,
    ) -> String {
        match snapshot::list(limit.unwrap_or(DEFAULT_LIMIT)).await {
            Ok((entries, truncated)) => {
                let mut lines: Vec<_> = entries
                    .into_iter()
                    .map(|(id, title)| format!("{id}\t{title}"))
                    .collect();
                if truncated {
                    lines.push(format!(
                        "[truncated: showing first {} snapshots]",
                        lines.len()
                    ));
                }
                lines.join("\n")
            }
            Err(error) => error.to_string(),
        }
    }
}
