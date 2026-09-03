# Capability verification — what an installed MADE can actually do

Status: normative guidance for maintainers, operators and coding agents.

## The rule

Do not infer the capabilities of an installed MADE from a README,
changelog entry, crate API or repository-wide search alone.

A defensible capability statement identifies four independent facts:

1. **Engine implementation** — the source tree contains a use case, port or
   adapter.
2. **Executable surface** — the running binary and selected backend expose the
   required MCP tools or gRPC RPCs.
3. **Execution ownership and authority** — a configured server handler or an
   authorized host performs the real external work.
4. **Durability** — the active composition persists the required state across
   the failure boundary being claimed.

“MADE can execute this workflow” is ambiguous until all four are
qualified.

## Required MCP verification sequence

1. Inspect the MCP `tools/list` response from the running process.
2. If `made_discover_capabilities` is present, call it and record the
   reported server version, backend, capability groups, executable tools and
   artifact generators.
3. Treat that backend-filtered catalog as authoritative for tool availability.
   If discovery is absent, do not infer a newer catalog from current
   documentation; identify the installed version and reason only from its
   actual `tools/list`.
4. Call `made_get_help` with `audience: "agent"` when available, then use
   only workflows whose complete tool sequence exists on that backend.
5. Establish execution ownership before advancing a step:
   - use server-owned execution only after verifying that a real
     `CeremonyStepHandlerPort` is configured;
   - otherwise claim the exact step, perform the work through the host's own
     authorized tools or workers, and complete it with observable output and
     evidence references.
6. Verify the external result. A terminal ceremony state or completed step is
   evidence of state-machine progress, not by itself evidence that an API,
   agent, tool or human performed the requested action.
7. Verify the storage composition separately before claiming restart recovery,
   failover, durable guards, leases, idempotency or audit retention.

Calling `made_claim_ceremony_step` reserves delegated work; it neither
performs that work nor grants external credentials or authority.

## Composition matrix

| Composition | Executable surface | Default execution | Ceremony durability |
|---|---|---|---|
| Bundled Codex plugin / isolated embedded `made-mcp` | Active embedded catalog; verify with `tools/list` and discovery | Default handler may be no-op; delegated host execution is explicit | SQLite file named by `MADE_MCP_STORE_PATH` (the launcher defaults it under `$XDG_STATE_HOME/underpass-made`); same durability and same published-definition boundary as `EmbeddedMade::open` |
| `EmbeddedMade::default()` | Rust embedded ceremony facade | `NoopCeremonyStepHandler` | In memory |
| `EmbeddedMade::open(path)` | Same Rust facade | No-op unless the host injects a handler | SQLite persists ceremony snapshots, unit-of-work state, audit journal, outbox and definition publications; mounted definitions and transcripts remain in memory unless replaced |
| Deployable `made` without `MADE_CEREMONY_STORE_PATH` | gRPC plus the selected MCP backend | Depends on configured server adapters | Ceremony state is in memory; optional PostgreSQL covers other aggregates, not ceremonies |
| Deployable `made` with `MADE_CEREMONY_STORE_PATH` | gRPC plus the selected MCP backend | Depends on configured server adapters | Ceremony state and publications use SQLite; multiple processes may share the store only when the filesystem supports SQLite locking and WAL semantics |
| MCP gRPC backend | Remote RPC-derived catalog; embedded-only extensions may be absent | Owned by the remote deployment | Determined by the remote deployment, not by the MCP adapter |

For the current repository composition, PostgreSQL persists deliberations,
councils, agents and related statistics. It is not the ceremony-state adapter.

### The published-definition restart boundary

Durable ceremony state alone does not guarantee restart recovery of every
instance. In the SQLite compositions, published definitions are durable but
mounted definitions are not, so:

- an instance started from a **published** definition
  (`made_publish_ceremony_definition`, then
  `made_start_published_ceremony`) rehydrates after the process reopens the
  store;
- an instance started from an inline `definition_yaml` (a mounted definition)
  persists its state, but rehydration fails after reopen with
  `not found: ceremony_definition`, because the definition it needs lived in
  memory.

Verify restart claims with the publish → start-published → reopen sequence;
`crates/made-embedded/tests/sqlite_store_api.rs` exercises the recovering
path. A host that must resume instances across restarts starts them from
published definitions.

## What discovery proves — and what it does not

`made_discover_capabilities` is generated from the same filtered catalog
used by MCP `tools/list`. It is the executable's self-description, so it
supersedes static tool lists for the installed process.

Discovery proves neither:

- that a non-no-op step or evidence handler is installed;
- that credentials, network access or provider scopes exist;
- that a human has approved a guard or delegated action;
- that generated report text was saved by the host;
- nor that any repository survives a process restart.

Those are composition, authority and observation questions.

## Documentation authoring rule

Avoid unqualified sentences such as “MADE persists ceremonies” or
“MADE executes agent steps.” State the boundary in the sentence:

- “The embedded MCP executable exposes `made_claim_ceremony_step` on the
  installed backend.”
- “The host executes the claimed step through its separately authorized
  worker.”
- “The deployable process persists ceremony state in SQLite when
  `MADE_CEREMONY_STORE_PATH` is configured.”
- “The default embedded handler can complete protocol transitions without
  performing external work.”

When a composition changes, update this runbook, the relevant distribution
README and the changelog in the same pull request. A new tool must appear in
the executable catalog and discovery tests before documentation can claim it
for that backend.

## Review checklist

Before approving a capability claim, verify:

- [ ] installed version and backend are named;
- [ ] required tools appear in the active catalog;
- [ ] the full workflow sequence is available;
- [ ] execution owner and external authority are explicit;
- [ ] no-op completion is distinguished from real work;
- [ ] durable and in-memory ports are listed separately;
- [ ] failure boundary and restart behavior are stated;
- [ ] generated artifacts identify who saves them;
- [ ] tests or source composition support the statement.
