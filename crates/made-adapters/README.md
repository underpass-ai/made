# made-adapters

Infrastructure adapters for
[MADE by Underpass](https://github.com/underpass-ai/made). Every port
declared in [`made-core`](https://crates.io/crates/made-core),
implemented against a concrete technology.

## No privileged provider

The engine is provider-agnostic, and this crate is where that claim is
kept honest: transports, buses and model vendors are peers, each behind
its own Cargo feature, none of them on by default beyond the deployable
service's own needs. Nothing in the domain or the use cases knows which
one you picked.

| Feature | Brings |
|---|---|
| `grpc`, `runtime-grpc` | tonic clients for the engine and the Underpass Runtime executor |
| `nats` | NATS messaging: trigger subjects in, outcome events out |
| `postgres` | SQLx-backed repositories |
| `redb` | Durable local ceremony store, journal, outbox and publications |
| `kmp` | Context bundles from a Kernel Memory Plane producer |
| `otel` | W3C trace context extraction on inbound gRPC |
| `agent-vllm`, `agent-anthropic`, `agent-openai` | LLM agent adapters, one per vendor |

Always available, with no feature at all: system clock, environment
configuration, in-memory registries and repositories, Prometheus metrics,
deterministic no-op agent, executor and messaging.

## The no-op adapters are not filler

`NoopAgent`, `NoopExecutor` and `NoopCeremonyStepHandler` are how a
ceremony's protocol gets exercised without pretending work happened. A
terminal step reached through a no-op handler proves the state machine,
not that an agent, a tool or a human did anything — the engine's
documentation is explicit about that boundary, and so is this crate.

## License

Apache-2.0.
