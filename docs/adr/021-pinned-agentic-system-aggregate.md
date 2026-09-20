# ADR 021 — The agentic system is a pinned design aggregate

Status: accepted for implementation; acceptance remains open.

Scope: issue #203, C7 lane C. This ADR fixes the contract; the lane implements
it.

## Context

A ceremony coordinates one procedure. The issue asks for the level above it: a
named system with business roles, logical participants, a collaboration
topology, several ceremonies composed together with dependencies and loops, a
supervision policy, and an attention policy for the integrator. It has to be
designable from intent, validated against reality, sealed, instantiated and then
observed while it runs.

Everything it composes already exists and already has identity: published
ceremony definitions with a name, a version and a digest. The design layer must
not become a second place where those definitions live, and must not become a
second execution engine.

## Decision

`AgenticSystem` references ceremonies; it does not own them. Every composition
carries an immutable pin of name, version and digest. Publishing the system
resolves each pin against the publication port and refuses when a pin does not
exist or its digest differs from the one the design recorded.

Persistence is a revision log with compare-and-swap: the whole document plus its
digest at each revision, saved against the revision the editor read. A
concurrent edit gets a revision conflict instead of overwriting. Creation is the
only save without an expected revision.

Execution is a different entity. `AgenticSystemExecution` has its own store and
links each composed ceremony to a real instance with a status, the resolved
requested profiles, how each logical participant was materialized, and the
integrator binding. Views of an execution distinguish what the design intended
from what is actually observed in the instances.

Profiles in the design are requested, never actual. `RequestedExecutionProfile`
states model, reasoning effort, required capabilities and fallback policy, and
instantiation turns it into the request on a claim. `ExecutionProfile` stays the
host's fact where it already lives. No field in the design is named for what
actually ran.

Validation is a single analysis producing located findings; an error blocks
publication. It refuses an unpublished pin or a changed digest, a ceremony role
with no participant bound, unknown participant, role or ceremony identifiers,
an integrator that is not an integrator role or has no participant, a violated
independence rule where reviewer and reviewed are the same participant or share
an independence group within one ceremony, a capability a role's profile
requires that no bound participant supplies, an input mapped to an output the
pinned definition does not declare, a dependency cycle with no bounded loop, a
loop anchored outside its cycle, a ceremony depending on itself, and a human
approval pointing at a guard that does not exist or is not a human approval.

A capability that is not available yields an unavailable materialization and a
skipped ceremony with a reason. Nothing is simulated to make a design look
satisfiable.

The topology is exported as a Mermaid diagram through a port, with edge styles
per collaboration kind and an accompanying text equivalent, so the picture is
never the only readable form.

## Alternatives rejected

Event sourcing the design aggregate. The journal exists to make the coordination
of real work replayable and auditable. A design document has a handful of
revisions produced by someone editing it, and no decision stream of its own.
Sourcing it would add event types, decisions, folds, goldens and a second stream
vocabulary to buy the one property that matters here — no lost update — which
compare-and-swap already gives. The engine would also end up with two aggregates
whose events look alike and mean different things.

Embedding ceremony definitions in the system. Copies drift from their
publication, and the system becomes a second place where a definition can change
without its digest changing.

Referencing ceremonies by name and version alone. A republished version would
silently change what the system composes, and validation would have nothing to
compare.

Reusing `CeremonyRevision` or the ceremony definition digest scheme. Distinct
aggregate, distinct domain separator; sharing them invites one identity to be
read as the other's.

Holding execution state on the aggregate. Every run would rewrite the design and
fight the compare-and-swap that protects human edits.

Declaring the actual model or host agent in the design. The design states
intent; the host decides what runs and the claim already records it. An `actual`
field in a design is a promise the design cannot keep.

A dedicated authorization scope for the aggregate. Deferred: the scope
vocabulary is a public contract, and this cut authorizes the aggregate and its
executions globally rather than adding a scope it cannot yet enforce well.

## Consequences

Three ports — repository, publication and execution store — with memory, SQLite
and PostgreSQL adapters and conformance suites including reopening. Nine
capabilities across the four surfaces in one capability group, with the design
document validated strictly at the boundary and rejecting unknown fields.

The same design JSON must produce the same YAML rendering and the same digest on
both backends, which is what makes the pin meaningful across compositions.

A reproducible example composes an already published ceremony with a bounded
review loop, and a walkthrough exercises intent, design, the failing validations,
the compare-and-swap conflict, republication, idempotent instantiation, real
evidence, advancing and the diagram, on SQLite with reopening.

## Declared limits of this version

Authorization for the aggregate and its executions uses the global scope; a
dedicated system scope is left for a later cut. Pins are resolved when a system
is validated and published; a definition deprecated afterwards does not
retroactively invalidate a sealed system. The diagram is Mermaid text and its
text equivalent — rendering to an image is the host's job and no renderer is
vendored.
