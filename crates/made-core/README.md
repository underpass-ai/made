# made-core

The domain core of [MADE by Underpass](https://github.com/underpass-ai/made) —
the Multi-Agent Deliberation Engine. Entities, value objects, domain
events and ports. No IO, no transport, no framework glue.

Everything else in the workspace depends on this crate; this crate
depends on nothing of ours. If a type here needed a socket, a clock or a
database to make sense, it would belong somewhere else.

## What lives here

- **Entities and aggregates** — councils, deliberations, ceremony
  definitions and instances. An aggregate protects its own transitions;
  callers never mutate its fields.
- **Value objects** — the vocabulary the boundaries speak. No primitive
  obsession: a specialty, a score or a ceremony id is a type, not a
  `String` or an `f64`, and its invariants are checked at construction
  rather than trusted afterwards.
- **Events** — what the engine says happened, in the shape the AsyncAPI
  contract publishes.
- **Ports** — the traits the application layer composes and the adapters
  implement: repositories, agents, executors, messaging, clock, metrics.

Domain errors are typed. IO errors do not exist here, because IO does not.

## Features

`conformance` ships a reusable test suite that any adapter implementing a
port can run against itself, so a new repository or agent proves it obeys
the same rules as the in-tree ones instead of hoping it does.

## Stability

This is a library published so the rest of the engine can be published,
not a curated public API. It moves with the engine's releases. Consumers
who want a stable surface should use
[`made-api`](https://crates.io/crates/made-api), which is versioned by
meaning rather than by release.

## License

Apache-2.0.
