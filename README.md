# D-PKB MCP

Filesystem MCP for my personal knowledge base.

## Tools

| Tool | Description |
| --- | --- |
| `list` | List a directory's immediate children. |
| `find` | Recursively find files and directories by name. |
| `find_rg` | Find files using ripgrep. |
| `search` | Recursively search text-file contents. |
| `search_rg` | Search text-file contents using ripgrep. |
| `read` | Read text with an optional line range. |
| `bin_read` | Read up to 64 KiB of bytes at an offset, returning base64 and the byte count. |
| `stat` | Return a file's SHA-256, total size, and modification time. |
| `create` | Create a new text file. |
| `overwrite` | Replace a text file's content when its SHA-256 matches. |
| `edit` | Replace one unique text occurrence when the file's SHA-256 matches. |
| `mkdir` | Create a directory. |
| `rename` | Move or rename a file or directory without overwriting the destination. |
| `remove` | Remove a file or an empty directory. |
| `download_link` | Return a temporary HTTP download subpath for a file copy. |
| `resource_upload_prepare` | Return a temporary HTTP PUT subpath for creating or conditionally replacing a file. |
