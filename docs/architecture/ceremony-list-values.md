# Ceremony list inputs

Issue #100 puts the list rules inside reusable values. MCP schemas advertise
those rules; the application and domain constructors enforce them for every
caller, including direct gRPC and the embedded facade.

| Input | Shared value | Accepted cardinality | Empty meaning |
|---|---|---|---|
| `ceremony_ids` | `made_app::usecases::CeremonyReportIds` | 1–100 distinct ids | Invalid request |
| `reconsider_when` | `made_core::value_objects::ReconsiderationConditions` | 1–100 distinct conditions | Invalid request |
| `target_role_ids` | `made_core::value_objects::InterventionRoleIds` | 1–100 distinct scoped roles | Whole table at the transport boundary |

Uniqueness is checked after the existing whitespace normalization. Report ids
and reconsideration conditions retain caller order. Scoped recipients retain
the existing ordered-set representation. Limits refuse the complete request;
they never truncate or silently deduplicate it. All four surfaces preserve their
existing error envelope: RPC `InvalidArgument`, MCP `invalid_request`, and typed
`DomainError` for the facade.

The proto repeated recipient field cannot distinguish an omitted list from an
empty one. Both already mean the whole table for proto and the versioned embedded
API; both MCP backends now apply that same normalization. A typed scoped
`CeremonyInterventionTarget::Roles` still requires a nonempty validated value.
`GenerateCeremonyReportInput::new` is now fallible; `from_ids` accepts an already
validated `CeremonyReportIds`.

Persisted events are facts that may predate these input limits. Deferral and
recipient values retain their historical JSON shape and accept historical
cardinalities during replay. New aggregate commands validate those values again,
so a deserialized historical value cannot bypass today's command limits.
Historical target payloads were already sets; duplicate target entries are
rejected during deserialization rather than silently erased. No event schema
version or response field changes.

`ceremony_list_boundaries` drives empty, exact-cap, oversized and duplicate
inputs on all four surfaces, including whitespace-normalized duplicates and
atomic refusal. It also exercises the deserialized-value command boundary.
`legacy_ceremony_list_replay` reads version-1 payloads with 101 recipients or 101
repeated conditions and rehydrates them without rewriting their data. Unit tests
pin the schema cap to all three values even though the gRPC-only MCP build has
no engine dependency.

```bash
cargo test -p made-core --test legacy_ceremony_list_replay
cargo test -p made-tests-integration --test ceremony_list_boundaries
cargo test -p made-mcp --lib list_schemas_publish_the_limits_enforced_by_the_shared_values
```
