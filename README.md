<p align="center"><img src="docs/assets/made-cuatro-voces.png" width="936" alt="MADE — Your business. Your agents. Your architecture. — by Underpass"></p>
<p align="center"><strong>Multi-Agent Deliberation Engine · by Underpass</strong></p>

MADE coordinates a shared procedure: who can act, which work is ready, what
needs review and when a person must decide. Your host supplies the agents,
tools and people. MADE validates their progress and records the accepted
results in an auditable ceremony event stream.

The default path is local: a plugin, an MCP process and SQLite. No MADE
account, deployed service, Docker or Kubernetes is needed. Rust applications
can embed the same engine. A separate service distribution supports gRPC,
provider-backed councils and Kubernetes.

## Start locally

These pages describe **0.6.0**. The release installation below requires the
0.6.0 crates and checksummed assets to be public. Before publication, use the
[source candidate route](docs/embedded/README.md#test-a-source-candidate).

Install the published version with Cargo:

```bash
cargo install made-mcp --version 0.6.0 --locked
mkdir -p "$HOME/.local/state/underpass-made"
MADE_MCP_BACKEND=embedded \
MADE_MCP_STORE_PATH="$HOME/.local/state/underpass-made/ceremonies.sqlite3" \
  made-mcp
```

`made-mcp` speaks MCP over stdio; register that command and environment in
your host using the [local setup guide](docs/embedded/README.md). Checksummed
[release binaries](https://github.com/underpass-ai/made/releases) avoid the
Rust toolchain. To embed the engine in Rust, start with the
[complete library example](docs/embedded/rust.md).

The plugin adds installation, design and execution skills. **The stable route
requires the 0.6.0 assets to be public and `marketplace` to have advanced to
that release.** The v0.5.0 marketplace snapshot uses the old `underpass`
identity and cannot provide `made@made`.

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@made
```

After those publication conditions hold, run `made-setup` and start a new
task. The [plugin guide](docs/plugins/README.md) also covers testing a local
candidate with `MADE_MCP_BIN`, Claude Code and the native Windows launcher.

## Give the host a procedure

> Design a change review: an author proposes a solution, a reviewer challenges
> it, and I approve the final result.

Design produces a draft. Publish its reviewed definition, then start a
session from that published name and version so it can resume after restart.
For each delegated step, the host claims the work, performs it and submits
its output. A claim alone performs nothing. The default no-op handler proves
engine wiring, not that external work happened.

Ceremonies can declare sequential or concurrent work, role eligibility,
human guards, retry policies, bounded repetition, transition budgets and
context writes. The `run_ceremony` driver claims eligible siblings and invokes
host-provided handlers with bounded concurrency. Hosts may also fan work out to
their own workers through the claim and completion APIs. MADE does not create
agents for the host.

## Pick your path

| Need | Start here |
|:--|:--|
| Use MADE through a coding agent | [Plugin](docs/plugins/README.md) · [Manual MCP setup](docs/embedded/README.md) |
| Put the engine inside a Rust application | [Embedding](docs/embedded/rust.md) |
| Design a reusable procedure | [Ceremony authoring](docs/authoring/README.md) |
| Execute, resume or inspect a session | [Runtime contract](docs/runtime/README.md) |
| Operate a shared service | [Kubernetes](docs/operations/deploy-kubernetes.md) |
| Build or extend MADE | [Architecture](docs/architecture/README.md) · [Development](docs/development/README.md) |

MADE is pre-1.0. Version 0.6.0 requires client changes from 0.5.x, including
the accepted claim's fence on completion. Use the running server's `tools/list` and
`made_discover_capabilities` to check the installed surface. See
[migrations](docs/migrations/README.md), [release history](CHANGELOG.md) and
[documentation home](docs/index.md).

[Apache-2.0](LICENSE). Copyright © 2026 Tirso García Ibáñez.
Part of [Underpass AI](https://underpassai.com).
