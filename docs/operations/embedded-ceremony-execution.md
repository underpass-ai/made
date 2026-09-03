# Embedded ceremony execution runbook

This runbook operates a MADE ceremony through the local MCP plugin
when Codex, rather than an engine-owned handler, performs the real work. The
whole runtime is embedded:

```text
Codex host
└── made-mcp (stdio)
    ├── ceremony engine
    ├── SQLite state, journal and published definitions
    └── claim / complete protocol
```

There is no MADE service, gRPC, NATS or PostgreSQL in this mode. The
plugin launcher selects `MADE_MCP_BACKEND=embedded` and defaults
`MADE_MCP_STORE_PATH` to:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3
```

Set `MADE_MCP_STORE_PATH` before startup to use a different file. SQLite WAL
allows multiple MCP processes to share it and serializes concurrent writes.

## The truthful execution contract

`made_run_ceremony_step` invokes the step handler configured inside the
engine. The plugin's default embedded handler is a no-op and must not be used
as evidence that an external search, scrape, model invocation or artifact
creation happened.

For work performed by Codex or another host, use this protocol:

1. Publish the reviewed definition with
   `made_publish_ceremony_definition`. Publication binds its name and version
   to an immutable digest.
2. Start it with `made_start_published_ceremony`. Ad-hoc
   `made_start_ceremony` runs are intentionally unbound and cannot reload
   their supplied definition after a process restart.
3. Read `next_step_id` from the returned instance.
4. Claim exactly that step with `made_claim_ceremony_step`. Use a stable
   `lease_owner_id` for the host and a stable `idempotency_key` when retrying
   the same claim. Choose `lease_ttl_ms` long enough for the bounded action.
5. Perform the stage instructions with the host's real tools. Respect normal
   tool permissions; a ceremony coordinates authority but does not create it.
6. Only after the work exists, call `made_complete_ceremony_step` with its
   real status and a structured `output` containing results and evidence
   references. A failed result must carry `error`.
7. Apply an enabled transition with
   `made_apply_ceremony_transition`, then repeat from step 3.
8. Stop only when `completed: true`, a human guard is waiting, or the instance
   records a genuine failure/cancellation.

The minimum success loop is therefore:

```text
publish → start_published → claim → perform → complete → transition → repeat
```

Never call `complete` merely because the host attempted the instructions. A
successful completion means the output and its cited artifacts are actually
available.

## Suggested completion output

Keep outputs machine-readable and small. Store large evidence in its owning
system and reference it:

```json
{
  "summary": "Observed five recurring compact-car cues.",
  "artifact": "candidates/trends/compact-car-2026-08-14.json",
  "source_refs": ["https://example.test/reference"],
  "counts": {"sources_read": 8, "candidates_kept": 5}
}
```

Do not put credentials, cookies or private session material in ceremony
context, outputs or the audit journal.

## Recovery after restart

1. Start the plugin against the same `MADE_MCP_STORE_PATH`.
2. Call `made_list_ceremony_instances`; do not create a replacement first.
   Entries the store cannot rehydrate come back as
   `{"ceremony_id": …, "rehydratable": false, "reason": …}` instead of
   failing the whole listing — state that exists is reported, not hidden.
3. Refresh the matching instance with `made_get_ceremony_instance`.
4. If a step is `in_progress`, inspect its lease and any durable artifact
   before retrying. Reuse the same idempotency key for the same claim.
5. Continue with the returned next step, enabled transition, guard or open
   intervention.

A published instance reloads both its snapshot and immutable definition from
SQLite. An ad-hoc supplied definition does not make that durability claim; use
the publication path for autonomous machinery.

## Human guards and interventions

If `waiting_for_human` is non-empty, do not infer approval. Obtain the current
person's explicit decision before calling `made_approve_ceremony_guard`.
Autonomous ceremonies should omit human guards rather than impersonate a human
approver.

Participant interventions remain agenda items. They can request an opinion,
investigation or action, but cannot bypass host permissions or a ceremony
guard.

## Observability and completion events

The durable audit journal is the source for semantic ceremony history. Claim
and completion produce the engine's step lifecycle records in the same SQLite
unit of work as the instance snapshot. `RUST_LOG=made_mcp=debug` adds MCP
tool-call diagnostics on stderr; stdout is reserved for JSON-RPC.

Use both layers deliberately:

- audit journal: what the ceremony says happened, in order;
- MCP logs: adapter startup, request failures and tool-call diagnostics;
- referenced artifacts: the actual external evidence produced by a stage.

A consumer that needs a finalization notification should observe the terminal
ceremony event from the audit/outbox contract, not infer completion from a UI
window closing. Exporting that event to an external observer is a separate host
integration; embedded SQLite remains the authoritative local record.

## Troubleshooting

| Symptom | Meaning / action |
|---|---|
| `MADE_MCP_STORE_PATH` is missing | Direct embedded binary startup is incomplete. Set an explicit path; the plugin launcher supplies the default. |
| database busy/locked error | A write held the SQLite commit lock longer than the configured timeout. Retry the operation and inspect the competing process if it persists. |
| `not found: ceremony_definition` after restart | The instance was started from supplied YAML. Publish it and start a new bound instance with explicit recovery provenance. |
| listing shows `"rehydratable": false` | Same boundary, seen from the listing: that instance's definition was never published, so only its stored snapshot remains. Reading it by id still fails. |
| completion rejected | The step was not claimed, is no longer in progress, or the result shape is invalid. Refresh the instance before retrying. |
| failed result rejected | Supply a non-empty `error`; non-failed statuses must omit it. |
| ceremony says complete but no real artifact exists | The no-op handler path was used or the host filed a false completion. Treat the run as invalid and use claim/perform/complete. |
