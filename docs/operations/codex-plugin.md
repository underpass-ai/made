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
Markdown report generation. It then proves durability the only way that
counts: a second launcher process publishes and starts a ceremony, a third
one reopens the same state file and reads that ceremony back with its state,
next step and bound definition digest intact. The binary is ignored by Git;
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

The embedded backend opens the redb state file named by
`MADE_MCP_REDB_PATH`; the launcher defaults it to
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb`, and
without a path the binary exits rather than run on memory that dies with the
process. Durability is still not authority, and it is not unconditional
recovery: an instance started from a published definition rehydrates, one
started from supplied YAML keeps its snapshot but cannot reload its
definition, and the listing marks it `"rehydratable": false`. Mounted
definitions and transcripts stay in memory unless the host replaces those
ports.

On the first start after the Choreographer → MADE rename, the launcher looks
for the former default
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-choreographer/ceremonies.redb`
only when the MADE destination does not exist. It opens that source through a
read-only file descriptor, clones it to the new MADE path, lets redb repair the
clone if the old process did not shut down cleanly, and migrates publication
digests and bound instances in one transaction. The source is never opened
writable. An explicit `MADE_MCP_LEGACY_REDB_PATH` enables the same flow for a
non-default source; `MADE_MCP_REDB_PATH` must name a new destination on the
first run.

Successful imports persist a `choreographer-v1-to-made-v1` receipt with the
source SHA-256 and row counts. Startup emits the same bounded fields as the
structured `made legacy state migration completed` event. Later starts verify
the receipt and do not reopen the source. A destination that already exists
without that receipt is refused instead of overwritten or silently adopted.

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
