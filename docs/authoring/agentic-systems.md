# Compose ceremonies into an agentic system

A ceremony coordinates one procedure. An agentic system is the level above it:
a named system with business roles, logical participants, a collaboration
topology, several ceremonies composed together with dependencies and bounded
loops, a supervision policy, and an attention policy for the integrator.

The system references ceremonies; it does not own them. Every composition
carries an immutable pin of name, version and digest, so what a run composed
can be shown afterwards to be exactly what its design named.
[ADR 021](../adr/021-pinned-agentic-system-aggregate.md) fixes that contract and
is the authority when this guide and the code disagree.

Read [author a ceremony](README.md) first. A system is written in terms of
published definitions, so there is nothing to compose until at least one
ceremony exists.

## The model

| Concept | What it is |
|:--|:--|
| `AgenticSystemId` | Which system this is. Stable across every edition of it. |
| `AgenticSystemRevision` | Which edition. Starts at 1 and counts up. An edit states the revision it read; a concurrent edit gets a conflict instead of overwriting. |
| `AgenticSystemDigest` | Content identity: SHA-256 over the canonical form under its own domain separator, so a system digest can never be read as a ceremony definition digest. |
| Lifecycle | `draft`, `published` or `deprecated`. Only a published revision can be run. Deprecation says "start nothing new from this" without invalidating runs already sealed. |
| `SystemRole` | A business role with an id, a stated responsibility and a kind: `integrator`, `contributor`, `reviewer`, `approver` or `observer`. The kinds are about authority, not job titles, and validation leans on them. |
| `LogicalParticipant` | Somebody or something expected to take part: an id, the role it plays, `person` or `agent`, and a `ParticipantBindingPolicy`. It is not an agent — the same design run twice may be played by different ones. |
| `ParticipantBindingPolicy` | What whoever plays the participant must be: an optional `host_kind`, the `capabilities` it supplies, and an optional `independence_group`. A policy, not a selection: naming a concrete agent here would make the design a deployment. |
| `CollaborationLink` | One arrow of the topology: `from`, `to`, a kind (`communication`, `coordination`, `execution`, `definition`), an optional channel, and whether the work itself changes hands (`handoff`). A link from a participant to itself is refused. |
| `RequestedExecutionProfile` | Per role: requested model, requested reasoning effort, required capabilities and a fallback policy with an optional fallback model. Requested, and only requested — never what actually ran. Refusing fallback and declaring one is refused. |
| `CeremonyComposition` | One composed ceremony: its id inside the system, a `DefinitionPin` of name, version and digest, a purpose, `depends_on`, an `activation`, `role_bindings` from the definition's seats to participants, and `inputs_from` mapping its inputs to another composition's declared outputs. |
| `CeremonyActivation` | `manual` (somebody starts it), `after_dependencies` (it starts once everything it waits for has completed) or `loop` with `after` and `max_rounds`. |
| `SupervisionPolicy` | What the system insists on beyond the ceremonies themselves: `human_approvals` naming guards a person must answer, `independence` rules between two roles, and `role_actions` per role. Three separate things, so that no run decides any of them for itself. |
| `AttentionPolicy` | What the integrator asked to be told about and how insistently: which kinds wake it, a coalescing window, a queue limit and an overflow policy. |

The whole document is validated strictly at the boundary and unknown fields are
refused. A design is written by an agent as often as by a person, and a
misspelled key that is silently ignored is a supervision policy that quietly is
not there.

## What validation refuses

Validation is one pass that collects every defect rather than stopping at the
first, and each finding names the element it is about, so a design can be fixed
in one round. An error blocks publication.

| Refusal | Why it matters |
|:--|:--|
| A pin no published definition matches, or one whose published digest differs from the pin | The pin is the whole reason a sealed system means something; a pin nobody can resolve composes nothing. |
| A seat the pinned definition declares with no participant bound | The ceremony stalls at run time on work nobody can claim. |
| A binding to a seat the pinned definition does not declare | Somebody's expectation that will silently never happen. |
| An unknown participant, role or composed-ceremony identifier | A check that reasoned about a name nothing declares would be reasoning about nothing. |
| An integrator role that is not declared, is not of kind `integrator`, or that no participant plays | The integrator is who the system answers to and who it asks; a design without one has no outside. |
| An independence rule violated inside one composed ceremony — reviewer and reviewed are the same participant, or share an `independence_group` | Two names for one opinion is not an independent review, which is the case that matters when both are agents. |
| A capability a role's requested profile requires that no participant playing that role declares | This is the defect that turns into a skipped ceremony during a run; it is cheaper to refuse it before publication. |
| An `inputs_from` entry naming an input the composition's own pinned definition does not declare | The wiring would deliver a value the ceremony never reads. |
| An `inputs_from` entry naming an output the producing definition does not declare, or a producer this system does not compose | Work would be read from somewhere that promises nothing. |
| A dependency cycle with no member declaring a bounded loop | Read literally, a cycle means nothing can ever start. |
| A loop whose `after` is not in a cycle with it | A dependency in looping clothes: no round could ever be sent back. |
| A ceremony that depends on itself | It could never begin, and no bound makes it able to. |
| A human approval over a guard the pinned definition does not declare, or over one that is not answered by a human | Naming an automatic guard as a human approval records a decision nobody made, which is the one thing a human guard exists to prevent. |
| Every composed ceremony waiting for another, so a run has nothing to begin with | A system that cannot start is not a system; it is a document. |

