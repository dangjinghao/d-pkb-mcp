//! The `find` tool: input schema, description, and handler.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::io;

use crate::{
    paths::{Walker, resolve_inside_root},
    tools::DEFAULT_LIMIT,
};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindParams {
    glob_pattern: String,
    path: Option<String>,
    limit: Option<usize>,
}

async fn collect_matches(
    start: PathBuf,
    matcher: &GlobMatcher,
    limit: usize,
) -> io::Result<(Vec<PathBuf>, bool)> {
    let mut matches = Vec::new();
    let mut truncated = false;
    let mut walker = Walker::new(start).await?;

    while let Some(path) = walker.next().await? {
        let matched = path.file_name().is_some_and(|name| matcher.is_match(name));
        if matched {
            if limit > 0 && matches.len() >= limit {
                truncated = true;
                break;
            }
            matches.push(path);
        }
    }

    Ok((matches, truncated))
}

fn format_matches(root: &Path, matches: &[PathBuf], truncated: bool) -> String {
    let mut lines: Vec<String> = matches
        .iter()
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    if truncated {
        lines.push(format!(
            "[truncated: showing first {} matches]",
            matches.len()
        ));
    }
    lines.join("\n")
}

#[tool_router(router = find_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Recursively find files and directories under <path>, like `find` with `-name`. \
        Match <glob_pattern> against each entry's basename, not its full relative path. \
        Only <glob_pattern> is interpreted as a glob; <path> is literal and defaults to the PKB root \
        when omitted or null. \
        <limit> defaults to DEFAULT_LIMIT matching entries when omitted or null; 0 means no limit. \
        Return matching paths and indicate whether results are truncated. \
        No matches is a successful empty result. Report failures as tool errors. \
        Relative paths are resolved from the PKB root, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    async fn find(
        &self,
        Parameters(FindParams {
            glob_pattern,
            path,
            limit,
        }): Parameters<FindParams>,
    ) -> String {
        let matcher = match Glob::new(&glob_pattern) {
            Ok(glob) => glob.compile_matcher(),
            Err(e) => return format!("Invalid <glob_pattern>: {e}"),
        };
        let root = self.pkb_root.as_path();
        let Some(resolved_path) = resolve_inside_root(root, path.as_deref().unwrap_or(".")).await
        else {
            return "Unsupported <path>".to_owned();
        };

        match collect_matches(resolved_path, &matcher, limit.unwrap_or(DEFAULT_LIMIT)).await {
            Ok((matches, truncated)) => format_matches(root, &matches, truncated),
            Err(e) => e.to_string(),
        }
    }
}
