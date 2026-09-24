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

Arrange the host directories under one parent, for example:

```text
/srv/pkb-data/
├── D/       # PKB files
└── .tmp/    # Temporary files used while writing
```

Run the container with that **one parent directory** mounted at `/data`:

```sh
docker run --rm --name d-pkb-mcp \
  --mount type=bind,src=/srv/pkb-data,dst=/data \
  -p 127.0.0.1:8000:8000 \
  d-pkb-mcp:local
```

The image starts with `--pkb-root /data/D --tmp-path /data/.tmp --addr 0.0.0.0:8000`.
Change the host path to the parent containing your PKB and temporary directory.
Both directories must be **in the same bind mount** : file updates stage content in
`/data/.tmp` and rename it into `/data/D`, which fails across mount points. The
server creates `.tmp` if it does not exist.

On a standard rootful Linux Docker daemon, the container runs as root by default,
so a root-owned PKB does not require a UID/GID mapping. Files created by the service
will be root-owned on the host. Rootless Docker or Docker user namespace remapping
changes that mapping; the mapped host user must have access to the bind mount.
The example publishes the port only on the host's loopback interface. For access
from another machine, use a private network or tunnel and bind the published port
to the appropriate host interface; this server does not authenticate MCP callers.
