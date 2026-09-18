# Upgrade an integration

The current published baseline is v0.5.0. This source documentation also
covers unreleased hardening; installing v0.5.0 does not provide those new
contracts. Match clients, plugin files and binaries deliberately, then
inspect the running capability catalogue.

## Completion fence after v0.5.0

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

Fencing alone changes no stored event schema. Combined with visits, the
fence includes the visit coordinate. Existing clients must be upgraded before
using the new mandatory completion boundary.

## Durable state visits after v0.5.0

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

## Bounded command lists after v0.5.0

New commands validate `ceremony_ids`, `reconsider_when` and `target_role_ids`
as unique lists of at most 100 items across Rust, direct gRPC and both MCP
backends. Report ids and reconsideration conditions must be nonempty. Empty
or omitted intervention targets means the whole table. Historical data
remains deserializable; stricter new command validation is not a rewrite of
old records.

## Catalogue identity

Source catalogues are now `made` in this repository. The stable
`marketplace` branch remains pinned to a published release; v0.5.0 still has
the old `underpass` metadata. Follow the
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
