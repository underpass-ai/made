# Changelog

Release history is retained in the
[complete pre-rebuild changelog](docs/history/pre-rebuild-2026-09-18/CHANGELOG.md).
That snapshot preserves the original entries byte for byte; it includes old
names, plans and some documentation errors. It is history, not current setup
guidance. In particular, the plugin's data directory remains `underpass-made`
even though the new catalogue identity is `made`.

## Unreleased

- Make native Windows `made-setup` replace the bundled registration command
  atomically with the batch launcher, so clean Codex and Claude installs need
  no manual MCP edit and repeated setup keeps exactly one registration. (#185)
- Add a bounded, read-only pause/resume preflight and durable host-handoff
  evidence across embedded Rust, gRPC and both MCP backends. Host quiescence,
  engine drain, receipt recovery and absolute deadlines remain separate facts;
  the protocol neither infers liveness nor grants takeover or extends clocks.
  Exact handoff retries return their durable receipt while changed payloads
  conflict. (#188)
- Add a durable host delivery ledger, integrator bindings and a host activation
  port (no public surface yet) as the shared foundation for #192 and #204. (#220)

## 0.7.8 — 2026-09-19

- Refresh the configured Codex or Claude marketplace before `made-setup` reads
  the installed plugin manifest, preventing a stale cache from configuring an
  older MADE release after publication. (#186)

## 0.7.7 — 2026-09-19

- Preserve line separators in the native Windows private configuration file
  so the `cmd` launcher can parse all four setup values.

## 0.7.6 — 2026-09-19

- Make the native Windows bootstrap verify the same private configuration
  reader used by the `cmd` launcher before starting the embedded server.

## 0.7.5 — 2026-09-19

- Normalize Windows store paths consistently between setup and the `cmd`
  launcher when addressing private per-store configuration.

## 0.7.4 — 2026-09-19

- Fix the native Windows setup adapter's trusted-host variable name so it does
  not collide with PowerShell's reserved `$Host` variable.

## 0.7.3 — 2026-09-19

- Make the shared Git Bash configuration adapter defer private-file
  permission validation to native Windows ACL checks while retaining strict
  mode checks on Unix hosts.

## 0.7.2 — 2026-09-19

- Harden the plugin publication checks across macOS and native Windows so the
  private embedded setup configuration is verified with portable file and ACL
  checks.

## 0.7.1 — 2026-09-19

- Automate embedded plugin setup for Codex and Claude: `made-setup` now
  creates owner-only per-store configuration, generates and preserves the
  search cursor HMAC key, bootstraps authorization idempotently, and keeps the
  shared launcher fail-closed for malformed or unreadable configuration.

## 0.7.0 — 2026-09-19

- Add orthogonal ceremony pause, resume and irreversible cancellation plus
  sealed ceremony, state and step deadlines across Rust, gRPC and both MCP
  backends. Paused sessions drain already accepted fenced work; timed-out or
  terminal late results become idempotent observations without changing
  outputs or terminality. Historical event bytes remain stable, and snapshot
  envelope v2 prevents older writers from reopening a partial lifecycle tail.

- Stream resumable ceremony progress as sealed event records over gRPC, both
  MCP backends and `EmbeddedMade`. Requests bound replay and waiting, return an
  explicit resume cursor/end reason, catch up with external store writers and
  cancel producers on client drop. (#163)
- Add composable `stages[].pattern` authoring for broadcast/collect, managed
  group chat, maker-checker, bounded handoff and magentic task-ledger flows.
  Ship executable fragments, cross-edition design parity and Mermaid pattern
  regions with concurrent fork/join rendering. (#159)
- Add an explicit application opt-in for bounded parallel initial proposals,
  sharing one call pool across simultaneous councils. Record experiment 003
  with a controlled provider-capacity fixture; retain sequential defaults. (#161)
- Add durable child-ceremony spawning with sealed publications, projected
  inputs, deterministic lineage and bounded depth. Parent steps open every
  planned child before completing; `children_completed` joins accept only
  verified terminal records, and cursor recovery resumes crashes across
  embedded, gRPC and both MCP backends. (#160)
- Add typed `synthesize` and deterministic strict-majority `vote` aggregation
  to the first step after an all-siblings concurrent join. YAML, design,
  protobuf, both MCP backends and the embedded facade share one schema;
  omitted aggregation preserves existing definition bytes and digests. (#156)
- Adopt the full-color Cuatro voces artwork and English slogan in the README,
  documentation, plugin and skill cards, MCP identity, and Helm chart icon.

- Expose all council, agent and output-contract operations through
  `EmbeddedMade` and the embedded MCP backend. Local composition accepts
  injected registries, agent factories, deliberation stores, validators,
  scoring, execution and messaging ports; defaults remain socket-free,
  retain in-process messages and reject orchestration until an executor is
  configured. (#157)
- Run siblings in concurrent ceremony states with bounded automatic fan-out,
  durable claims before handlers, drained completions and early-join checks
  between batches. Document delegated host/subagent fan-out and prove distinct
  claims from two processes sharing SQLite. (#155)
- Project peak live ceremony claims per state visit/iteration and classified
  step failures from ordered sealed events in both editions. Preserve
  `NoValidProposal` as a typed `StepFailed` v4 result; old payloads retain
  their bytes and versions. Clarify that `num_agents` caps council size. (#158)

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
