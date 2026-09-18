# Architecture decision records

Durable decisions about the MADE core, its contracts and its
distributions. Each record states what was decided and what it costs, not how
the code is organised — that lives in the architecture docs.

A decision is superseded, never rewritten: the reason it changed is worth as
much as the decision itself.

- [ADR-001](001-working-session-vocabulary.md): working sessions are the public
  name; `Ceremony` is the domain
- [ADR-002](002-ceremony-definition-analysis.md): analysis reports every defect;
  construction still fails fast
- [ADR-003](003-audit-journal-and-durability.md): the engine owns the audit
  contract; the host owns durability (its "not event sourcing" sentence is
  superseded by ADR-012)
- [ADR-004](004-published-embedded-api-contract.md): `made-api` is the
  contract a consumer compiles against
- [ADR-005](005-structured-ceremony-design-tool.md): ceremony design accepts
  structured intent and returns an unpublished draft
- [ADR-006](006-ceremony-reports-are-persisted-state-projections.md): ceremony
  reports project persisted state and journal records without inferred prose
- [ADR-007](007-mcp-self-description-uses-the-executable-catalog.md): MCP
  discovery and help are projections constrained by the executable catalog
- [ADR-008](008-legacy-redb-migration-is-copy-on-write.md), superseded by
  ADR-011: legacy redb state was imported read-only into a new, receipted MADE database
- [ADR-009](009-a-second-storage-engine-behind-one-seam.md), superseded by
  ADR-011: two storage engines sat behind one seam
- [ADR-010](010-bounded-step-repetition-is-not-retry.md): successful semantic
  repetition is bounded, durable and distinct from technical retry
- [ADR-011](011-sqlite-is-the-only-embedded-ceremony-store.md): SQLite is the
  sole canonical embedded ceremony store; Redb is a release-bound migration input
- [ADR-012](012-a-ceremony-is-its-event-stream.md): a ceremony is its event
  stream; snapshots are a cache, everything else is a projection (supersedes
  the "not event sourcing" sentence of ADR-003)
- [ADR-013](013-memory-is-mades-own-bounded-context.md): memory is MADE's own
  bounded context; SQLite is the reference implementation, kernels are
  out-of-tree adapters
- [ADR-014](014-the-local-edition-leads-and-the-api-keeps-parity.md): the
  local edition leads and the API keeps parity; divergence is a named row in
  a checked-in exception list
- [ADR-015](015-concurrent-states-and-join-guards.md): concurrent state claims,
  join guards and bounded parallelism
- [ADR-016](016-bounded-definition-primitives.md): state repetition, output
  guards, role binding, context writes, cycle budgets and fragment location
