# Upgrade an integration

This guide covers the **0.5.x → 0.6.0** upgrade. Version 0.6.0 changes client
contracts; installing v0.5.0 does not provide them. The release installation
requires public 0.6.0 assets and the stable catalogue at that release. Before
publication, use a [source candidate](../embedded/README.md#test-a-source-candidate).
Match clients, plugin files and binaries, then inspect the running capability
catalogue.

## Unreleased lifecycle writers and snapshot v2

New writers seal pause, resume, cancellation and explicit deadline events.
Historical event and snapshot bytes are unchanged: absent lifecycle/deadline
fields retain their old representation, and old completion derives its
terminal phase from `completed_at`. Current readers accept both legacy and v2
snapshot envelopes.

After a lifecycle writer appends, a v2 snapshot prevents a 0.6.0 reader from
loading a partial tail and admitting work while ignoring a pause or terminal
event. Upgrade every writer before enabling lifecycle controls, and do not use
an older binary as a rollback writer. See the [snapshot v2 compatibility
barrier](snapshots-v2.md) for the exact reader matrix and recovery procedure.

## Unreleased durable councils

Configured local stores now retain councils, agent descriptors, contracts,
deliberations, statistics and their independent journal/cursors. PostgreSQL
adds the journal and contract tables without rewriting existing council data.
Legacy imports record one snapshot provenance fact instead of inventing past
phase events. See [council durability and migration](councils-durable.md) for
export/import validation, provider credential handling and consumer recovery.

## Unreleased classified failures

The Unreleased writer retains a typed handler `NoValidProposal` in the
`failure_kind` field of its sealed `StepFailed` result, using payload version
4. Other failures retain their prior schema selection. Existing result
payloads omit the field and retain their canonical bytes and hashes.
The classification is read from events; no historical error text is parsed
to invent it. Version 0.6.0 does not read `StepFailed` v4, so use an upgraded
reader before allowing these writers to append to a shared store.

## Completion fence in 0.6.0

The accepted claim response adds `claim_fence` (protobuf field 2). Completion
requires that unchanged value (request field 7). Both MCP backends return it
at the top level of `structuredContent`, and their completion schema requires
it. Capture it before external work; never look up a newer claim and attach
an old result to that identity.

| Input | Result |
|:--|:--|
| Missing, empty or malformed fence | Invalid request / gRPC `INVALID_ARGUMENT` |
| Valid but wrong or superseded fence | Refused / gRPC `FAILED_PRECONDITION` |
| Accepted fence for the current claim | Normal result validation and append |

A refused result appends nothing and leaves the replacement lease and
output/context unchanged. This prevents stale completion acceptance; it
does not cancel effects outside MADE.

Rust `EmbeddedMade::start_step` returns `StartCeremonyStepOutput`. Replace
uses of the old returned `StepAttempt` with `claim.attempt()`. Retain
`claim.claim_fence().clone()` and pass it as the fifth argument to
`CompleteCeremonyStepInput::new(id, step, result, actor_kind, fence)`.
The output also exposes the accepted instance and version. Application-owned
`run_step` retains its own fence automatically.

Fencing alone changes no stored event schema. The integrated fence includes
the state visit, state iteration, step iteration and attempt along with the
exact lease identity. Existing clients must be upgraded before using the new
mandatory completion boundary.

## Durable state visits in 0.6.0

New transitions record a destination entry and exact step reset set. Each
entry increments a ceremony-wide positive `state_visit`, including self
transitions; within-state repetition does not. New step and context facts
carry that coordinate, so repeated step ids in later visits remain distinct.

Old streams are not rewritten. Absent historical visits mean visit 1 and
remain absent when reserialized. Old transition payloads retain their old
fold. The first new transition after an old prefix opens visit 2. Snapshots
remain caches of this fold; recovery must work from every snapshot cut and
from full replay.

| Event | Historical schema versions | Visit-bearing version |
|:--|:--|:--|
| `StepStarted` | 1, 2, 3 | 4 |
| `StepCompleted`, `StepFailed` | 1, 2 | 3 |
| `TransitionApplied` | 1, 2 | 3 |
| `ContextWritten`, `StateIterationStarted` | 1 | 2 |

New readers reject inconsistent version/payload combinations. Older binaries
must not be assumed to read newly appended schemas; plan rollback around
store compatibility, not only an executable downgrade. Preserve a consistent
backup before upgrading writers.

## Bounded command lists in 0.6.0

New commands validate `ceremony_ids`, `reconsider_when` and `target_role_ids`
as unique lists of at most 100 items across Rust, direct gRPC and both MCP
backends. Report ids and reconsideration conditions must be nonempty. Empty
or omitted intervention targets means the whole table. Uniqueness is checked
after whitespace normalization. Report ids and conditions retain input order;
scoped role ids retain their ordered-set shape.
Invalid lists reject the request rather than being shortened or deduplicated.

Rust `GenerateCeremonyReportInput::new` now returns a result; handle the
validation error, or use `from_ids` with a validated `CeremonyReportIds`.
Typed `CeremonyInterventionTarget::Roles` still requires a nonempty validated
set; the empty-list-to-table rule applies at the transport boundary.
Historical oversized lists remain deserializable for replay. Commands validate
again, so replay compatibility cannot bypass current input rules. Historical
target payloads are sets; duplicate role ids are rejected during their
construction/deserialization. These list changes add no event schema version
or response field.

## Counted joins through durable publication

`steps_completed:n` definitions now round-trip through published-definition
serialization, SQLite reopening, direct gRPC and both MCP backends. Earlier
code could analyze a counted join successfully but fail when serializing it
for publication. The corrected tagged object representation removes that failure;
the guard still counts successful work in the state being left. The
[two-check example](../authoring/examples/two-checks.yaml) intentionally uses
`steps_completed:2` so the full publication path exercises this contract.

## Catalogue identity

The 0.6.0 source catalogues are `made` in this repository. The stable
`marketplace` branch advances only after the matching release assets are
public; its v0.5.0 snapshot has the old `underpass` metadata. Follow the
[plugin migration](../plugins/README.md) to inspect and replace an old
registration without duplicating MCP servers or deleting ceremony data.

## Pre-stream SQLite sessions

Stop every process writing the store, preserve a consistent backup, then run
the current embedded-enabled binary's explicit importer:

```bash
made-mcp migrate-store /absolute/path/to/ceremonies.sqlite3
```

It imports pre-event-stream snapshots into sealed streams on a working copy,
verifies the imported state and installs the copy only on success, preserving
the original as `.pre-stream.backup`. An already imported store is unchanged.
Do not run the file-copy migration while another writer is active. Inspect
the receipt, reopen, verify journals and check the remaining work before
resuming external execution. The retained
[v0.3.1 fixture provenance](../../crates/made-tests-integration/fixtures/stores/v0.3.1/README.md)
explains the historical store used by migration tests.

## Redb to SQLite

Current MADE opens SQLite only. A legacy `ceremonies.redb` requires the
one-time converter in made-mcp v0.2.0:

```bash
made-mcp share-store /absolute/path/to/ceremonies.redb
```

Run that command with the v0.2.0 migration binary, not the current one.
The conversion preserves the source and verifies the copy. Point the current
launcher at the resulting `ceremonies.sqlite3`. If the launcher sees a legacy
default without a converted SQLite file, it stops before creating an empty
replacement.

A historical snapshot without a published definition is not made
rehydratable merely by changing storage format. Verify session listings,
definition identity and journal integrity after any store move.
