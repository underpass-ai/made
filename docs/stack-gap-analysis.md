# Stack Gap Analysis

Snapshot date: 2026-04-25

This document records the current gaps between the Choreographer and the
role it aims to play in the Underpass platform alongside:

- [underpass-runtime](https://github.com/underpass-ai/underpass-runtime)
- [rehydration-kernel](https://github.com/underpass-ai/rehydration-kernel)

The goal is not to market readiness. The goal is to state what is wired,
what is not, and what must change before this service can honestly be
described as a production peer in that stack.

## Scope

This analysis is based on:

- the local `underpass-ai/underpass-choreographer` checkout
- repository docs, chart, contracts, and CI scripts in this repo
- the public README surfaces of `underpass-runtime` and
  `rehydration-kernel`, fetched via `gh`

## What Is Healthy Today

- The Rust workspace shape is solid: `core` -> `app` -> `adapters` -> binary.
- Unit and in-process tests pass locally on the full provider feature
  matrix.
- `cargo clippy` passes locally on the full provider feature matrix.
- Criterion benches compile.
- Helm lint passes.
- The local fast-path now matches CI materially better:
  `just check` includes the contract gate, `quality-gate.sh` uses the
  same provider feature matrix as CI, and benches are compile-gated
  locally too.
- The container-backed integration scripts now fail fast with targeted
  Docker/Podman socket guidance instead of surfacing an opaque
  `SocketNotFoundError` from inside Rust.
- The contract surface is honest about `task_description_template`:
  it is currently literal producer-supplied text, not a rendered
  template language.
- The repo is disciplined about honest caveats in several places,
  especially around provider wiring and streaming limitations.

Local commands validated on this workstation:

```bash
cargo test --workspace --locked
cargo test --workspace --locked \
  --features choreo-adapters/agent-anthropic \
  --features choreo-adapters/agent-openai \
  --features choreo-adapters/agent-vllm
cargo clippy --workspace --all-targets --locked \
  --features choreo-adapters/agent-anthropic \
  --features choreo-adapters/agent-openai \
  --features choreo-adapters/agent-vllm \
  -- -D warnings
bash scripts/ci/bench-compile.sh
bash scripts/ci/helm-lint.sh
```

Local limitations of this snapshot:

- `bash scripts/ci/contract-gate.sh` could not complete here because the
  AsyncAPI CLI was not installed on this workstation.
- Container-backed integration suites were not validated end-to-end here
  because `testcontainers` could not reach a working Docker-compatible
  socket from the current environment.

## High-Severity Gaps

### 1. Stack integration is still conceptual, not wired

The Choreographer README positions the service as the coordination plane
between Kernel and Runtime. The architecture supports that direction, but
the actual production wiring is not present yet.

Current blockers:

- The binary composes [`NoopExecutor`](../crates/choreo-adapters/src/noop/executor.rs)
  unconditionally.
- The binary composes [`NoopAgentFactory`](../crates/choreo-adapters/src/noop/agent_factory.rs)
  unconditionally.
- The only wired `AgentFactoryPort` accepts `kind == "noop"` and rejects
  every real provider kind.
- There is no Runtime gRPC executor adapter.
- There is no Kernel adapter or context-rehydration path.
- There is no end-to-end stack flow that proves:
  trigger -> rehydrate context -> deliberate -> execute via Runtime.

Practical consequence: today this service is a well-structured
deliberation service, not yet a fully integrated Underpass coordination
plane.

## Medium-Severity Gaps

### 2. Event correlation is partially improved

As of 2026-04-26, `EventEnvelope` carries both `correlation_id` and
`causation_id`, and `TaskMetadata` preserves causal ids from inbound
triggers through deliberation and orchestration lifecycle events.
`TaskDispatchedEvent` now records the source trigger id when the task
was built from an inbound trigger.

Relevant files:

- [`crates/choreo-core/src/events/envelope.rs`](../crates/choreo-core/src/events/envelope.rs)
- [`crates/choreo-app/src/usecases/deliberate.rs`](../crates/choreo-app/src/usecases/deliberate.rs)
- [`crates/choreo-app/src/usecases/orchestrate.rs`](../crates/choreo-app/src/usecases/orchestrate.rs)
- [`crates/choreo-adapters/src/grpc/mappers/event.rs`](../crates/choreo-adapters/src/grpc/mappers/event.rs)

Remaining gap: stack E2E coverage should assert causal metadata on the
event bus.

### 3. TLS appears in the chart surface but not in the server wiring

The Helm chart exposes `tls.mode` and `existingSecret`, but the gRPC
server currently starts with plain `tonic::transport::Server::builder()`
and no TLS configuration path was found in the binary.

Relevant files:

- [`charts/choreographer/values.yaml`](../charts/choreographer/values.yaml)
- [`crates/choreo/src/runtime.rs`](../crates/choreo/src/runtime.rs)

Practical consequence: the repo exposes a deployment surface that is not
backed by the binary yet. In a stack where Runtime and Kernel already
lean on TLS/mTLS, this is a real operational gap.

### 4. "JetStream" is named, but durable JetStream behavior is not wired

Docs and AsyncAPI describe the broker as NATS JetStream, but the current
adapter uses plain publish/subscribe semantics. No stream, consumer,
durable subscription, replay, or explicit ack model is wired in the
service.

Relevant files:

- [`specs/asyncapi/choreographer.asyncapi.yaml`](../specs/asyncapi/choreographer.asyncapi.yaml)
- [`crates/choreo-adapters/src/lib.rs`](../crates/choreo-adapters/src/lib.rs)
- [`crates/choreo-adapters/src/nats/messaging.rs`](../crates/choreo-adapters/src/nats/messaging.rs)
- [`crates/choreo-adapters/src/nats/subscriber.rs`](../crates/choreo-adapters/src/nats/subscriber.rs)

Practical consequence: either the wording should be narrowed to plain
NATS, or the adapter must grow into actual JetStream semantics.

## Recommended Hardening Plan

### Phase 2: Real stack integration

Wire the service to the other two Underpass planes.

1. Add a Runtime executor adapter over gRPC.
2. Add TLS/mTLS-capable client wiring for that Runtime adapter.
3. Add a dispatching `AgentFactoryPort` in the binary so provider-backed
   agents can actually be materialized from descriptors.
4. Define the Kernel integration boundary:
   whether context is pulled synchronously before deliberation, attached
   into task attributes upstream, or both.
5. Add one reproducible stack test proving:
   trigger -> optional context rehydration -> deliberate -> Runtime
   execution -> outbound events.

### Phase 3: Transport and operations hardening

Bring the deployment surface up to the standard set by Runtime and
Kernel.

1. Either implement server TLS/mTLS in the binary and chart, or remove
   the TLS values until they are real.
2. Decide whether the service is plain NATS or JetStream. Then align
   docs, spec, and code to one answer.
3. Propagate `correlation_id` and `trigger_event_id` through the full
   event lifecycle.
4. Add a container-backed test that exercises whichever NATS mode is
   declared as supported.
5. Add one release-gate stack smoke test that runs against deployed
   Runtime and Kernel surfaces, not only the Choreographer in isolation.

## Honest Current Position

As of this snapshot, the Choreographer is:

- a healthy Rust service
- a credible coordination-plane skeleton
- a real gRPC + NATS + persistence application
- not yet an end-to-end integrated peer of Runtime and Kernel

The shortest honest summary is:

> The Choreographer is structurally ready for Underpass stack
> integration, but the adapters and enforcement needed to make that
> integration real are still incomplete.
