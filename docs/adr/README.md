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
  contract; the host owns durability
- [ADR-004](004-published-embedded-api-contract.md): `made-api` is the
  contract a consumer compiles against
- [ADR-005](005-structured-ceremony-design-tool.md): ceremony design accepts
  structured intent and returns an unpublished draft
- [ADR-006](006-ceremony-reports-are-persisted-state-projections.md): ceremony
  reports project persisted state and journal records without inferred prose
- [ADR-007](007-mcp-self-description-uses-the-executable-catalog.md): MCP
  discovery and help are projections constrained by the executable catalog
- [ADR-008](008-legacy-redb-migration-is-copy-on-write.md): legacy redb state
  is imported read-only into a new, receipted MADE database
- [ADR-009](009-a-second-storage-engine-behind-one-seam.md): two storage
  engines sit behind one seam, so two agent hosts can share one ceremony store
- [ADR-010](010-bounded-step-repetition-is-not-retry.md): successful semantic
  repetition is bounded, durable and distinct from technical retry
