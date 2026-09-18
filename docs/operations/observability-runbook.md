# Inspect a running engine

Begin with the backend and exact binary version. MCP discovery describes its
available tools; service status and metrics describe the running composition.
No-op adapters can be healthy, so availability and real work need different
evidence.

The deployed service exposes `/healthz`, `/readyz` and `/metrics` on its HTTP
port (8080 by default). The embedded composition owns a local metrics registry
but opens no HTTP endpoint automatically. Inject a recorder/exporter when the
host needs one.

Sealed ceremony events feed metrics, structured logs, tracing and other
subscribers. Inspect the relevant ceremony id and execution coordinates
before correlating a worker error with a state change. Events accepted into
the stream are different from rejected command attempts.

For diagnosis:

1. Check process readiness and backend/store configuration.
2. List and read the affected ceremony, its published definition identity,
   pending steps, human guards and interventions.
3. Read event pages from the relevant stream position and verify the journal.
4. Compare a suspected completion with its accepted claim identity, visit,
   iteration and attempt. A stale fence refusal must leave the stream unchanged.
5. Inspect external handler/tool evidence separately.

Use the global feed and named consumer cursor for publication lag; use the
per-ceremony stream version for reconstruction. They are not interchangeable.
A host using an event transport should invoke its recovery path at startup so
pending publication can proceed without waiting for a new append.

`make run-otel` builds the service with the optional telemetry feature.
Exporter configuration and available environment settings live in the service
and adapter configuration source; never place credentials in logged descriptor
attributes. If exporting traces, keep the collector reachable under the chosen
network policy and avoid treating a missing trace as proof that no event exists.
