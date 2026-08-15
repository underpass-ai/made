# ADR-006: Ceremony reports are persisted-state projections

Status: Accepted

## Context

An MCP caller needs a portable account of one or more durable working sessions.
Letting the caller supply summaries would make the narrative unauditable, while
reading product artifacts would couple the engine to one consumer. Rendering
only the current snapshot would also omit the ordered audit facts that say what
happened.

## Decision

`made_generate_ceremony_report` is an embedded MCP read capability. It reads
each `CeremonyInstance`, resolves the exact definition it runs, and reads the
same store's `AuditJournalPort`. The embedded builder therefore accepts one
ceremony store implementing repository, unit-of-work and audit-journal ports;
the three views cannot be configured from different sources.

The MCP adapter owns Markdown rendering. The engine facade exposes ordered
audit-record reads, while domain and application types remain independent of a
presentation format. Reports preserve caller id order and contain no generated
time, random identifier or inferred prose. Empty lists, duplicate ids and any
unknown id fail the whole request explicitly.

Markdown headings and section order are public contract. Domain values are
rendered as deterministic pretty JSON. Code fences are always longer than the
longest backtick run in their content, and caller titles are escaped. Values are
not truncated: large persisted outputs produce large responses, so MCP client
and transport limits remain visible instead of silently changing evidence.

The result returns Markdown plus ids, completed/incomplete counts, definition
versions and available digests. `persisted: false` states that the tool creates
no file and performs no write.

## Consequences

- Reports can be regenerated after reopening a redb store and remain byte-stable
  while the selected ceremonies do not change.
- A report contains only facts available in the definition, snapshot and audit
  journal; it cannot repair missing evidence or reconstruct payloads the journal
  never recorded.
- The capability is not advertised by gRPC because no report RPC exists. Adding
  a remote report later requires an explicit protocol decision rather than a
  backend pretending to support it.
- Unbounded faithful output can exceed a client's response budget. Callers
  should request fewer ceremony ids rather than expect silent truncation.