## Bounded loops

Work going round for revision is a system working as designed. Work going round
forever is a system nobody can wait for. The difference is a declared bound:

```json
{"kind": "loop", "after": "drafting", "max_rounds": 3}
```

That activation on `review` says review runs after drafting, so the dependency
pointing the other way — drafting waiting for review — is the back edge. The
back edge is cut when deciding what can begin, which is what lets drafting start
at all, and it is kept for the analysis that asks whether the cycle is bounded.
Without the cut, a system that sends work out to be reviewed could never start;
without the full graph, an unbounded cycle would pass validation.

When the looping composition completes and its bound is not yet reached, every
member of its cycle is reopened for the next round. Reopening the anchor alone
would leave the loop itself settled and the round would never come back to it.
Rounds are bounded per composition by `max_rounds`; reaching the bound settles
the cycle rather than restarting it.

## A worked example

[`api/examples/agentic-systems/integrator-delivery-system.json`](../../api/examples/agentic-systems/integrator-delivery-system.json)
composes the published
[integrator delivery ceremony](examples/integrator-delivery.yaml) with a bounded
review loop. That definition declares one required input, `delivery_brief`, five
seats — `API_IMPLEMENTER`, `STORAGE_IMPLEMENTER`, `INTEGRATOR`,
`INDEPENDENT_REVIEWER` and `HUMAN_APPROVER` — and a `human_approved` guard of
`type: human` that alone opens the final transition.

A run reads as one path:

**Request.** Somebody asks for a delivery. The host task that receives the ask
instantiates a published revision of the system, supplying the brief. The run
is idempotent by the caller's execution identity: asking again hands back the
run that already exists, after checking it is running the same design.

**Delegation.** Instantiation materializes each logical participant through the
host and starts what can start. The two implementer seats and the integrator
seat are filled by whatever the host produced for those participants; the design
never named them. The integrator binding records where the integrator wants to
be reached.

**Execution.** The composed ceremony runs as an ordinary ceremony instance,
through the [claim, work and complete loop](../runtime/README.md). The system
adds no second execution engine: it links each composition to a real instance
and reads that instance's own folded state back.

**Evidence.** Completion records and evidence stay where they already live, on
the ceremony instance. A composition's status is observed from its instance, not
hoped for, and an input a later composition receives is taken from the context
an earlier one actually left behind.

**Review.** `INDEPENDENT_REVIEWER` reviews the integrated result. The
supervision policy's independence rule between the reviewer role and the
integrator role is what makes "independent" a checked fact rather than a
convention: validation refuses a design in which one participant, or one
independence group, sits in both seats of the same composed ceremony.

**Response.** The human approver answers `human_approved`. Only then does the
final transition become enabled, and only then is the delivery closed. The run's
own state is folded from its links, so the summary and the detail cannot
disagree: a failed composition fails the run even while others are still going,
because what it promised is no longer deliverable.

Wiring between compositions is checked against the definitions, in both
directions. `integrator-delivery.yaml` declares no `outputs`, so nothing
downstream can read from it until it does; a composition that tried would be
refused at validation rather than at run time.

## The interaction surface

Four things are easy to confuse and mean different things. Each has its own
tool, and all four tools act on one ceremony instance, not on the system.

| Interaction | What it means | Tool |
|:--|:--|:--|
| A question for the integrator | A participant raises a question, an investigation or an action request. It records the ask; it neither bypasses a guard nor authorizes an external action. | `made_request_ceremony_intervention`, answered with `made_respond_to_ceremony_intervention` |
| A change of plan | Moving the session along an edge the definition declares, once its guards are enabled. A change the definition does not declare is a new system revision, republished and run again — a sealed design is not edited in flight. | `made_apply_ceremony_transition` |
| A cancellation | Ending one ceremony instance. It is the instance's end, not the system run's: other compositions are unaffected and the design is untouched. | `made_cancel_ceremony` |
| An approval | A person's actual decision on a human guard. A deferral is not a refusal: it preserves the statement, the reason and the conditions for reconsideration, and leaves the guard unsatisfied. | `made_approve_ceremony_guard`, `made_defer_ceremony_guard` |

