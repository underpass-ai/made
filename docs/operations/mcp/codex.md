# Codex CLI configuration

Codex CLI reads MCP servers from its TOML config (usually
`~/.codex/config.toml` or `~/.config/codex/config.toml`). The
`made-mcp` adapter is added once. Its callable surface depends on the selected
backend and installed version; inspect MCP `tools/list`, then call
`made_discover_capabilities` instead of relying on a fixed tool count.

See the canonical UX reference at
[`docs/operations/mcp-stdio.md`](../mcp-stdio.md) for the tool list,
env-var reference, and TLS posture options.

## Local embedded engine

Install from crates.io:

```bash
cargo install made-mcp --locked
```

The dev fallback (in-tree source) lives at
`MADE_MCP_INSTALL_MODE=git bash scripts/mcp/install-made-mcp.sh`
in the repo.

```bash
mkdir -p "$HOME/.local/state/underpass-made"
codex mcp add made \
  --env MADE_MCP_BACKEND=embedded \
  --env MADE_MCP_STORE_PATH="$HOME/.local/state/underpass-made/ceremonies.sqlite3" \
  -- made-mcp
```

That command records an absolute store path after the shell expands `$HOME`.
Codex and Claude Code can point their separate MCP processes at this same
SQLite WAL file. If you edit TOML directly, use an absolute path:

```toml
[mcp_servers.made]
command = "made-mcp"

[mcp_servers.made.env]
MADE_MCP_BACKEND = "embedded"
MADE_MCP_STORE_PATH = "/home/YOU/.local/state/underpass-made/ceremonies.sqlite3"
```

The embedded backend fails fast when `MADE_MCP_STORE_PATH` is absent. Publish a
definition before starting any session that must rehydrate after a process
restart.

## Connect to deployed MADE over gRPC

Point the same adapter at a running MADE service when you need the cluster
edition:

```bash
codex mcp add made \
  --env MADE_MCP_GRPC_ENDPOINT=https://made.example.com \
  -- made-mcp
```

The equivalent TOML is:

```toml
[mcp_servers.made]
command = "made-mcp"

[mcp_servers.made.env]
MADE_MCP_GRPC_ENDPOINT = "https://made.example.com"
```

## Dev from a checkout

When you want to run against the in-tree build (no install step), use
an absolute manifest path so the config works from any working
directory:

```bash
mkdir -p "$HOME/.local/state/underpass-made"
codex mcp add made \
  --env MADE_MCP_BACKEND=embedded \
  --env MADE_MCP_STORE_PATH="$HOME/.local/state/underpass-made/ceremonies.sqlite3" \
  -- cargo run -q --manifest-path /path/to/made/Cargo.toml -p made-mcp --locked --no-default-features --features embedded
```

Which writes:

```toml
[mcp_servers.made]
command = "cargo"
args = ["run", "-q", "--manifest-path", "/path/to/made/Cargo.toml", "-p", "made-mcp", "--locked", "--no-default-features", "--features", "embedded"]

[mcp_servers.made.env]
MADE_MCP_BACKEND = "embedded"
MADE_MCP_STORE_PATH = "/home/YOU/.local/state/underpass-made/ceremonies.sqlite3"
```

## Fixture mode (no MADE running)

Useful for verifying that Codex picks the tools up at all:

```toml
[mcp_servers.made]
command = "made-mcp"

[mcp_servers.made.env]
MADE_MCP_BACKEND = "fixture"
```

The fixture-filtered `made_*` surface plus discovery and help becomes callable;
backend calls return deterministic canned responses (no network). Use
`tools/list` and `made_discover_capabilities` to inspect that installed
surface.

## mTLS to a hardened deployment

When MADE is behind mTLS (chart's
`tls.mode=mutual`), point Codex at the local cert bundle:

```toml
[mcp_servers.made]
command = "made-mcp"

[mcp_servers.made.env]
MADE_MCP_GRPC_ENDPOINT = "https://made.underpass.svc:50055"
MADE_MCP_GRPC_TLS_MODE = "mutual"
MADE_MCP_GRPC_TLS_CA_PATH = "/var/run/made-tls/ca.crt"
MADE_MCP_GRPC_TLS_CERT_PATH = "/var/run/made-tls/tls.crt"
MADE_MCP_GRPC_TLS_KEY_PATH = "/var/run/made-tls/tls.key"
MADE_MCP_GRPC_TLS_DOMAIN_NAME = "made-grpc"
```

The same `_TLS_*` envs trigger auto-detection — setting them is
enough; `_TLS_MODE` is a manual override when you want it explicit
for self-documentation.

## Verifying

After updating the config, restart Codex and ask the agent:

> Discover MADE's active backend and available capabilities.

Codex should call `made_discover_capabilities` and report the backend you
configured. In embedded mode, ceremony tools are present and the council
surface is absent. In gRPC mode, a compatible full MADE service exposes the
council surface as well.
