# D-PKB MCP

My personal knowledge base MCP.

Currently, it's just a genernal filesystem MCP with:

- automatical file snapshot operation before any modification
- snapshort restore operation

## Tools

- `list <dir-path> [limit]`
    List directory with metadata.
- `read <file-path> [limit] [start]`
    Read file content at most `[limit]` lines. If `[start]` is presented it will started at `[start]` line.
- `write <file-path> <content>`
    Create or overwrite a file.
- `edit <path> <old-str> <new-str>`
    Edit file based on string replace. If more than one string is matched, should be failed to return.
- `find <glob-pattern> <path> [limit]`
    Find file name with `<glob-pattern>` in `<path>`.
- `search <regex-pattern> <path> [limit]`
    Find file content with `<regex-pattern>` in `<path>`.
- `undo <path>`
    Undo the previous `<path>` file modification.
