# Changelog

Release history is retained in the
[complete pre-rebuild changelog](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md).
That snapshot preserves the original entries byte for byte; it includes old
names, plans and some documentation errors. It is history, not current setup
guidance. In particular, the plugin's data directory remains `underpass-made`
even though the new catalogue identity is `made`.

## Unreleased

- Add typed `synthesize` and deterministic strict-majority `vote` aggregation
  to the first step after an all-siblings concurrent join. YAML, design,
  protobuf, both MCP backends and the embedded facade share one schema;
  omitted aggregation preserves existing definition bytes and digests. (#156)
- Wait for a private NATS inbox round trip before E2E scenarios trigger events
  from another connection. The harness no longer treats a local socket flush
  as server acknowledgement of its subscription. (#153)

## 0.6.0 — 2026-09-18

The release installation paths require public 0.6.0 assets and the stable
`marketplace` branch at that release. Before publication, use the
[source candidate route](docs/embedded/README.md#test-a-source-candidate).

### Breaking client changes

- Require the accepted claim's opaque `claim_fence` on completion across
  direct RPC, both MCP backends and Rust. Missing identities are refused;
  Rust callers must retain the new claim output and pass its fence to
  completion. Claim responses keep
  the accepted snapshot/audit identity; delegated and application-owned work
  cannot rebind old results to a replacement claim. (#127)
- Validate unique report ids, reconsideration conditions and scoped recipients
  with a shared 100-item cap at command boundaries. Report ids and conditions
  must be nonempty; empty recipient lists address the table. Rust callers use
  the validated list types and fallible constructors. Historical event payloads
  remain readable. (#100)

### Execution and diagnostics

- Add durable state visits to execution coordinates and fact identities. New
  transitions seal destination reset sets; old event bytes and fold behavior
  remain compatible. State repetition retains its visit; a transition opens a
  new visit. New event schemas require a compatible reader. (#129)
- Warn during definition analysis when global `all_steps_completed` can wait
  on downstream work. Runtime guard semantics are unchanged. (#143)
- Fix counted-join serialization so `steps_completed:n` definitions can be
  published, reopened and executed across the shared surfaces. Other guard
  representations and existing definition digests are unchanged. (#150)

### Installation and documentation

- Prepare the co-located marketplace identity `made`, displayed as **MADE**,
  independently of KMP. The stable branch advances only after the new immutable
  release and its required assets publish. (#39)
- Rebuild documentation from a new structure around local setup, host execution,
  public contracts and service operations; preserve previous prose and hashes.
- Add the MADE block wordmark and matching SVG to active entrypoints.
- Remove the repository `.kmp` memory export; this does not remove live stores.

[Migration guidance](docs/migrations/README.md) covers required client changes
and historical-data behavior. The
[hardening prose snapshot](docs/history/hardening-integration-2026-09-18/INDEX.md)
preserves the incoming ADRs and implementation notes without changing the
original documentation archive.

## 0.5.0 — 2026-09-18

Event-stream foundation, durable SQLite session state/memory, ceremony
history and reporting, cross-surface parity, and bounded concurrency,
repetition, transitions and context primitives. Hosts still schedule and
perform external work; full orchestration patterns remain future work.

[Complete release record](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md#050---2026-09-18)
· [Release](https://github.com/underpass-ai/made/releases/tag/v0.5.0).

## Earlier release records

The archived changelog retains the complete entries and validation detail
for [0.3.0](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md#030---2026-09-03),
[0.2.0](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md#020---2026-09-02),
and 0.1.0–0.1.6. The v0.2.0 binary provides the one-time Redb-to-SQLite
conversion needed by old stores. Use the [migration guide](docs/migrations/README.md)
before changing stored data or clients.
