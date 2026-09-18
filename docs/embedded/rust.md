# Embed MADE in Rust

`made-embedded` composes the ceremony use cases in your process. The caller
owns the async runtime and all external work. There is no hidden server.
The narrower `made-api::CeremonyEngineApi` trait is a transport-neutral
consumer boundary; the `EmbeddedMade` facade exposes additional typed host
operations.

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

## Compatibility

The workspace release version and a ceremony's definition version are
separate identities. Rust integration is pre-1.0; review the
[migration guide](../migrations/README.md) before upgrading. In particular,
the unreleased completion API requires the fence returned by the accepted
claim. The runtime keeps it automatically for server-owned `run_step`.

The embedded dependency boundary excludes gRPC, NATS and Postgres clients.
`made-adapters` is used with default features disabled; adding a transport
adapter is an explicit host composition choice.
