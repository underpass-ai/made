# Claude Desktop configuration

Claude Desktop reads MCP servers from `claude_desktop_config.json`.
Path varies by OS:

| OS      | Path                                                                          |
|---------|-------------------------------------------------------------------------------|
| macOS   | `~/Library/Application Support/Claude/claude_desktop_config.json`             |
| Linux   | `~/.config/Claude/claude_desktop_config.json`                                 |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json`                                 |

See the canonical UX reference at
[`docs/operations/mcp-stdio.md`](../mcp-stdio.md) for the tool list,
env-var reference, and TLS posture options.

## Installed binary

Install from crates.io:

```bash
cargo install made-mcp --locked
```

The dev fallback (in-tree source) lives at
`MADE_MCP_INSTALL_MODE=git bash scripts/mcp/install-made-mcp.sh`
in the repo.

```json
{
  "mcpServers": {
    "made": {
      "command": "made-mcp",
      "env": {
        "MADE_MCP_GRPC_ENDPOINT": "https://made.example.com"
      }
    }
  }
}
```

The binary must be on Claude's `PATH`. If `cargo install` placed it
under `~/.cargo/bin` and that directory is not in Claude's `PATH`,
use an absolute path:

```json
"command": "/home/<you>/.cargo/bin/made-mcp"
```

## Dev from a checkout

```json
{
  "mcpServers": {
    "made": {
      "command": "cargo",
      "args": [
        "run", "-q",
        "--manifest-path", "/path/to/underpass-orchestrator/Cargo.toml",
        "-p", "made-mcp",
        "--locked"
      ],
      "env": {
        "MADE_MCP_GRPC_ENDPOINT": "https://made.example.com"
      }
    }
  }
}
```

## Fixture mode (no MADE running)

```json
{
  "mcpServers": {
    "made": {
      "command": "made-mcp",
      "env": {
        "MADE_MCP_BACKEND": "fixture"
      }
    }
  }
}
```

Every `made_*` tool becomes callable and returns its deterministic
canned response.

## mTLS to a hardened deployment

```json
{
  "mcpServers": {
    "made": {
      "command": "made-mcp",
      "env": {
        "MADE_MCP_GRPC_ENDPOINT": "https://made.underpass.svc:50055",
        "MADE_MCP_GRPC_TLS_MODE": "mutual",
        "MADE_MCP_GRPC_TLS_CA_PATH": "/var/run/made-tls/ca.crt",
        "MADE_MCP_GRPC_TLS_CERT_PATH": "/var/run/made-tls/tls.crt",
        "MADE_MCP_GRPC_TLS_KEY_PATH": "/var/run/made-tls/tls.key",
        "MADE_MCP_GRPC_TLS_DOMAIN_NAME": "made-grpc"
      }
    }
  }
}
```

## Verifying

After saving the config, restart Claude Desktop completely (the
config is read on launch). In a new conversation:

> List MADE's councils.

Claude should call `made_list_councils` and show the result.

If the tools do not appear:

- check Claude Desktop's "Developer" panel for stderr from the
  spawned `made-mcp` process — the adapter writes JSON tracing
  to stderr on launch (`backend`, `grpc_tls`);
- verify `made-mcp --version` runs from a terminal with the
  same `PATH` Claude inherits;
- rule out config-parse errors by validating the JSON file.
