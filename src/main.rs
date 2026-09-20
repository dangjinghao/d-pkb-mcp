use std::sync::Arc;

const DEFAULT_LIMIT: u32 = 1024;

use rmcp::{
    handler::server::wrapper::Parameters,
    schemars, tool, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListParams {
    dir_path: String,
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReadParams {
    file_path: String,
    limit: Option<u32>,
    start: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WriteParams {
    file_path: String,
    content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EditParams {
    file_path: String,
    old_str: String,
    new_str: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct MkdirParams {
    path: String,
    parents: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RenameParams {
    src_path: String,
    dst_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RemoveParams {
    path: String,
    recursive: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FindParams {
    glob_pattern: String,
    path: String,
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchParams {
    regex_pattern: String,
    path: String,
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UndoParams {
    path: String,
}

#[derive(Clone)]
struct PKBManager;

#[tool_router(server_handler)]
impl PKBManager {
    #[tool(
        description = "List the immediate children of <dir_path>, like `ls -l`, without recursion. \
        Return each entry's name, type, size, and modification time. \
        <limit> defaults to DEFAULT_LIMIT entries when omitted or null. Indicate whether results are truncated. \
        Relative paths are resolved from the PKB root, not a mutable working directory. \
        Paths are literal: no shell, tilde, environment-variable, or wildcard expansion. \
        Reject paths outside the PKB root and access to internal snapshots. \
        Return an empty result for an empty directory; report failures as tool errors."
    )]
    fn list(&self, Parameters(ListParams { dir_path, limit }): Parameters<ListParams>) -> String {
        // TODO
        dir_path
    }
    #[tool(
        description = "Read the text file at <file_path>, like `cat` with an optional line range. \
        <start> is a 1-based, inclusive line number and defaults to 1 when omitted or null. \
        <limit> defaults to DEFAULT_LIMIT lines when omitted or null. \
        Indicate whether results are truncated and provide the next line number when more content remains. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and access to internal snapshots. \
        Report failures as tool errors."
    )]
    fn read(
        &self,
        Parameters(ReadParams {
            file_path,
            limit,
            start,
        }): Parameters<ReadParams>,
    ) -> String {
        // TODO
        file_path
    }
    #[tool(
        description = "Write <content> to <file_path>, like shell output redirection (`>`). \
        Create the file if it does not exist, or overwrite its entire content if it does. \
        The parent directory must already exist; do not create parent directories automatically. \
        Validate the request and save a recovery snapshot before modifying the file. \
        If validation or snapshot creation fails, leave the file unchanged and report a tool error. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and access to internal snapshots."
    )]
    fn write(
        &self,
        Parameters(WriteParams { file_path, content }): Parameters<WriteParams>,
    ) -> String {
        // TODO
        file_path
    }

    #[tool(
        description = "Replace exactly one literal occurrence of <old_str> with <new_str> in <file_path>. \
        This is a literal string replacement, not a regular-expression substitution. \
        <old_str> must be nonempty and must match exactly once; otherwise leave the file unchanged \
        and report a tool error. Save a recovery snapshot before modifying the file; \
        if snapshot creation fails, leave the file unchanged and report a tool error. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and access to internal snapshots."
    )]
    fn edit(
        &self,
        Parameters(EditParams {
            file_path,
            old_str,
            new_str,
        }): Parameters<EditParams>,
    ) -> String {
        // TODO
        file_path
    }

    #[tool(description = "Create a directory at <path>, like `mkdir`. \
        <parents> defaults to false when omitted or null: the parent directory must exist \
        and an existing target is an error. With <parents>=true, behave like `mkdir -p`: \
        create missing parent directories and succeed if <path> is already a directory. \
        An existing file in place of a required directory is an error. \
        Validate the request and save a recovery snapshot before creating directories; \
        if validation or snapshot creation fails, make no changes and report a tool error. \
        Record which directories were created for undo. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and access to internal snapshots.")]
    fn mkdir(&self, Parameters(MkdirParams { path, parents }): Parameters<MkdirParams>) -> String {
        // TODO
        path
    }

    #[tool(
        name = "rename",
        description = "Move or rename the file or directory at <src_path> to <dst_path>, like `mv` with overwriting disabled. \
        If <dst_path> is an existing directory, place the source inside it using the source basename; \
        otherwise <dst_path> is the exact new path and its parent directory must already exist. \
        Report a tool error if the resulting destination already exists, the source is missing, \
        or a directory would be moved into itself or one of its descendants. \
        Do not merge directories or overwrite existing entries. \
        Validate the request and save a recovery snapshot before moving; \
        if validation or snapshot creation fails, make no changes and report a tool error. \
        Record both the source and resulting destination paths; undo uses the resulting destination path. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject moving the PKB root, paths outside it, and access to internal snapshots."
    )]
    fn rename(
        &self,
        Parameters(RenameParams { src_path, dst_path }): Parameters<RenameParams>,
    ) -> String {
        // TODO
        src_path
    }

    #[tool(description = "Remove a file or an empty directory at <path>. \
        <recursive> defaults to false when omitted or null; a nonempty directory is then an error. \
        With <recursive>=true, remove a directory and its contents, like `rm -r`. \
        A missing path is an error; there is no force mode. \
        Remove symbolic links themselves without following their targets. \
        Validate the request and save a recovery snapshot before removal; \
        if validation or snapshot creation fails, make no changes and report a tool error. \
        Preserve a recovery record for the deleted path so undo can restore it. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject removing the PKB root, paths outside it, and access to internal snapshots.")]
    fn remove(
        &self,
        Parameters(RemoveParams { path, recursive }): Parameters<RemoveParams>,
    ) -> String {
        // TODO
        path
    }

    #[tool(
        description = "Recursively find files and directories under <path>, like `find` with `-name`. \
        Match <glob_pattern> against each entry's basename, not its full relative path. \
        Only <glob_pattern> is interpreted as a glob; <path> is literal. \
        <limit> defaults to DEFAULT_LIMIT matching entries when omitted or null. \
        Return matching paths and indicate whether results are truncated. \
        No matches is a successful empty result. Report failures as tool errors. \
        Relative paths are resolved from the PKB root, with no shell expansion. \
        Reject paths outside the PKB root and exclude internal snapshots."
    )]
    fn find(
        &self,
        Parameters(FindParams {
            glob_pattern,
            path,
            limit,
        }): Parameters<FindParams>,
    ) -> String {
        // TODO
        glob_pattern
    }

    #[tool(
        description = "Search text-file contents under <path> recursively, like `grep -r -n`. \
        Interpret <regex_pattern> as a regular expression and return each matching line \
        with its file path, 1-based line number, and text. \
        <limit> defaults to DEFAULT_LIMIT matching lines in total when omitted or null, not files \
        or individual matches. Indicate whether results are truncated. \
        No matches is a successful empty result. Report invalid regular expressions and other failures \
        as tool errors. Relative paths are resolved from the PKB root. \
        Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and exclude internal snapshots."
    )]
    fn search(
        &self,
        Parameters(SearchParams {
            regex_pattern,
            path,
            limit,
        }): Parameters<SearchParams>,
    ) -> String {
        // TODO
        regex_pattern
    }

    #[tool(
        description = "Restore <path> to its state before its most recent recorded modification. \
        Use the recovery snapshot associated with that path's modification, not merely the newest \
        snapshot of the whole PKB. Restore a deleted file or directory even when <path> no longer exists. \
        For a rename, pass the resulting destination path to move the entry back to its recorded source; \
        fail if that source path is now occupied. For mkdir, remove only directories created by that \
        operation, and fail if they contain subsequently added entries. \
        Restore only paths involved in the recorded operation, leaving unrelated paths unchanged. \
        This is a snapshot recovery operation, not a POSIX command. \
        Report a tool error if no recovery record is available. \
        Relative paths are resolved from the PKB root. Paths are literal, with no shell expansion. \
        Reject paths outside the PKB root and direct access to internal snapshots."
    )]
    fn undo(&self, Parameters(UndoParams { path }): Parameters<UndoParams>) -> String {
        // TODO
        path
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = StreamableHttpService::new(
        || Ok(PKBManager),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
    println!("MCP server listening on http://127.0.0.1:8000/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
