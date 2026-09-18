# Changelog

Release history is retained in the
[complete pre-rebuild changelog](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md).
That snapshot preserves the original entries byte for byte; it includes old
names, plans and some documentation errors. It is history, not current setup
guidance. In particular, the plugin's data directory remains `underpass-made`
even though the new catalogue identity is `made`.

## Unreleased

- Rebuild the documentation around local setup, host execution, public
  contracts and service operations; preserve the previous tree and hashes.
- Add the MADE block wordmark and matching SVG to active entrypoints.
- Remove the repository `.kmp` memory export; this does not remove live stores.
- Rename the source marketplace catalogue to `made`, co-located in this
  repository. The stable `marketplace` branch advances only after a new
  immutable release and its required assets publish.
- Warn during definition analysis when global `all_steps_completed` can wait
  on downstream work. Runtime guard semantics are unchanged.
- Require the accepted claim's opaque fence on completion. Claim responses keep
  the accepted snapshot/audit identity; delegated and application-owned work
  cannot rebind old results to a replacement claim.
- Add durable state visits to execution coordinates and fact identities. New
  transitions seal destination reset sets; old event bytes and fold behavior
  remain compatible.
- Validate bounded unique report ids, reconsideration conditions and scoped
  recipients at command boundaries; empty recipient lists address the table.
- Fix counted-join serialization so `steps_completed:n` definitions can be
  published, reopened and executed across the shared surfaces.
- Display the co-located `made` catalogue as **MADE**.

These changes are integrated in this source tree and remain unreleased.
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
