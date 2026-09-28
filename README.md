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

These guides describe **0.8.0**. Install the plugin from the published tag
so its skills, setup scripts and executable use the same release:

```bash
codex plugin marketplace add underpass-ai/made --ref v0.8.0
codex plugin add made@made
```

Run `made-setup`, then start a new task. Setup downloads and verifies the
release binary, configures authorization and a persistent search cursor key,
and bootstraps the local SQLite store. Cargo is not required. The
[plugin guide](docs/plugins/README.md) covers Claude Code, Windows and updates.
As of 2026-09-28, the rolling `marketplace` branch still points to 0.7.8;
refreshing that branch alone does not install 0.8.0.

For manual MCP registration, install the matching binary:

```bash
cargo install made-mcp --version 0.8.0 --locked
made-mcp --version
```

Then follow the [local setup guide](docs/embedded/README.md#register-mcp)
to bootstrap authorization and register the command with all required
configuration. Setting only the backend and SQLite path is insufficient.
Checksummed [release binaries](https://github.com/underpass-ai/made/releases/tag/v0.8.0)
avoid the Rust toolchain. To embed the engine in Rust, start with the
[complete library example](docs/embedded/rust.md).

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
| Hand off a definition, question a working agent, compose a system, drive a whole scope | [0.8 capabilities](docs/corte7/README.md) |
| Operate a shared service | [Kubernetes](docs/operations/deploy-kubernetes.md) |
| Build or extend MADE | [Architecture](docs/architecture/README.md) · [Development](docs/development/README.md) |

MADE is pre-1.0. Upgrades from older releases can require client and store
changes, including completion fences and compatible snapshot/event readers.
Use the running server's `tools/list` and
`made_discover_capabilities` to check the installed surface. See
[migrations](docs/migrations/README.md), [release history](CHANGELOG.md) and
[documentation home](docs/index.md).

[Apache-2.0](LICENSE). Copyright © 2026 Tirso García Ibáñez.
Part of [Underpass AI](https://underpassai.com).
