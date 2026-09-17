# A ceremony store written by MADE v0.3.1

`ceremonies.sqlite3` was produced by `made-mcp` built from the tag
**`v0.3.1`** (commit `c7dad9f`), driven over stdio on its embedded
SQLite backend. It is the store `made-mcp migrate-store` is proved
against: a file whose instances were written by the code that wrote
such files, before a ceremony was its event stream.

- Produced on 2026-09-17 by [`produce.sh`](produce.sh), which
  re-derives it from the tag.
- 53,248 bytes.
- Tables: `ceremony_instances` (2 rows), `audit_journal` (13 records),
  `published_definitions` (2), `outbox` (0), `state_migrations` (0).
  There is no `ceremony_events` table: v0.3.1 had none.

## The session that produced it

Every call went through `made-mcp` v0.3.1's MCP tools:

1. `made_publish_ceremony_definition` — [`pre_stream_session.yaml`](pre_stream_session.yaml)
   at version `1.0`.
2. `pre-stream-complete`: started from the published definition, both
   steps claimed and completed, both transitions applied. It ends in
   the terminal state `CLOSED` at revision 7, bound to the published
   digest.
3. `pre-stream-midflight`: started from the same definition, `gather`
   claimed and completed, the `gathered` transition applied, and the
   intervention `still-open` requested and left open. It sits in
   `SETTLING` at revision 5 with `settle` still pending, so the next
   claim after an import is a real next move rather than a repeat.
4. `made_publish_ceremony_definition` again — the same definition at
   version `1.1`, so the catalog carries two versions.

## What it proves

The migration test imports both instances, checks that the fold of each
new stream equals the snapshot that was imported, verifies the chain of
every migrated stream, and drives the mid-flight session's next claim.
Both editions read the same file, so a regression that lost a step
record, an intervention or a definition binding fails here rather than
on an operator's machine.
