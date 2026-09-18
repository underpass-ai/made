# Contribute to MADE

Start with the [architecture](docs/architecture/README.md) and
[development guide](docs/development/README.md). Explain the concrete problem,
the resulting behavior and the evidence in each PR.

Keep the engine's vocabulary independent of consuming products and providers.
Domain invariants belong in typed values and aggregates; use cases compose
ports; adapters own transport, serialization and external IO. Put a focused
production type in each file and preserve the dependency direction.

For a public change, update its schema, adapters, parity/support claims,
examples and migration notes together. Stored-event changes need historical
byte/hash compatibility and replay tests. Concurrency fixes need evidence
that competing claims and refused results preserve durable state.

Run checks appropriate to the boundary changed and report exact outcomes.
Documentation alone does not need a full rebuild, but compiled fixtures and
support tables do. Never present a stub, no-op or mocked call as proof of
real external execution. Benchmark claims need reproducible measurements.

Use [the PR template](.github/pull_request_template.md), keep the changelog
traceable and follow [release procedure](docs/development/release.md) for
publication. Report vulnerabilities privately via [Security](SECURITY.md).
