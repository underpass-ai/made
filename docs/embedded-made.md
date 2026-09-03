# MADE Embedded

Status: implemented first slice. The embedded surface currently covers the
ceremony engine, with process-local defaults and a durable SQLite
composition. Native embedded facades for the broader council and deliberation
APIs are not claimed yet.

## One engine, two distributions

MADE has two consumption modes. They share `made-core`,
`made-app`, domain invariants and the workspace release version.

| Distribution | Entry point | Owns | Does not require |
|---|---|---|---|
| Deployable | `made` binary | process config, gRPC/HTTP servers, optional NATS and Postgres wiring | an embedding host |
| Embedded | `made-embedded` library | in-process facade and host adapters | sockets, gRPC, NATS, Postgres or environment configuration |

The embedded crate is not a second ceremony engine and does not duplicate
domain behavior. It calls the same application use cases used by the
deployable composition.

```text
                  made-core
            domain + ports + invariants
                       ^
                       |
                   made-app
                  use cases
                 /         \
                /           \
       made-embedded      made
       host callbacks       gRPC/NATS/HTTP
       injected ports       deployment config
```

Both distributions report the same Cargo workspace version. Ceremony
definitions retain their own independent `CeremonyVersion`; release version
and definition version solve different compatibility problems.

## Architectural boundaries

- The domain remains transport-, provider- and product-agnostic.
- The embedded facade tells application use cases what to do; it does not
  mutate aggregates or persistence state itself.
- Every replaceable dependency is a `made-core` port.
- Every concrete host integration is an adapter.
- Domain aggregates continue to own state transitions and invariants.
- One production class lives in one source file.
- The host retains ownership of its async runtime and the lifecycle of injected
  resources.

## Default embedded adapters

`EmbeddedMade::default()` deliberately chooses a safe local profile:

| Port | Default adapter |
|---|---|
| ceremony definitions | `InMemoryCeremonyDefinitionRepository` |
| ceremony instances | `InMemoryCeremonyInstanceRepository` |
| ceremony transcript | `InMemoryCeremonyTranscriptStore` |
| step execution | `NoopCeremonyStepHandler` |
| clock | `SystemClock` |
| metrics | `NoopMetricsRecorder` |

These defaults start no service and perform no remote IO. They are suitable for
single-process workflows, tests and hosts that begin with ephemeral state.
They are not a durability claim: a host that must resume after process loss
injects persistent implementations of the same repositories and context port.

## Durable SQLite composition

`EmbeddedMade::open(path)` supplies a
`SqliteCeremonyStore` to the ceremony-store and definition-publication ports.
That composition persists ceremony snapshots, unit-of-work state, the audit
journal, outbox rows and published definitions across process restarts. Its
crash/reopen behavior is exercised by
`crates/made-embedded/tests/sqlite_store_api.rs`.

The constructor does not silently make every port durable. Mounted definition
repositories and ceremony transcripts still use their default in-memory
adapters unless the host injects replacements. Step execution and evidence
collection also keep their default no-op adapters unless the host supplies real
implementations. SQLite persistence therefore proves durable ceremony state; it
does not prove that external work occurred.

One consequence deserves emphasis: an instance started from a mounted
(unpublished) definition persists its state but cannot rehydrate after the
store reopens — loading it fails with `not found: ceremony_definition`. A host
that must resume instances across restarts publishes the definition first and
starts instances from the published identity.

## Host callback adapter

`CallbackCeremonyStepHandler` turns an async Rust callback into a
`CeremonyStepHandlerPort`. It is the smallest useful boundary for a host that
wants its own agent runtime, tool system or human interaction to execute a
ceremony step.

```rust,no_run
use made_core::value_objects::{StepOutput, StepResult};
use made_embedded::EmbeddedMade;

let MADE = EmbeddedMade::builder()
    .with_step_handler_callback(|request| async move {
        let _kind = request.handler_kind();
        // Delegate to the host's own agent/tool/human subsystem here.
        StepResult::completed(StepOutput::empty())
    })
    .build();
```

For richer integrations the builder accepts `Arc<dyn ...Port>` for:

- definition repository;
- instance repository;
- transcript store;
- step handler;
- clock;
- metrics recorder.

The host keeps the concrete adapter handle when it needs adapter-specific
administration. The embedded facade does not expose a service locator.

## Ceremony API

The first slice exposes commands and queries required for both one-shot and
human-active execution:

- mount one or more typed definitions, or one YAML definition;
- run a ceremony to completion;
- start a ceremony without advancing it;
- start, run or complete an individual step;
- approve a human guard;
- apply an authorized transition;
- retrieve definitions, an instance and its transcript.

Mounting and queries pass through `made-app` use cases. Execution passes
through the existing ceremony use cases; the embedded crate contains no second
state machine.

## Dependency boundary

`made-embedded` depends on `made-adapters` with default features disabled.
The adapter crate now gates the outbound Runtime gRPC client behind
`runtime-grpc`, separately from the inbound `grpc`, `nats` and `postgres`
features. The embedded dependency tree contains none of `tonic`, `async-nats`
or `sqlx`.

The deployable binary enables `grpc`, `nats`, `postgres` and `runtime-grpc`
explicitly, preserving its existing deployment capabilities.

## Current limits

- The embedded facade currently covers ceremonies, not every public gRPC RPC.
- `EmbeddedMade::default()` is process-local and ephemeral.
  `EmbeddedMade::open(path)` persists the ceremony store and definition publications,
  but mounted definitions and transcripts remain host-configured boundaries.
- Callbacks execute on the caller's async runtime; MADE does not create
  or hide a runtime.
- Packaging to crates.io and a stable compatibility commitment wait for the
  repository's first public release.
