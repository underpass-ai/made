# Embed MADE in Rust

`made-embedded` composes the ceremony use cases in your process. The caller
owns the async runtime and all external work. There is no hidden server.
The narrower `made-api::CeremonyEngineApi` trait is a transport-neutral
consumer boundary; the `EmbeddedMade` facade exposes additional typed host
operations.

`EmbeddedMade::default()`, `EmbeddedMade::open(path)` and a builder completed
without authorization deliberately leave the embedding host as the trust
boundary. They are the unprotected in-process composition used by the examples
below; actor fields remain provenance rather than credentials. A protected
direct facade installs a `TrustedHostAuthorizationGate` on the builder and an
authorization policy store through `with_authorization_policy`. Every protected
public read or mutation then requires an active `AuthorizationOperationScope`
admitted for the exact action, authoritative scope and target. Missing or
mismatched context fails before the facade reads or mutates protected state.
See the checked
[protected facade example](../../crates/made-embedded/tests/protected_facade_authorization.rs)
for approval by one principal and execution by another.

## Publish, start and reopen

Add matching release versions of `made-api` and `made-embedded`, plus Tokio.
This example follows the checked
[SQLite public API test](../../crates/made-embedded/tests/sqlite_store_api.rs):

```rust,no_run
use std::collections::BTreeMap;
use made_api::{CeremonyEngineApi, StartCeremonyRequest};
use made_embedded::EmbeddedMade;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = r#"
version: "1.0"
name: review
states:
  - id: OPEN
    initial: true
  - id: CLOSED
    terminal: true
transitions:
  - from: OPEN
    to: CLOSED
    trigger: close
    guards: []
steps: []
guards: {}
roles: []
"#;
    let engine = EmbeddedMade::open("ceremonies.sqlite3")?;
    let analysis = engine.analyze_definition(yaml).await?;
    assert!(analysis.publishable);
    CeremonyEngineApi::publish_definition(&engine, yaml).await?;
    engine.start_ceremony(StartCeremonyRequest {
        ceremony_id: "review-1".into(),
        definition_name: analysis.definition_name,
        definition_version: analysis.definition_version,
        context: BTreeMap::new(),
        actor_id: "my-host".into(),
        actor_kind: "service".into(),
    }).await?;
    drop(engine);

    let reopened = EmbeddedMade::open("ceremonies.sqlite3")?;
    let session = reopened.ceremony("review-1").await?;
    assert_eq!(session.definition_name, "review");
    Ok(())
}
```

This demonstrates durable publication and session identity, not agent work.
See [authoring](../authoring/README.md) to add steps and
[runtime](../runtime/README.md) to execute them.

## Supply real work

`EmbeddedMade::default()` uses in-memory ceremony adapters, `ForgetfulMemory`
and a no-op step handler. `EmbeddedMade::open(path)` supplies SQLite-backed
ceremony state, publication and memory, but does not supply an agent runtime.

Use the builder to inject `CeremonyStepHandlerPort` or its callback adapter:

```rust
use made_core::value_objects::{StepOutput, StepResult};
use made_embedded::EmbeddedMade;

let engine = EmbeddedMade::builder()
    .with_step_handler_callback(|request| async move {
        let _handler = request.handler_kind();
        // Invoke the host's authorized agent/tool here and capture its output.
        StepResult::completed(StepOutput::empty())
    })
    .build();
```

The callback above is a skeleton; replace the empty result with observed work.
`CallbackCeremonyEvidenceSource` is the corresponding read-only evidence
adapter. For durable custom composition, retain an
`Arc<SqliteCeremonyStore>` and inject it using
`with_ceremony_store_and_memory`, `with_definition_publications` and
`with_event_cursor`; then inject the handler. See the
[builder](../../crates/made-embedded/src/embedded_made_builder.rs) for exact
signatures. The typed combined store-and-memory method prevents accidentally
selecting different writer and reader stores.

Keep adapter handles for adapter-specific administration. Optional event
transport and metrics constructors wire those ports explicitly. A local
metrics registry opens no exporter endpoint by itself.

