# Hexagonal target and migration order

MADE uses crate boundaries as architectural boundaries. Dependencies point
inward; DTO mapping and infrastructure stay at the edge.

| Crate | Responsibility | Allowed inward MADE dependencies |
|---|---|---|
| `made-core` | Aggregates, entities, value objects, domain events and outbound ports | none |
| `made-api` | Stable embedded inbound contract and public DTOs | none |
| `made-app` | One application use case per operation; transaction orchestration | `made-core` |
| `made-proto`, `made-mcp-proto` | Generated/vendored transport DTOs | none |
| `made-adapters` | Inbound/outbound adapters, transport DTO mappers and persistence | `made-core`, `made-app`, `made-proto` |
| `made-embedded` | In-process composition root and `made-api` mapper | API, app, core, adapters |
| `made` | Deployable composition root | app, core, adapters, proto |
| `made-mcp` | MCP inbound adapter and embedded/remote composition | app, core, adapters, embedded, MCP proto |
| test/runner crates | Contract, integration and end-to-end drivers | any production ring they verify |

## Rules

- Domain APIs exchange validated value objects, not raw identity, content,
  quantity or status primitives.
- Aggregates are the only code allowed to change aggregate state.
- Ports are role-specific and contain no transport or vendor vocabulary.
- Incoming DTOs are converted by mappers before a use case is invoked; domain
  objects never deserialize transport concerns directly.
- A production source file owns at most one primary `struct`, `enum`, `trait`,
  `union` or public type alias.
- Production sections over 600 lines are migration debt and may only shrink;
  colocated unit tests and test-driver/support crates are excluded from this
  size metric, but still obey the one-primary-type rule.
- The full quality gate enforces at least 80% line coverage. Test-driver crates
  are not used to hide uncovered production code.

## Crate-by-crate migration

1. `made-core`: remove deployment concepts, split port DTOs from traits, and
   replace primitive identities/content/statuses with value objects.
2. `made-app`: keep one input, output and use case per file and split reusable
   application services from use-case orchestration.
3. `made-adapters`: split provider configuration, clients, wire DTOs and
   mappers; each adapter implements one narrow port.
4. `made-api` and `made-embedded`: keep public API DTOs independent of domain
   types and map at the embedded boundary.
5. `made`, `made-mcp`, proto and test drivers: keep only composition or
   transport orchestration; move reusable behavior inward to the appropriate
   ring.

`scripts/ci/architecture-gate.sh` makes the dependency rules absolute and the
remaining file/primitive debt a ratchet. The checked-in baseline is an
inventory, not an exemption: it can shrink but CI rejects growth or new debt.

## Branch migration status

On `refactor/hexagonal-ddd`, the inward rings have been migrated first:

- All 637 Rust source files have at most one primary public type. The gate also
  rejects any regression in that rule for production and test code.
- `made-core` has typed agent, execution, evidence, support and contract
  boundaries, no public primitive fields, and no deployment configuration
  port. Only the ceremony aggregate remains above the 600-line budget.
- `made-api` has one public contract DTO per source file and remains independent
  from the domain crate.
- `made-app` separates production use cases from their inputs and outputs and
  has no remaining structural debt in the baseline.
- `made-adapters` keeps configuration, wire DTOs, persistence records and
  runtime state outside the domain. Four large implementation modules remain
  as explicit migration debt.
- `made`, `made-api`, `made-embedded`, proto and test-driver crates have no
  remaining structural debt. `made-mcp` has two large transport modules left.

The current baseline contains 7 large-file debt entries, down from 70 at the
start of this branch. They are recorded in `conformance.tsv` and cannot grow;
new debt is rejected. A migration slice is complete only when the normal gate
passes and its paid entries are removed by refreshing the baseline.
