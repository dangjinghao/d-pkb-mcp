# D-PKB MCP

Filesystem MCP for my personal knowledge base. Snapshot and undo support is not implemented yet.

## Run

```sh
d-pkb-mcp --pkb-root /path/to/pkb
```

`--pkb-root` defaults to `.` and must point to an existing directory on the server.
Relative root paths are resolved from the server's startup working directory.
Tool paths are resolved relative to this root. The MCP endpoint is
`http://127.0.0.1:8000/mcp`.

## Docker

Build the image:

```sh
docker build -t d-pkb-mcp:local .
```

Mount the PKB volume directly at `/data`:

```sh
docker run --rm --name d-pkb-mcp \
  -v content:/data \
  -p 127.0.0.1:8000:8000 \
  d-pkb-mcp:local
```

`content` is a Docker named volume. To use an existing host directory, replace it
with a path, for example `-v /srv/pkb/D:/data`.

The image starts with `--pkb-root /data --tmp-path /data/.tmp --addr 0.0.0.0:8000`.
The server creates `/data/.tmp` if needed and stages writes there before renaming them into the PKB. This keeps both paths in the same mount, which is required for atomic replacement.

On a standard rootful Linux Docker daemon, the container runs as root by default,
so a root-owned PKB does not require a UID/GID mapping. Files created by the service
will be root-owned on the host. Rootless Docker or Docker user namespace remapping
changes that mapping; the mapped host user must have access to the bind mount.
The example publishes the port only on the host's loopback interface. For access
from another machine, use a private network or tunnel and bind the published port
to the appropriate host interface; this server does not authenticate MCP callers.