## Compose councils locally

The facade also exposes council, agent and output-contract operations. The
generic builder is process-local and reads no provider configuration. This
example registers the deterministic built-in `noop` agent and creates a
council without opening a socket. The same source is compiled as
[`local_council.rs`](../../crates/made-embedded/examples/local_council.rs):

```rust,no_run
use made_app::usecases::CreateCouncilInput;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, Specialty,
};
use made_embedded::EmbeddedMade;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = EmbeddedMade::builder().build();
    let agent_id = engine.register_agent(AgentDescriptor {
        id: AgentId::new("reviewer-1")?,
        specialty: Specialty::new("review")?,
        kind: AgentKind::new("noop")?,
        attributes: Attributes::empty(),
    }).await?;
    let council = engine.create_council(CreateCouncilInput {
        council_id: CouncilId::new("review-council")?,
        specialty: Specialty::new("review")?,
        agents: vec![agent_id],
    }).await?;
    assert_eq!(council.size(), 1);
    Ok(())
}
```

The default council, agent, contract and deliberation registries live only for
the lifetime of this composition. `EmbeddedMade::open(path)` makes ceremony
streams, definitions and session memory durable; it does not make those
council registries durable. Inject the corresponding ports on
`EmbeddedMade::builder()` when the host needs a different lifecycle.

The default messaging adapter is an in-process recorder. It preserves complete
typed event payloads in order and does not publish them to a broker. Inject a
`MessagingPort` to deliver events elsewhere. The default executor rejects
`orchestrate` with `embedded executor is not configured`; inject an
`ExecutorPort` before asking MADE to run external work. Council deliberation
and `run_council_decision` use registered agents and do not turn that rejected
executor into a successful fake runtime.

Provider adapters are compile-time features: `agent-vllm`, `agent-openai` and
`agent-anthropic`. Building a feature only makes that adapter available.
`EmbeddedMade::builder()` still performs no environment read and no network
call; inject an `AgentFactoryPort` explicitly. The convenience `open*` paths
compose enabled providers from their documented environment configuration,
but make no provider request until a registered agent is used. The repository
tests provider composition with deterministic doubles and do not establish
credentials, quota, endpoint policy or real-model behavior.

## Compatibility

The workspace release version and a ceremony's definition version are
separate identities. Rust integration is pre-1.0; review the
[migration guide](../migrations/README.md) before upgrading. In particular,
0.6.0 requires the fence returned by the accepted claim on
every completion; v0.5.0 predates that contract. The runtime keeps it
automatically for server-owned `run_step`.

The embedded dependency boundary excludes gRPC, NATS and Postgres clients.
`made-adapters` is used with default features disabled; adding a transport
adapter is an explicit host composition choice.

## Follow ceremony progress

`EmbeddedMade::stream_ceremony` returns sealed `AuditRecord` frames followed
by one typed `End` frame. Keep the end frame's `resume_after_sequence` and pass
it back as the next input cursor. A zero wait performs replay only:

```rust,no_run
use futures::StreamExt;
use made_app::usecases::{CeremonyProgressFrame, StreamCeremonyInput};
use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, CeremonyProgressWait, StreamVersion,
};

# async fn follow(engine: &made_embedded::EmbeddedMade) -> Result<(), Box<dyn std::error::Error>> {
let mut progress = engine.stream_ceremony(StreamCeremonyInput::new(
    CeremonyId::new("review-1")?,
    StreamVersion::EMPTY,
    CeremonyEventPageLimit::DEFAULT,
    CeremonyProgressWait::IMMEDIATE,
)).await?;
while let Some(frame) = progress.next().await {
    match frame? {
        CeremonyProgressFrame::Record(record) => println!("{}", record.sequence().value()),
        CeremonyProgressFrame::End(end) => {
            println!("resume after {}", end.resume_after_sequence().value());
            break;
        }
    }
}
# Ok(())
# }
```

The facade uses a bounded channel and awaited backpressure. Dropping the stream
cancels its producer. Its 250 ms cross-process catch-up interval is a polling
setting, not a delivery deadline.
