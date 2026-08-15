# MADE Codex plugin

This bundle runs the MADE ceremony engine as a local MCP stdio
process. It does not require a MADE service, gRPC, NATS, or a
database.

## Capability truth

This plugin selects the embedded MCP backend, but “embedded” does not by itself
mean durable or externally authorized. The bundled composition uses
process-local repositories and may use `NoopCeremonyStepHandler`. A terminal
step from that default proves ceremony protocol and state-machine behavior, not
that an agent, tool, API, or human performed the requested work.

Treat the running executable as the source of truth for tool availability:
inspect MCP `tools/list`, then call `made_discover_capabilities` when
present. Establish execution ownership and durability separately. The
[capability-verification runbook](../../docs/operations/capability-verification.md)
defines the required checks and wording for agents and documentation.

The repository packaging script places the isolated embedded binary at
`bin/made-mcp`. Codex starts it through `scripts/run-embedded-mcp.sh`.

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

The bundled zero-infrastructure process keeps its repositories in memory.
`made_list_ceremony_instances` can recover host-side conversation loss while
that process remains alive. Surviving a process restart requires a host to wire
durable instance, definition, and context repositories.

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
