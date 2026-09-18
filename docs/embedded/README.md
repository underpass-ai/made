# Local MADE

Use the [plugin](../plugins/README.md) for bundled skills and a release-matched
executable. Use the manual route below when you manage MCP registration.
For an in-process engine, see [Rust embedding](rust.md).

## Install a binary

These instructions target **0.6.0**. The registry command and release downloads
require that version to have been published. For a candidate whose assets do
not exist yet, use the source route below; an installed 0.5.0 binary does not
implement the new completion contract.

```bash
cargo install made-mcp --version 0.6.0 --locked
made-mcp --version
```

Cargo's default features include the embedded and gRPC backends. Alternatively
choose the 0.6.0 executable and its SHA-256 file for your platform from a
[published release](https://github.com/underpass-ai/made/releases).
Check the digest before running it. The plugin's setup adapter automates this
for Linux x86_64/arm64, macOS arm64 and Windows x86_64 once the assets are public.

## Test a source candidate

From the reviewed 0.6.0 checkout, build an embedded-only executable. In a
POSIX shell:

```bash
cargo build -p made-mcp --release --locked --no-default-features --features embedded
MADE_CANDIDATE_TARGET="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
export MADE_MCP_BIN="${MADE_CANDIDATE_TARGET}/release/made-mcp"
"$MADE_MCP_BIN" --version
```

Use the reported Cargo target directory; it may be outside the checkout. On
native Windows the executable ends in `made-mcp.exe`. The build should report
0.6.0. Register this absolute executable path manually, or set `MADE_MCP_BIN`
to it in the plugin's host launch environment before starting a new task.
An export in an unrelated shell does not configure a running desktop host.
See [local plugin testing](../plugins/README.md#test-a-local-candidate).

Before the release assets exist, do not run the download installer for the
0.6.0 manifest or silently fall back to a 0.5.0 binary. A source build is a
candidate test, not evidence that 0.6.0 has been published.

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

The POSIX plugin store defaults to
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`.
The native Windows launcher defaults to
`%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`, with
`%USERPROFILE%\.local\state` as the state-root fallback when `LOCALAPPDATA`
is absent. Native Windows hosts without Bash must use the plugin's `.cmd`
launcher; see [Windows setup](../plugins/README.md#native-windows-launcher).
`MADE_MCP_STORE_PATH` overrides either default. This data directory is independent of
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
