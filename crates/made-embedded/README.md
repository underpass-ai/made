# made-embedded

The in-process distribution of the
[MADE by Underpass](https://github.com/underpass-ai/made) ceremony
engine.

`EmbeddedMade` runs the same use cases as the deployable service without
opening a socket or reading process-wide configuration. No gRPC, no NATS,
no database required — and none forbidden either: hosts inject whatever
adapters they want behind the ports.

```rust
use made_embedded::EmbeddedMade;

// Everything in memory, dies with the process.
let engine = EmbeddedMade::default();

// Or durable on the canonical SQLite WAL store: state survives a restart.
let engine = EmbeddedMade::open("ceremonies.sqlite3")?;
```

## What durable does and does not mean

With SQLite, ceremony snapshots, the unit of work, the audit journal, the
outbox and definition publications are persisted. Mounted definitions and
transcripts stay in memory unless the host replaces those ports.

That difference has a consequence worth knowing before you rely on it: an
instance started from a **published** definition rehydrates after a
restart, while one started from supplied YAML keeps its snapshot but
cannot reload the definition it ran. The engine reports the second kind
as unrehydratable rather than hiding it or failing the whole listing.

## Host-owned execution

`CallbackCeremonyStepHandler` and `CallbackCeremonyEvidenceSource` let the
host perform the real work — a search, a model call, a deploy — while the
engine coordinates the ceremony around it. The claim/complete protocol
means a step is only recorded as done when the host says it produced
something, with its output attached.

## License

Apache-2.0.
