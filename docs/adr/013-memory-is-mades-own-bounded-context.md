# ADR-013: Memory is MADE's own bounded context

Status: Implemented (decided 2026-09-16; verified 2026-09-18). The KMP adapter
left the tree in #41, recall and the trimmed port landed in E1/E2 (#84), the
SQLite reference adapter and durable composition landed in E3 (#102), and E5
closed the documentation in #121. The slices are
§3.8 of [`../orchestration-patterns-plan.md`](../orchestration-patterns-plan.md)

## Context

MADE has memory ports (`MemoryWriterPort`, `MemoryReaderPort`), value objects
and a ten-property conformance suite that name no kernel, no tool and no
JSON key. Two implementations pass the suite without any external system.
That part is right.

The rest is not. The engine writes memory after six kinds of commit and
never reads it: `MemoryReaderPort` has no consumer, so its shape answers
only to the backend it was drawn from. The one external adapter, for KMP,
sits behind an empty Cargo feature, is selectable by no configuration, is
wired into no binary, launches a binary by its pre-rename name with
pre-rename environment variables, and decides idempotency by matching the
kernel's English error text. Both composition roots ship a memory that
forgets everything. The memory scope is the instance id, so even a working
adapter could never let one ceremony recall another. KMP's vocabulary has
leaked into the domain in small ways: a doc comment citing a KMP issue
number, `"kmp:"` prefixes inside `DomainError` reasons, adapter-named spans.

The archived publication plan already said not to add an in-tree KMP adapter
if a caller-supplied context bundle was enough. The adapter was added anyway
and then never wired.

## Decision

**Memory is what a later ceremony needs to know from earlier ones, in
MADE's words.** Decisions, constraints, observations, outcomes and the
reasons between them, under a scope the definition declares. A kernel, a
graph, a vector store are adapters; their vocabulary maps at the boundary
through DTOs and never enters the domain.

**Recall is a use case.** A ceremony that declares a `memory_scope` input
recalls that scope when it starts, and the recollection is rendered into the
first brief as what earlier sessions decided, bounded in size, decisions and
constraints first. The default scope stays the instance id and means "no
shared memory", and says so.

**The port is trimmed to what MADE uses.** Capabilities no implementation
declares and types nothing reads (`AnsweringQuestions`, `MemoryQuestion`,
`MemoryDimension`) leave, or gain a consumer. Domain comments cite no other
product; error reasons name no adapter; spans are named for the port.

**SQLite is the reference implementation, in tree, on by default.** A
`SqliteSessionMemory` in the ceremonies store passes the conformance suite,
survives restart, and is what the embedded edition ships. The server
selects it or nothing. Two durable implementations of the port passing the
same suite are the evidence that the port is not one backend's shape — the
argument ADR-003 already makes for storage.

**Memory writes are a projection of the event stream** (ADR-012). The
recorder consumes sealed events through the subscriber port instead of
being called from use cases; it stays outside the transaction and never
fails a ceremony.

**The KMP adapter leaves the tree.** It becomes its own crate, depending on
`made-core` only, gated by the same conformance suite, with its names fixed
to the current KMP surface and its refusal matching replaced by structured
error codes once KMP exposes them. Where the crate lives — this
organisation, or the KMP repository — is a packaging decision recorded when
it moves. MADE's release does not depend on KMP's wording.

## Implementation result

MADE owns the memory value objects, recall use case, ports and conformance
suite. `SqliteSessionMemory` is the in-tree reference adapter and shares the
ceremony SQLite engine in path-aware embedded and server compositions (#102).
The generic `EmbeddedMadeBuilder` remains deliberately forgetful until its
host supplies memory. Memory writes are derived from sealed ceremony records
through the subscriber seam (#83). MADE ships no KMP adapter; an external KMP
adapter may implement the same port and must pass the same conformance suite.

## Consequences

- The embedded edition remembers across sessions with no second product
  installed; the cluster edition remembers when configured.
- The blueprint catalog's `rehydrate` phase has an engine mechanism behind
  it instead of a host convention.
- Removing dead capabilities is a contract change of the memory port; it
  ships with a version bump of the port and a conformance suite that no
  longer tests what was removed.
- The `kmp` Cargo feature and the CI matrix arm that built it go away from
  this repository.
- The platform table in `README.md` and the stack gap analysis describe KMP
  as one possible memory adapter that is not shipped here, which is what
  the code will then say too.
