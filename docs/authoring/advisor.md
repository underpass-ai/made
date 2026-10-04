# The advisor ceremony

The advisor ceremony puts a second, usually stronger, reviewer beside the role
that does the work. The reviewer reads the whole transcript, never does the
work itself, and is consulted at the two moments where advice is cheapest and
worth the most: **before** substantive work starts, and **when the executor
believes the work is done**, with the deliverable already durable. It is the
shape of the advisor tool in Claude Code, shipped as a canonical stage
pattern: `kind: advisor`. It requires MADE 0.10.0.

Use it when one role owns a deliverable and a mistake in its approach is
expensive to discover late: a change to a shared contract, a migration, an
incident write-up. Use [`maker_checker`](../../api/examples/ceremonies/fragments/maker_checker.yaml) instead when the
second role is a gate that approves or rejects a finished artifact and has
nothing to say before it exists.

## Declare it

```json
{
  "id": "delivery",
  "pattern": {
    "kind": "advisor",
    "roles": ["EXECUTOR", "ADVISOR"],
    "fallback_role_id": "HUMAN",
    "max_iterations": 2,
    "instructions": "Implement the change and its tests."
  }
}
```

| field | rule |
|:--|:--|
| `roles` | exactly two, in order: the executor, then the advisor |
| `max_iterations` | required; how many work/review passes before escalation |
| `fallback_role_id` | required; who decides when advice still blocks at the cap |
| `instructions` | the executor's work; the pattern appends the durability and advice rules |

Which model backs the advisor is decided where the role is bound to an agent,
not in the pattern. Bind `ADVISOR` to the stronger model and `EXECUTOR` to the
faster one; the definition stays the same if that choice changes.

## What it expands to

For a stage `delivery` with `max_iterations: 2` the designer produces:

| state | steps, in order | exits |
|:--|:--|:--|
| `DELIVERY_PLAN` | `delivery_orient` (executor), `delivery_advise` (advisor) | `DELIVERY_ITERATION_1` |
| `DELIVERY_ITERATION_1` | `delivery_work_1` (executor), `delivery_review_1` (advisor) | `blocks=false` → deliver; `blocks=true` → iteration 2 |
| `DELIVERY_ITERATION_2` | `delivery_work_2`, `delivery_review_2` | `blocks=false` → deliver; `blocks=true` → fallback |
| `DELIVERY_FALLBACK` | `delivery_fallback` (fallback role) | `DELIVERY_DELIVER` |
| `DELIVERY_DELIVER` | `delivery_deliver` (executor) | next stage or `COMPLETED` |

Every step sees prior contributions, so the advisor reviews the same evidence
the executor gathered. The states carry the `x-pattern: advisor` annotation for
diagrams and reports; behavior lives only in the guards and transitions.

## The contract of each step

- **orient** — read the material, locate what the task touches and state the
  intended approach. No deliverable yet. Orientation is not substantive work.
- **advise** — the first consultation. The advisor names the approach to take
  and the traps to avoid, before anything is built on an assumption.
- **work_n** — the executor does the work and makes the deliverable durable
  (written, saved, committed) before returning, so a review that takes time
  never holds the only copy. It gives the advice serious weight; where its own
  evidence contradicts the advice, it says so and shows the evidence instead
  of silently switching.
- **review_n** — returns `blocks` (boolean) and `advice`. `blocks` is `true`
  only when a concern changes the result. The advisor does not approve or
  reject; it says whether the advice must be acted on before delivery.
- **fallback** — reached only when advice still blocks after the last pass.
  A person, or the declared fallback role, decides.
- **deliver** — the executor delivers the reviewed result and keeps any advice
  left unresolved in the record.

`blocks` is projected from the reviewer's output with
`project_winner_fields`, so a deliberating handler may back the advisor seat
as well as a host callback.

## Canonical fragment

The shipped fragment is
[advisor.yaml](../../api/examples/ceremonies/fragments/advisor.yaml). It is the
designer's exact output for a stage `coordination` with roles `OPS` (executor)
and `SECURITY` (advisor), fallback `HUMAN` and two iterations. A test designs
that intent over gRPC and the embedded edition and compares the result byte
for byte with the file, and the contract gate keeps the packaged copies in
`made-app` and `made-mcp` identical to it. `made_design_ceremony` lists it in
its pattern catalog.
