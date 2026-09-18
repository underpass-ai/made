# MADE — Structured work for your agents

**Multi-Agent Deliberation Engine, by Underpass.**

MADE gives your coding agent a shared procedure for work: roles, steps,
review, human approval and a record of the result. The embedded edition runs
locally inside your agent host, with ceremony state and session memory in
SQLite. No MADE server, Docker or Kubernetes is required.

Your agents do the work. MADE keeps track of who can act, what must happen
next and which results have been accepted.

## Start with the embedded edition

Install the MCP server with Rust, or download a checksummed executable from
[Releases](https://github.com/underpass-ai/made/releases):

```bash
cargo install made-mcp --locked
```

Choose where sessions should survive a restart:

```bash
mkdir -p "$HOME/.local/state/underpass-made"
MADE_MCP_BACKEND=embedded \
MADE_MCP_STORE_PATH="$HOME/.local/state/underpass-made/ceremonies.sqlite3" \
  made-mcp
```

Register that command and those environment variables in your MCP host.
Codex and Claude Code can share the same local SQLite file. See the
[MCP setup guide](docs/operations/mcp-stdio.md) for host configuration.

The [MADE plugin](plugins/made/README.md) adds setup, ceremony-design and
ceremony-running skills. Its setup downloads the release-matched executable
and verifies its checksum, so plugin users do not need a Rust toolchain.

## Give the agent a procedure

For example:

> Design a ceremony to review this change. Have an author propose a solution,
> a reviewer challenge it, and me approve the final result.

With the plugin installed, the agent can design and publish the ceremony,
then guide its execution. MADE records claims and results; the host supplies
the agents, tools or people that perform each step.

- **Define the work:** reusable YAML ceremonies with roles, steps and guards.
- **Control progression:** retries, human approvals, bounded repetition and
  concurrent steps with explicit joins and capacity limits.
- **Pass results forward:** select allowed roles from context and copy declared
  output fields into context atomically.
- **Resume and inspect:** published definitions, sealed event history, session
  memory and reports backed by the local store.

Publish a definition before starting a session you want to resume after a
restart. A completed protocol step only proves real external work when the
host has connected that work to the step. The
[execution guide](docs/operations/embedded-ceremony-execution.md) shows that
claim → execute → complete loop.

## Use it in your own application

Rust hosts can embed the same engine through `made-embedded`, supplying async
callbacks for agent, tool or human work. The host owns execution and its
runtime; MADE owns the ceremony rules and event history.

See [Embedding MADE](docs/embedded-made.md) and the
[ceremony authoring guide](docs/operations/ceremony-authoring-runbook.md).

## Also deployable on Kubernetes

MADE also ships as a service with a Helm chart. This edition adds the full
gRPC API, provider-backed councils, an optional LLM judge, NATS messaging,
Postgres for council data, and metrics and traces. Ceremonies and session
memory use SQLite when a ceremony-store path is configured.

Both editions use the same ceremony engine. Choose Kubernetes when you need
a deployed service and are ready to operate it.

[Deploy on Kubernetes](docs/operations/deploy-kubernetes.md) ·
[Compare editions](docs/editions.md)

## Project status

MADE is pre-1.0. Version 0.5.0 includes the event-stream foundation and the
primitives for claimable concurrency. Hosts still schedule concurrent work;
automatic parallel drivers, aggregation and the complete orchestration
patterns remain on the [roadmap](docs/orchestration-patterns-plan.md).

MADE works independently of [KMP](https://github.com/underpass-ai/kmp) and
[Underpass Runtime](https://github.com/underpass-ai/underpass-runtime).
The embedded path needs neither service.

## Documentation

- [Documentation home](docs/index.md)
- [Plugin installation](plugins/made/README.md)
- [Ceremony examples](api/examples/ceremonies)
- [Operations and observability](docs/operations/observability-runbook.md)
- [Development](docs/dev-loop.md) · [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md) · [Security](SECURITY.md)

## License

[Apache License 2.0](LICENSE). Copyright © 2026 Tirso García Ibáñez.
Part of [Underpass AI](https://underpassai.com).
