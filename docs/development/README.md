# Develop MADE

Use the pinned [Rust toolchain](../../rust-toolchain.toml) and committed
`Cargo.lock`. `Cargo.toml` declares MSRV 1.97; the checkout pins 1.97.1 with
Clippy and rustfmt. Protobuf generation needs `protoc`; contract validation
needs `buf` and the AsyncAPI CLI. Container and chart checks additionally need
the tools their scripts name.

## Run the relevant gates

```bash
bash scripts/ci/dev-loop.sh
make check
```

The development loop is the quick repository feedback path. `make check`
runs contract validation, formatting, strict Clippy, workspace tests and bench
compilation with the configured provider features. The aggregate
`scripts/ci/quality-gate.sh` and
[quality workflow](../../.github/workflows/quality-gate.yml) define CI behavior.
Use the planner for changed-file routing; compiled documentation and
machine-readable ledgers can require code gates even when they live in docs.

For the embedded/plugin boundary:

```bash
cargo test -p made-embedded --locked
cargo test -p made-mcp --all-targets --no-default-features --features embedded --locked
bash scripts/ci/embedded-dependency-boundary.sh
bash scripts/ci/made-plugin-smoke.sh
bash scripts/ci/made-plugin-bootstrap.sh
```

The plugin smoke drives the launcher and reopens persisted state. Bootstrap
checks release URL selection, checksum/setup and startup with simulated
downloads and Cargo absent. Their scripts define fixtures,
network requirements and cleanup. Use the matching release/source identity
when interpreting their results.

For deployment changes:

```bash
make helm-lint
make integration
make e2e-compose
make e2e-kubernetes
```

Integration uses containers for NATS and Postgres. Kubernetes E2E requires a
reachable cluster, kubectl and Helm; it does not create the cluster. Use a
dedicated environment. Real-provider E2E targets require their endpoint/model
configuration and are separate from provider-compatible stub tests.

## What the gates protect

The contract gate validates protobuf/AsyncAPI and compares packaged proto and
ceremony fragment copies. Parity tests compare the four ceremony surfaces
with [parity.tsv](../architecture/parity.tsv), then compare complete sessions
across both MCP backends. The support table is an `include_str!` test input.
Architecture conformance and coverage floors have separate checked ledgers.

For persistence changes, test historical event bytes/hashes, full fold,
snapshot tails and reopen. For concurrency changes, use real competing
clients and assert that a refused command appends nothing. A mocked return
value is not sufficient evidence of transactional behavior.

Keep scratch under the repository's ignored `tmp/` and clean it after the
run. `target/` is a build cache, not review evidence. Retain compact check
results and reproducible commands with the change; never commit credentials
or database dumps.

## Benchmarks and changes

Benchmark code lives in `made-core/benches` and `made-app/benches`.
[Experiments](../experiments/README.md) retain scripts and historical raw
results. Those old measurements are not claims about current performance.
Record environment, method, results and limitations for a new measurement.

Follow [Contributing](../../CONTRIBUTING.md), keep public examples aligned
with the actual schema, and review [migrations](../migrations/README.md) when
changing execution identity or stored events. Releases follow the separate
[release procedure](release.md).
