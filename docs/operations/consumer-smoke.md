# Verify the service as a consumer

`made-consumer-smoke` drives public service contracts. Start the configured
service, then:

```bash
cargo run -p made-consumer-smoke --locked -- \
  --endpoint http://127.0.0.1:50055 --chain all
```

For bus checks, supply `--nats-url nats://127.0.0.1:4222`. Without a NATS
endpoint the bus checks are skipped; do not report that as verified messaging.
Inspect the binary's `--help` for TLS and chain selectors that match this build.

The smoke creates its own inputs and exercises the reachable boundary. It
may mutate service state and invoke configured providers; use a dedicated
smoke environment. A successful stub/no-op chain establishes integration,
not real-provider quality or external execution.

Implementation: [consumer-smoke](../../crates/made-consumer-smoke/src/main.rs).
For the full deployment path, see [Kubernetes](deploy-kubernetes.md).
