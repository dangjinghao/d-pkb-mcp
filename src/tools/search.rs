//! The `search` tool: input schema, description, and handler.

use std::path::{Path, PathBuf};

use regex::Regex;
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use tokio::{
    fs::File,
    io::{self, AsyncBufReadExt, BufReader},
};

use crate::{
    paths::{Walker, resolve_inside_root},
    tools::DEFAULT_LIMIT,
};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchParams {
    regex_pattern: String,
    path: Option<String>,
    limit: Option<usize>,
}

struct LineMatch {
    path: PathBuf,
    line_number: usize,
    text: String,
}

async fn collect_matches(
    start: PathBuf,
    regex: &Regex,
    limit: usize,
) -> io::Result<(Vec<LineMatch>, bool)> {
    let mut matches = Vec::new();
    let mut walker = Walker::new(start).await?;

    while let Some(path) = walker.next().await? {
        let Ok(file) = File::open(&path).await else {
            continue;
        };
        let mut lines = BufReader::new(file).lines();
        let mut line_number = 0;

        while let Ok(Some(line)) = lines.next_line().await {
            line_number += 1;
            if !regex.is_match(&line) {
                continue;
            }
            if limit > 0 && matches.len() >= limit {
                return Ok((matches, true));
            }
            matches.push(LineMatch {
                path: path.clone(),
                line_number,
                text: line,
            });
        }
    }

    Ok((matches, false))
}

fn format_matches(root: &Path, matches: &[LineMatch], truncated: bool) -> String {
    let mut lines: Vec<String> = matches
        .iter()
        .map(|line_match| {
            format!(
                "{}:{}:{}",
                line_match
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&line_match.path)
                    .to_string_lossy(),
                line_match.line_number,
                line_match.text
            )
        })
        .collect();
    if truncated {
        lines.push(format!(
            "[truncated: showing first {} matching lines]",
            matches.len()
        ));
    }
    lines.join("\n")
}

#[tool_router(router = search_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Search text-file contents under <path> recursively, like `grep -r -n`. \
        Interpret <regex_pattern> as a regular expression and return each matching line \
        with its file path, 1-based line number, and text. \
        <limit> defaults to DEFAULT_LIMIT matching lines in total when omitted or null, not files \
        or individual matches; 0 means no limit. Indicate whether results are truncated. \
        No matches is a successful empty result. \
        Skip files that cannot be opened, and stop reading a file at its first non-UTF-8 line. \
        Report invalid regular expressions and other failures as tool errors. \
        Relative paths are resolved from the PKB root. \
        <path> defaults to the PKB root when omitted or null. \
        Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    async fn search(
        &self,
        Parameters(SearchParams {
            regex_pattern,
            path,
            limit,
        }): Parameters<SearchParams>,
    ) -> String {
        let regex = match Regex::new(&regex_pattern) {
            Ok(regex) => regex,
            Err(e) => return format!("Invalid <regex_pattern>: {e}"),
        };
        let root = self.pkb_root.as_path();
        let Some(resolved_path) = resolve_inside_root(root, path.as_deref().unwrap_or(".")) else {
            return "Unsupported <path>".to_owned();
        };

        match collect_matches(resolved_path, &regex, limit.unwrap_or(DEFAULT_LIMIT)).await {
            Ok((matches, truncated)) => format_matches(root, &matches, truncated),
            Err(e) => e.to_string(),
        }
    }
}
