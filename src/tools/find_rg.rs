//! The `find_rg` tool: ripgrep-backed recursive file search.

use std::ffi::OsString;

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use crate::{paths::resolve_inside_root, rg, tools::DEFAULT_LIMIT};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindRgParams {
    glob_pattern: String,
    path: Option<String>,
    limit: Option<usize>,
}

fn output_text(output: rg::Output) -> String {
    let mut lines = output.lines;
    if output.truncated {
        lines.push(format!(
            "[truncated: showing first {} matches]",
            lines.len()
        ));
    }
    lines.join("\n")
}

#[tool_router(router = find_rg_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Recursively find files under <path> by running ripgrep (`rg --files`), a faster \
        alternative to `find` on large knowledge bases. \
        Match <glob_pattern> with rg's glob syntax: a pattern without `/` matches a basename at any \
        depth, while a pattern containing `/` is matched against the path relative to <path>. \
        Only <glob_pattern> is interpreted as a glob; <path> is literal and defaults to the PKB root \
        when omitted or null. \
        Like `find`, this includes hidden entries and does not honor ignore files such as \
        `.gitignore` and `.ignore`; unlike `find`, it lists files only, never directories. \
        <limit> defaults to DEFAULT_LIMIT matching entries when omitted or null; 0 means no limit. \
        Return rg's output unchanged (one path per line, relative to the PKB root) and append a \
        trailing `[truncated: ...]` line when results are truncated. \
        No matches is a successful empty result. If `rg` is not installed, return \
        \"`rg` is not available, use native tool instead\". \
        Report other failures as tool errors. Relative paths are resolved from the PKB root, \
        with no shell expansion. Reject paths outside the PKB root."
    )]
    async fn find_rg(
        &self,
        Parameters(FindRgParams {
            glob_pattern,
            path,
            limit,
        }): Parameters<FindRgParams>,
    ) -> String {
        let root = self.pkb_root.as_path();
        let Some(resolved_path) = resolve_inside_root(root, path.as_deref().unwrap_or(".")).await
        else {
            return "Unsupported <path>".to_owned();
        };

        let mut args = vec![
            OsString::from("--files"),
            OsString::from("--no-config"),
            OsString::from("--hidden"),
            OsString::from("--no-ignore"),
            OsString::from("--sort"),
            OsString::from("path"),
            OsString::from(format!("--glob={glob_pattern}")),
        ];

        let relative_path = resolved_path.strip_prefix(root).unwrap_or(&resolved_path);
        if !relative_path.as_os_str().is_empty() {
            args.push(relative_path.as_os_str().to_os_string());
        }

        match rg::run(root, args, limit.unwrap_or(DEFAULT_LIMIT)).await {
            Ok(output) => output_text(output),
            Err(e) => e,
        }
    }
}
