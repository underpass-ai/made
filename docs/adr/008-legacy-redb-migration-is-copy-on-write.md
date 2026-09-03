# ADR-008: Legacy redb migration is copy-on-write

Status: Superseded by ADR-011

Implementation status last verified: 2026-08-15

## Context

The rename from Choreographer to MADE changed the domain separator used by
ceremony-definition digests. The definition content did not change, but an
instance persisted before the rename remains bound to its Choreographer digest
while current code seals the same publication with a MADE digest. Refusing to
rehydrate is correct: ignoring the mismatch would turn a verified identity into
a name-based assertion.

An old redb file may also have been copied while its process was running. redb
can recover such a database, but recovery requires a writable file. Opening the
only legacy copy writable would mutate the evidence before migration had been
proved successful.

## Decision

Migration is an explicit copy-on-write import between two paths. The source is
opened with a read-only file descriptor and streamed into a destination created
with `create_new`; an existing destination is never overwritten. Any redb
recovery runs only against that clone.

The importer validates each stored publication against exactly two supported
digest schemes: `underpass.choreo.ceremony-definition.v1` and
`underpass.made.ceremony-definition.v1`. A digest matching neither aborts the
destination transaction. Verified legacy publications are resealed under the
MADE scheme, and bound instances with the same name and version are rebound to
that new identity. Ceremony revision advances when its stored binding changes.
Unbound instances remain unbound; the importer does not invent a missing
definition.

Publication updates, instance updates and the migration receipt commit in one
redb transaction. Journal and outbox bytes already present in the cloned file
are preserved. The receipt records a stable migration id, the SHA-256 of the
source bytes, both digest schemes, bounded row counts and completion time.

Plugin startup performs the import automatically when a legacy source is
configured and the MADE destination is absent. A later start accepts an
existing destination only when its durable receipt is present. It then does not
reopen the legacy source.

Operational observability is a structured info event carrying the same
non-payload fields as the receipt. Ceremony YAML, context, outputs and evidence
are never logged by the migration.

## Consequences

- The legacy file remains byte-for-byte recoverable and independently
  inspectable.
- A failed import may leave a new destination without a receipt. MADE refuses
  to adopt it; an operator chooses another destination or explicitly removes
  it after inspection.
- The migration fixes the rename-induced digest mismatch without weakening the
  invariant that a bound instance must resolve to exact published content.
- An ad-hoc instance whose definition was never persisted can still be listed
  but cannot become rehydratable through this migration.
