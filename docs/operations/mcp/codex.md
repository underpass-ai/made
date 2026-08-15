# Codex CLI configuration

Codex CLI reads MCP servers from its TOML config (usually
`~/.codex/config.toml` or `~/.config/codex/config.toml`). The
`made-mcp` adapter is added once; every Codex session can then call
the 35 backend-owned `made_*` tools plus the two server-owned discovery and
help tools exposed in gRPC mode.

See the canonical UX reference at
[`docs/operations/mcp-stdio.md`](../mcp-stdio.md) for the tool list,
env-var reference, and TLS posture options.

## Quick add (installed binary)

Install from crates.io:

```bash
cargo install made-mcp --locked
```

The dev fallback (in-tree source) lives at
`MADE_MCP_INSTALL_MODE=git bash scripts/mcp/install-made-mcp.sh`
in the repo.

```bash
codex mcp add made \
  --env MADE_MCP_GRPC_ENDPOINT=https://made.example.com \
  -- made-mcp
```

The command writes:

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
codex mcp add made \
  --env MADE_MCP_GRPC_ENDPOINT=https://made.example.com \
  -- cargo run -q --manifest-path /path/to/underpass-orchestrator/Cargo.toml -p made-mcp --locked
```

Which writes:

```toml
[mcp_servers.made]
command = "cargo"
args = ["run", "-q", "--manifest-path", "/path/to/underpass-orchestrator/Cargo.toml", "-p", "made-mcp", "--locked"]

[mcp_servers.made.env]
MADE_MCP_GRPC_ENDPOINT = "https://made.example.com"
```

## Fixture mode (no MADE running)

Useful for verifying that Codex picks the tools up at all:

```toml
[mcp_servers.made]
command = "made-mcp"

[mcp_servers.made.env]
MADE_MCP_BACKEND = "fixture"
```

The 35 backend-owned `made_*` tools plus discovery and help become callable;
backend calls return deterministic canned responses (no network), while the
server-owned tools describe that filtered fixture surface.

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

> List MADE's councils.

Codex should call `made_list_councils` and return the live result
(or the fixture's canned list, depending on backend).
