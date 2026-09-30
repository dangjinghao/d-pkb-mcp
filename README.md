# D-PKB MCP

Filesystem MCP for my personal knowledge base.

## Run

```sh
d-pkb-mcp --pkb-root /path/to/pkb
```

`--pkb-root` defaults to `.` and must point to an existing directory on the server.
Relative root paths are resolved from the server's startup working directory.
Tool paths are resolved relative to this root. The MCP endpoint is
`http://127.0.0.1:8000/mcp`.

## Binary reads and downloads

`stat(path)` returns structured `{sha256, full_size, mtime}` for a regular file.
`sha256` is the lowercase SHA-256 of all raw bytes, `full_size` is the total byte
count, and `mtime` is a UTC RFC 3339 modification timestamp. Text and binary files
are supported; directories and missing files are errors. Hashing scans the whole
file in bounded buffers while holding the existing operation lock, but returns
no file content. External filesystem writers are not covered by that lock.

Use `stat` to get `sha256` for a write tool's `if_hash` without calling `read`.
For changes based on binary ranges, call `stat` before `bin_read` and keep the
original hash for the final conditional write. A stale hash must fail the write;
`mtime` is informational and should not replace SHA-256 version checks. Existing
`overwrite` and `edit` still accept text content; this tool does not add binary
write support.

`bin_read(path, start, size)` reads a byte range and returns structured
`{base64, size}`. Offsets are zero-based; input `size` must be between 0 and
65,536 bytes (64 KiB), regardless of total file size. Output `size` is the actual
raw byte count. Reads crossing EOF return remaining bytes; starting at EOF
returns empty content, while starting beyond EOF is an error.

`download_link(path, secs?)` returns only `/downloads/<token>` as text, without
a scheme or host. `secs` defaults to 300 and accepts 1–3,600 seconds.
Download with an ordinary HTTP GET,
for example `curl --fail --output attachment.bin "$url"`. The server streams raw
bytes from a private copy, so later source changes do not affect the download.
Links can be reused until expiry. Expired or unknown links return HTTP 404;
downloads already started may finish. HTTP Range/partial downloads are not
supported; a GET with a Range header receives the full file with HTTP 200.

The agent assembles the download URL using the externally reachable MCP endpoint:
remove its trailing `/mcp` and append the returned subpath. For example,
`https://example.com/pkb/mcp` plus `/downloads/TOKEN` becomes
`https://example.com/pkb/downloads/TOKEN`. Preserve the reverse-proxy prefix;
do not resolve the leading slash against the host root. The proxy must forward
both MCP and download routes. No server-side domain configuration is required.

Copies use anonymous temporary files in `--tmp-path`, inaccessible through PKB
paths and automatically removed when their last owner closes them. They expire
in the background; active downloads retain their copies until completion or
disconnect. Restarting the server invalidates all links. Up to 128 copies may
exist, including active downloads; combined size defaults to 1 GiB and can be
changed with `--download-quota-bytes`. Copy preparation holds the existing write
lock and can delay other file operations; external filesystem writers are not
covered by that lock. Both tools follow the existing no-symbolic-links PKB
assumption. Anyone holding a URL can download its copy during its lifetime.

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
