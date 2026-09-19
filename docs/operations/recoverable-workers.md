# Recoverable worker host

`CeremonyWorkerHost` composes discovery, application claims and a bounded
`CeremonyWorkerDriver`. The host chooses the connector, repository root,
concurrency and lease TTL. It explicitly runs claim and recovery pages; neither
the MCP binary nor the service starts an external worker daemon implicitly.
The current driver checks deadlines before admission and drains admitted work
on cooperative stop. It does not renew leases: choose a TTL appropriate for
the bounded connector timeout, and use recovery after lease loss.

An operation ID identifies a ceremony, step, state visit and iterations. A
technical attempt, worker or replacement claim does not change that identity.
The sealed intent preserves the exact request bytes before external execution.
A receipt records the observed result, producing fence, connector capability,
source kind and linked artifacts. Artifact provenance must match that receipt
and the artifact store must contain the exact committed reference.

## External script connector

`RepositoryScriptExecutionConnector` accepts an executable inside the explicitly
configured repository root. It passes the operation ID, request digest, producing
fence, request file and result file as separate arguments. The executable must
write the documented result envelope durably before returning. It is trusted
host code and runs with the host process's permissions.

The connector seals request bytes and creates an exclusive durable admission
marker before spawning. A second process cannot execute that operation again.
If the marker exists without a readable result, recovery reports reconciliation
required. This conservative state covers a crash before the effect as well as
one after the effect but before recording its result. An operator must resolve
it; the connector never clears the marker to repeat the mutation blindly.
A durable result can instead be queried and adopted by a valid replacement
claim. This is operation lookup, not an exactly-once guarantee for arbitrary
external systems.

`CeremonyStepHandlerConnector` adapts an existing handler. Its declared recovery
capability determines whether the engine may recover automatically. Fixtures
and no-op sources remain explicit in receipts and do not prove provider quality.

## Verification and soak

`repository_script_execution_recovery` tests process contention and process
loss after an external effect. `durable_fixture_execution_recovery` tests
receipt recovery, adoption and artifact mismatch rejection. Public transport
parity exercises lookup, paging, completion and adoption with linked artifacts.

The repository includes a reproducible host-loop example:

```bash
mkdir -p tmp/worker-soak
cargo run -p made-tests-integration --example worker_soak -- \
  --store "$PWD/tmp/worker-soak/store.sqlite3" --repository "$PWD" \
  --duration-seconds 3600 --workers 4
```

Its external Python script writes a real local effect and a durable result.
The example re-reads journals, completion state, receipts and effect bytes,
pages the entire remaining recovery set and rejects duplicate invocations.
Running it again against the same store checks persistence across host runs.
Its JSON output alone does not measure CPU, RSS or public transport latency;
those require an external measurement harness. This synthetic write workload
does not substitute for a software-change scenario or an actual model smoke.
