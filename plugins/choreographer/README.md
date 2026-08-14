# Choreographer Codex plugin

This bundle runs the Choreographer ceremony engine as a local MCP stdio
process. It does not require a Choreographer service, gRPC, NATS, or a
database.

The repository packaging script places the isolated embedded binary at
`bin/choreo-mcp`. Codex starts it through `scripts/run-embedded-mcp.sh`.

Executable scope:

- `choreo_discover_capabilities` to return the installed server version,
  backend, capability groups, executable tools and artifact generators from
  the same filtered catalog used by MCP `tools/list`;
- `choreo_get_help` with `audience: user` or `audience: agent` for
  human-readable and structured workflow guidance. Agent help includes
  preconditions, authority boundaries, delegated-host sequencing and error
  handling;
- `choreo_design_ceremony` to turn structured intent into an analysed,
  unpublished linear ceremony draft;
- `choreo_run_ceremony` for one-shot terminal execution;
- `choreo_start_ceremony`, `choreo_run_ceremony_step`,
  `choreo_claim_ceremony_step`, `choreo_complete_ceremony_step`,
  `choreo_approve_ceremony_guard`, `choreo_defer_ceremony_guard`,
  `choreo_apply_ceremony_transition`, and
  `choreo_get_ceremony_instance` for persistent, human-authorized flows;
- `choreo_list_ceremony_instances` to rediscover resumable meetings known to
  the active backend;
- `choreo_generate_ceremony_report` to project selected ceremony snapshots
  and their ordered audit journals into deterministic Markdown. It returns
  `persisted: false`; the host chooses whether and where to save that text;
- `choreo_request_ceremony_intervention`,
  `choreo_respond_to_ceremony_intervention`,
  `choreo_collect_ceremony_evidence`, and
  `choreo_close_ceremony_intervention` for participant-created live agenda
  items controlled by the requesting role.

The bundled zero-infrastructure process keeps its repositories in memory.
`choreo_list_ceremony_instances` can recover host-side conversation loss while
that process remains alive. Surviving a process restart requires a host to wire
durable instance, definition, and context repositories.

Start an unfamiliar session with `choreo_discover_capabilities`. Its
`artifact_generators` array marks `choreo_generate_ceremony_report`, including
the exact response field and the host-owned persistence boundary. This is a
machine-readable view of the running binary, not a static copy of this README.

For a guided workflow, call:

```json
{"name":"choreo_get_help","arguments":{"audience":"user"}}
```

or:

```json
{"name":"choreo_get_help","arguments":{"audience":"agent"}}
```

The returned `help_markdown` is meant for display. The parallel structured
fields let an agent choose only workflows whose complete tool sequence is
available on the active backend.

Step execution has two explicit ownership paths. A verified server-owned
handler may be invoked with `choreo_run_ceremony_step`. Otherwise the host
claims the exact step, performs the real work through its own authorized
worker/tools, and completes it with observable output and evidence references.
The bundled default may use `NoopCeremonyStepHandler`; an empty completed step
proves protocol/state-machine wiring only. Claiming a step performs no work and
grants no external authority.
