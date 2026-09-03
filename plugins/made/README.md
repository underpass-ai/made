# MADE plugin — Codex and Claude Code

This bundle runs the MADE ceremony engine as a local MCP stdio
process. It does not require a MADE service, gRPC, NATS, or a
database.

## Installation

**Claude Code** — from this repository's stable marketplace branch:

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@underpass
/made:setup
```

**Codex:**

```text
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@underpass
```

Then ask Codex to run `made-setup`. Setup downloads the `made-mcp` executable
from the immutable release matching the plugin manifest, verifies its
published SHA-256 checksum, and installs it into the plugin's ignored `bin/`
directory. No Rust toolchain is required. Start a new host thread after first
setup or an update so the MCP server and skills reload together.

To update Claude Code, run `/plugin marketplace update underpass`,
`/plugin update made@underpass`, and `/made:setup`. For Codex, run
`codex plugin marketplace upgrade underpass`, reinstall with
`codex plugin add made@underpass`, then run `made-setup`. The marketplace and
engine therefore move to the same immutable release before the new thread
starts.

The bundle installs into both certified hosts:

- **Codex** reads `.codex-plugin/plugin.json` and starts the MCP server
  through `.mcp.json`.
- **Claude Code** reads `.claude-plugin/plugin.json` and the same
  `.mcp.json`. The bundle layout is identical; only the manifest differs.

Prebuilt packages are attached to each GitHub Release as
`made-plugin-<version>-<os>-<arch>.tar.gz` with a per-archive `.sha256`
checksum. The same release carries standalone
`made-mcp-v<version>-<target>` executables and checksums used by `made-setup`.
To install an archive manually, verify it, unpack it, and point the host at
the resulting `made/` directory:

```bash
sha256sum -c made-plugin-<version>-<os>-<arch>.sha256
tar -xzf made-plugin-<version>-<os>-<arch>.tar.gz
```

On Windows hosts, register the MCP server with
`scripts\run-embedded-mcp.cmd` instead of the `.sh` launcher; the state
file defaults to `%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`.

To build the package from a checkout instead:

```bash
just plugin-package   # writes dist/plugin/made-plugin-<version>-<os>-<arch>.tar.gz
```

The version stamped into both manifests comes from the workspace
`Cargo.toml`; release tags (`v*`) must match it or packaging fails.

## Capability truth

This plugin selects the embedded MCP backend, and the launcher points it at
the canonical SQLite WAL store —
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`
unless `MADE_MCP_STORE_PATH` says otherwise — so ceremonies survive the MCP
process and multiple agent hosts can share one path. Durable is not the same
as authorized, and not the same as fully
recoverable: the bundled composition may use `NoopCeremonyStepHandler`, so a
terminal step from that default proves ceremony protocol and state-machine
behavior, not that an agent, tool, API, or human performed the requested work.
Only instances started from a *published* definition rehydrate after a
restart; the listing reports the rest as `"rehydratable": false`. The
[embedded ceremony execution runbook](../../docs/operations/embedded-ceremony-execution.md)
walks the loop that keeps both claims honest.

Treat the running executable as the source of truth for tool availability:
inspect MCP `tools/list`, then call `made_discover_capabilities` when
present. Establish execution ownership and durability separately. The
[capability-verification runbook](../../docs/operations/capability-verification.md)
defines the required checks and wording for agents and documentation.

The repository packaging script places the isolated embedded binary at
`bin/made-mcp`, and the launcher prefers it — a release bundle or `made-setup`
pins the binary that plugin version was tested against. The launcher retains a
`made-mcp` on `PATH` as a manual-development fallback. If neither is present it
fails with an explicit setup command instead of requiring Cargo.

The launcher never silently creates a fresh SQLite store beside the former
Redb default. If it finds that legacy file, startup stops with the exact
`made-mcp` v0.2.0 conversion command; the old file remains intact as a backup.

Executable scope:

- `made_discover_capabilities` to return the installed server version,
  backend, capability groups, executable tools and artifact generators from
  the same filtered catalog used by MCP `tools/list`;
- `made_get_help` with `audience: user` or `audience: agent` for
  human-readable and structured workflow guidance. Agent help includes
  preconditions, authority boundaries, delegated-host sequencing and error
  handling;
- `made_design_ceremony` to turn structured intent into an analysed,
  unpublished linear ceremony draft;
- `made_run_ceremony` for one-shot terminal execution;
- `made_start_ceremony`, `made_run_ceremony_step`,
  `made_claim_ceremony_step`, `made_complete_ceremony_step`,
  `made_approve_ceremony_guard`, `made_defer_ceremony_guard`,
  `made_apply_ceremony_transition`, and
  `made_get_ceremony_instance` for persistent, human-authorized flows;
- `made_list_ceremony_instances` to rediscover resumable meetings known to
  the active backend;
- `made_generate_ceremony_report` to project selected ceremony snapshots
  and their ordered audit journals into deterministic Markdown. It returns
  `persisted: false`; the host chooses whether and where to save that text;
- `made_request_ceremony_intervention`,
  `made_respond_to_ceremony_intervention`,
  `made_collect_ceremony_evidence`, and
  `made_close_ceremony_intervention` for participant-created live agenda
  items controlled by the requesting role.

The bundled zero-infrastructure process persists ceremony instances, published
definitions, the audit journal and outbox in SQLite. Mounted definitions and
transcripts remain in memory. `made_list_ceremony_instances` therefore recovers
published-definition sessions after a process restart and marks ad-hoc sessions
as unrehydratable instead of hiding their persisted snapshots.

Start an unfamiliar session with `made_discover_capabilities`. Its
`artifact_generators` array marks `made_generate_ceremony_report`, including
the exact response field and the host-owned persistence boundary. This is a
machine-readable view of the running binary, not a static copy of this README.

For a guided workflow, call:

```json
{"name":"made_get_help","arguments":{"audience":"user"}}
```

or:

```json
{"name":"made_get_help","arguments":{"audience":"agent"}}
```

The returned `help_markdown` is meant for display. The parallel structured
fields let an agent choose only workflows whose complete tool sequence is
available on the active backend.

Step execution has two explicit ownership paths. A verified server-owned
handler may be invoked with `made_run_ceremony_step`. Otherwise the host
claims the exact step, performs the real work through its own authorized
worker/tools, and completes it with observable output and evidence references.
The bundled default may use `NoopCeremonyStepHandler`; an empty completed step
proves protocol/state-machine wiring only. Claiming a step performs no work and
grants no external authority.
