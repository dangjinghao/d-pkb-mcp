# D-PKB MCP

My personal knowledge base MCP.

Currently, it's just a filesystem MCP with:

- automatical file snapshot operation before any modification
- snapshort restore operation

## Run

```sh
cargo run -- --pkb-root /path/to/pkb
```

`--pkb-root` is required and must point to an existing directory on the server.
Relative root paths are resolved from the server's startup working directory.
Tool paths are resolved relative to this root. The MCP endpoint is
`http://127.0.0.1:8000/mcp`.