None of these advances the system. Advancing a run is
`made_advance_agentic_system_execution`, which starts whatever the run is now
ready for and sends bounded loops round again. It is idempotent by
construction: what can start is derived from what the instances say rather than
from a cursor, and each instance has a deterministic identity, so advancing
twice with nothing having changed starts nothing twice.

## What MADE controls and what the host executes

MADE coordinates, validates, seals and records. It decides what may start, what
a pin resolves to, whether a design is publishable and what a run has observed.

The host decides which agent or person shows up for a logical participant, and
performs the work. The design states what would do; the host says what there is.

When a required capability is not available, the participant is materialized as
`unavailable` with a stated reason, and every ceremony that needed it is
`skipped` with a reason. A skipped composition is neither a success nor a
failure, and it releases nothing downstream: it produced no outputs, so anything
reading them would be reading nothing. Nothing is ever simulated to make a
design look satisfiable — a design proved by a stand-in has been proved about
nothing, and "unavailable" on its own is not an answer anybody can act on, which
is why the reason is stated rather than inferred.

## Requested capabilities against available ones

`RequestedExecutionProfile` is intention. It says what a role's work asks of
whatever executes it: a model, a reasoning effort, required capabilities and
what to do when they cannot be had. No field in a design is named for what
actually ran, because that would be a promise the design cannot keep.

What actually ran lives where it already lived: in the `ExecutionProfile`
recorded on the claim by the host. Instantiation turns a requested profile into
the request on that claim, and the claim records the selection. Views of a run
therefore keep the two apart, and a reader can always tell what was asked for
from what happened.

This is the surface of this source tree, not of the published package. The
changelog carries it under `Unreleased`, to publish as 0.8.0; the published
0.7.8 package does not contain it.

## The diagram

`made_render_agentic_system_diagram` returns two things: Mermaid text and its
text equivalent. The text equivalent is not a spare part. It is generated from
the same design, so a reader using a screen reader, a reader in a terminal and a
reader reviewing a diff get the same content the drawing carries, and the two
cannot say different things.

Edges carry meaning, because a picture in which every line means the same thing
is unreadable:

| Edge | Collaboration |
|:--|:--|
| `-->` | communication |
| `-.->` | coordination |
| `==>` | execution |
| `---` | definition |

The same key is drawn into the diagram itself, so a picture pasted elsewhere
arrives with it.

The flowchart is laid out in lanes: `Users`, `Host task`, `MADE`, `Architecture`
and `Ceremonies`, with the integrator drawn in the architecture lane and the
spine running from the users through the host task and the integrator into
MADE. That spine is the path a piece of work actually takes; drawing
participants without it produces a correct graph nobody can read.

When a run is supplied, each composition carries a CSS class for what the run
observed: `linkPending`, `linkStarted`, `linkCompleted`, `linkFailed` or
`linkSkipped`. A design on its own gets no classes at all, because it has no
observed state and calling everything "pending" would say something about a
reality nobody has looked at.

Rendering to SVG or PNG is the host's job. MADE vendors no renderer — not
`mermaid-cli`, not a browser engine, nothing. Shipping one to draw a box would
cost more than the box is worth, and every host that would display this can
already render Mermaid.

## The nine tools

| Tool | What it does |
|:--|:--|
| `made_design_agentic_system` | Saves a design document as a new revision, against the revision the author read. |
| `made_get_agentic_system` | Returns one system, at its current revision. |
| `made_list_agentic_systems` | Pages through the systems that exist. |
| `made_validate_agentic_system` | Runs the whole analysis against the definitions the pins resolve to, and returns every located finding. |
| `made_publish_agentic_system` | Seals a revision after resolving every pin and refusing any blocking finding. |
| `made_instantiate_agentic_system` | Opens a run of a published revision, materializes participants and starts what can start. Idempotent by execution identity. |
| `made_advance_agentic_system_execution` | Settles what the instances say, starts what is now ready and sends bounded loops round again. |
| `made_get_agentic_system_execution` | Returns one run, keeping what the design intended apart from what the instances observed. |
| `made_render_agentic_system_diagram` | Renders a design, and optionally one run of it, as Mermaid text and its text equivalent. |

All nine authorize against the global scope in this version, by decision rather
than by omission: the scope vocabulary is a public contract, and this cut would
rather authorize globally than add a scope it cannot yet enforce well. A
dedicated system scope is left for a later cut.

## Declared limits of this version

Authorization uses the global scope, as above.

Pins are resolved when a system is validated and published. A definition
deprecated after that does not retroactively invalidate a system already sealed:
the sealed revision keeps composing the digests it named.

The diagram is Mermaid text and its text equivalent. Turning either into an
image is outside MADE.
