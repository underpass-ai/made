# Local MADE

Use the [plugin](../plugins/README.md) for bundled skills and a release-matched
executable. Use the manual route below when you manage MCP registration.
For an in-process engine, see [Rust embedding](rust.md).

## Install a binary

```bash
cargo install made-mcp --locked
made-mcp --version
```

Cargo's default features include the embedded and gRPC backends. An
embedded-only source build uses:

```bash
cargo build -p made-mcp --release --locked --no-default-features --features embedded
```

Alternatively choose the executable and its SHA-256 file for your platform
from a [published release](https://github.com/underpass-ai/made/releases).
Check the digest before running it. The plugin's setup adapter automates this
for Linux x86_64/arm64, macOS arm64 and Windows x86_64.

## Register MCP

The embedded backend requires an explicit SQLite path. Create its parent
directory and use an absolute path in host configuration. A minimal Codex
configuration is:

```toml
[mcp_servers.made]
command = "/absolute/path/to/made-mcp"

[mcp_servers.made.env]
MADE_MCP_BACKEND = "embedded"
MADE_MCP_STORE_PATH = "/absolute/path/to/ceremonies.sqlite3"
```

A host accepting the standard JSON `mcpServers` shape can use:

```json
{
  "mcpServers": {
    "made": {
      "command": "/absolute/path/to/made-mcp",
      "env": {
        "MADE_MCP_BACKEND": "embedded",
        "MADE_MCP_STORE_PATH": "/absolute/path/to/ceremonies.sqlite3"
      }
    }
  }
}
```

Use one active MADE registration in a host. When switching to the plugin,
remove the duplicate manual registration and retain the same store path.
The protocol owns stdout; diagnostics belong on stderr.

Start a new host task, call `made_discover_capabilities`, and then
`made_get_help` with `audience: "agent"` or `"user"`. A discovered tool is
available on that backend; a step handler still needs its own real host
implementation.

## Persistence and recovery

SQLite holds sealed ceremony events, snapshots, published definitions,
consumer positions and session memory. Published definitions bind a name,
version and digest. Publish before starting a session that must survive a
restart. A supplied or mounted YAML definition can remain process-local:
its instance may be listed with `rehydratable: false` after reopening.

The default plugin store is
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`.
`MADE_MCP_STORE_PATH` overrides it. This data directory is independent of
the marketplace identity and the disposable plugin cache.

Concurrent SQLite clients coordinate appends through store transactions and
optimistic version checks. Share a local file only through supported store
access; do not put a live SQLite file on a file-sync service or copy just the
main database while WAL writes are active. For a backup, stop writers and
preserve the database consistently, or use SQLite's backup facilities.

If startup detects a legacy Redb default without its converted SQLite file,
it refuses to create an empty replacement. Follow [storage migration](../migrations/README.md).
For a lost session id, list instances and inspect the matching session before
creating a successor. Recovery never repeats external work by itself.
