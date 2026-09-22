//! The `search_rg` tool: ripgrep-backed recursive content search.

use std::ffi::OsString;

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};

use crate::{paths::resolve_inside_root, rg, tools::DEFAULT_LIMIT};

use super::PkbManager;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchRgParams {
    regex_pattern: String,
    path: String,
    limit: Option<usize>,
}

fn output_text(output: rg::Output) -> String {
    let mut lines = output.lines;
    if output.truncated {
        lines.push(format!(
            "[truncated: showing first {} matching lines]",
            lines.len()
        ));
    }
    lines.join("\n")
}

#[tool_router(router = search_rg_router, vis = "pub(super)")]
impl PkbManager {
    #[tool(
        description = "Search text-file contents under <path> by running ripgrep (`rg -n`), a faster \
        alternative to `search` on large knowledge bases. \
        Interpret <regex_pattern> as a regular expression and return each matching line \
        with its file path, 1-based line number, and text. \
        Like `search`, this includes hidden files and does not honor ignore files such as \
        `.gitignore` and `.ignore`; it uses rg's own regex, encoding, and binary-file rules. \
        <limit> defaults to DEFAULT_LIMIT matching lines in total when omitted or null, not files \
        or individual matches; 0 means no limit. Return rg's output unchanged (one `path:line:text` \
        match per line, with paths relative to the PKB root) and append a trailing `[truncated: ...]` \
        line when results are truncated. \
        No matches is a successful empty result. If `rg` is not installed, return \
        \"`rg` is not available, use native tool instead\". \
        Report invalid regular expressions and other failures as tool errors. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root."
    )]
    async fn search_rg(
        &self,
        Parameters(SearchRgParams {
            regex_pattern,
            path,
            limit,
        }): Parameters<SearchRgParams>,
    ) -> String {
        let root = self.pkb_root.as_path();
        let Some(resolved_path) = resolve_inside_root(root, &path).await else {
            return "Unsupported <path>".to_owned();
        };

        let mut args = vec![
            OsString::from("--line-number"),
            OsString::from("--no-heading"),
            OsString::from("--with-filename"),
            OsString::from("--color=never"),
            OsString::from("--no-config"),
            OsString::from("--hidden"),
            OsString::from("--no-ignore"),
            OsString::from("--sort"),
            OsString::from("path"),
            OsString::from("--"),
            OsString::from(regex_pattern),
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
