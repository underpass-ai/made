# Codex plugin acceptance ladder

The `plugins/made` bundle packages the embedded ceremony engine as a
local MCP stdio server for Codex. Acceptance is cumulative: a later level is
not considered valid unless every earlier level remains green.

## Levels

| Level | Boundary | Evidence |
|---|---|---|
| 1 | Embedded library | `cargo test -p made-embedded --locked` |
| 2 | MCP backend in process | The embedded server advertises only tools it can execute and completes a real ceremony. |
| 3 | MCP binary over stdio | A child process completes `initialize`, `tools/list`, and `tools/call`. |
| 4 | Dependency isolation | The embedded binary tree contains no gRPC, protobuf, NATS, or SQL client. |
| 5 | Plugin bundle | The manifest validates and the bundle launcher completes the same ceremony. |
| 6 | Codex installation | Codex installs the local marketplace entry and discovers the bundled MCP server in a new thread. |

Run levels 2–4 directly:

```bash
cargo test -p made-mcp --all-targets \
  --no-default-features --features embedded --locked
bash scripts/ci/embedded-dependency-boundary.sh
```

Build and execute level 5:

```bash
bash scripts/ci/made-plugin-smoke.sh
```

The smoke builds an isolated release binary, places it at
`plugins/made/bin/made-mcp`, starts it through the plugin's own
launcher, and verifies initialization, the executable tool catalog, machine
discovery, both help audiences, ceremony design and execution, and actual
Markdown report generation. The binary is ignored by Git;
source, manifest, skill, launcher, and tests remain reviewable.

## Current capability

The installed plugin exposes the embedded ceremony engine's design,
publication, one-shot and incremental execution, delegated-host
claim/work/complete coordination, recovery, interventions, evidence and
read-only report projection. `made_discover_capabilities`
describes the exact running version, backend and executable surface, while
`made_get_help` returns `user` or `agent` guidance derived against that
surface. The smoke requires `made_generate_ceremony_report` to be advertised
and to generate a real report from a ceremony completed in the same process.

The embedded default is process-scoped memory. Tool discovery does not imply
restart durability: a host must wire durable instance, definition and context
repositories when state must survive the MCP process.

The smoke verifies the claim/complete tools are exposed. Behavioral tests keep
three cases distinct: the bundled no-op handler, a configured real
server-owned handler, and delegated-host work recorded only after a claim and
an evidence-bearing completion.

## Installation boundary

The repo-local bundle is installed only after levels 1–5 pass. Installation
copies the validated bundle to a personal local plugin source, adds the
personal marketplace entry, and runs `codex plugin add`. Codex loads new plugin
skills and MCP tools at the start of a new thread, so the final functional
check intentionally happens there.
