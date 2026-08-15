# Changelog

All notable changes to MADE are tracked here.

`v0.1.0` is the first tagged release. Keep new entries under
`Unreleased` until the release process in `docs/release.md` bumps the
version and creates the next immutable tag; released sections are not
edited afterwards.

The format follows the spirit of Keep a Changelog, with categories kept
short and factual. Do not add claims here unless the behavior is
implemented and covered by a committed gate, smoke test, or documented
operator command.

## Unreleased

_Nothing yet._

## 0.1.0 - 2026-08-15

First tagged release. Everything below shipped under the previous name,
`underpass-choreographer`, except where an entry says otherwise; the
rename to MADE is itself the first entry under Changed.

### Added

- **Durable embedded MCP backend.** `MADE_MCP_BACKEND=embedded` now opens the
  redb state file named by `MADE_MCP_REDB_PATH`, so ceremonies started through
  the stdio adapter survive the MCP process. The variable is mandatory —
  without it the binary exits with code 2 instead of inventing a location or
  running on memory that dies with the process — and the Codex plugin launcher
  supplies `${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb`
  by default. `MadeMcpServer::embedded_redb(path)` exposes the same
  composition to Rust hosts; `MadeMcpServer::embedded()` remains the in-memory
  one. The plugin smoke now proves the claim across processes: one launcher
  publishes and starts a ceremony, a second reopens the same file and reads it
  back with its state, next step and bound definition digest intact.
- An embedded ceremony execution runbook covering the delegated-host loop
  (publish → start-published → claim → perform → complete → transition),
  restart recovery, the durability boundary and its failure modes.

- A capability-verification runbook that separates implemented engine features,
  the active MCP executable catalog, execution ownership and configured
  durability. Root, plugin and embedded documentation now direct agents to
  verify each claim independently.
- MCP self-description through `made_discover_capabilities`, derived from
  the active backend-filtered tool catalog, plus `made_get_help` guidance
  for users and agents. Discovery marks artifact generators and their
  persistence boundary; agent help covers preconditions, authority,
  delegated-host sequencing and errors. The Codex plugin smoke now proves the
  report generator is advertised and generates Markdown.
- Embedded MCP adapters for host-owned step execution through
  `made_claim_ceremony_step` and `made_complete_ceremony_step`, reusing the
  existing start/complete application use cases. Guidance distinguishes a
  configured real server handler, the bundled no-op default, and delegated
  host work completed only with observable output/evidence.
- Embedded MCP ceremony reports through `made_generate_ceremony_report`.
  Reports project one or more persisted snapshots, resolved definitions and
  ordered audit journals into deterministic, injection-safe Markdown, return
  structured completion and definition-binding metadata, and perform no write.
- Host-owned MCP server identity for embedded compositions. The default remains
  `underpass-made-mcp`, while an embedding application can advertise its own
  name and version during the MCP initialization handshake.
- Embedded ceremony instance discovery through
  `made_list_ceremony_instances`. Hosts can enumerate recoverable meetings
  after losing conversation context, refresh the selected instance, and resume
  without approving guards, closing interventions, or replaying operational
  work. Process-restart durability remains a responsibility of the repositories
  configured by the embedded host.
- Embedded MCP backend and repo-local Codex plugin bundle. The isolated
  `made-mcp` build completes the MCP stdio handshake and runs ceremonies
  without gRPC/protobuf. Its current backend-filtered catalog also includes
  discovery, authoring, incremental controls, delegated-host execution,
  interventions, evidence and reports; callers must use
  `made_discover_capabilities` for the exact installed surface. Direct,
  process, dependency-boundary and plugin-launcher smoke tests cover the
  bundle.
- `made-embedded`, an in-process distribution of the ceremony engine with
  local defaults, injectable domain ports, an async host-callback step adapter,
  incremental human-active operations, and no required gRPC, NATS or Postgres
  dependency. It uses the same domain and application use cases as the
  deployable binary and carries the same workspace release version.
- Ceremony step `output_contract` (#118): declarative deterministic policy
  gates in ceremony YAML — `contract_id`, `format`, `required_fields`,
  `allowed_values`, optional embedded `json_schema`; unknown keys are
  rejected. Proposals that fail the gate fail the deliberation as
  `NoValidProposal{contract_id}`.
- Evidence grounding rule (#119): optional `evidence` block on an
  `OutputContract` — each claim object must cite `evidence_refs` that exist
  in an allowed set, resolvable per-run from the `RunCeremony` context
  (`allowed_refs_from_context`) or a static list; enforced by the
  `claims-evidence-grounded` validator.
- Helm chart NOTES banner (#115): `helm install` prints a loud warning when
  trace export (OTLP endpoint) is not configured, so deliberations are never
  silently run unobserved.
- Operations runbooks: observability wiring (traces, metrics, logs) (#116)
  and ceremony authoring (schema, rounds, sizing, verification) (#117).
- Observability — Prometheus metrics: the binary exposes the operational
  metric families at `GET /metrics` (HTTP port `8080`) through a
  `MetricsRecorderPort` (core) and a `PrometheusMetricsRecorder` adapter
  (explicit registry, no global recorder), alongside the original
  `Statistics`-backed counters. Covers deliberation quality (duration,
  winner-score distribution, terminal outcome), the LLM judge (latency,
  score, errors by kind, discrimination, tokens, scoring mode), the
  proposing providers (request latency, errors, in-flight gauge, tokens),
  the ceremony engine (outcomes, durations, per-step status, blocked
  transitions), NATS publish (latency + errors), and the Postgres pool.
  Wired through a `with_metrics` opt-in so only the composition root
  installs the live recorder. Covered by unit tests.
- Observability — distributed tracing: with the `otel` feature and an OTLP
  endpoint configured, a deliberation is exported as one trace whose span
  events carry the debate itself — proposals, peer critiques, validator
  verdicts, judge scores, and the winning rationale — over mutual TLS to
  the in-cluster collector.
- Ceremony "meeting record": the winning contribution of each ceremony step
  is returned on the `RunCeremony` response (`CeremonyStepExecution.output`),
  so the full prose outcome of a run is a first-class API artifact.
- LLM-as-judge scoring: an optional `JudgeAwareScoring` strategy fed by an
  `LlmJudgeValidator` that ranks deliberation proposals by intrinsic
  quality instead of validator pass-fraction. Opt-in via
  `MADE_JUDGE_ENABLED` (with `MADE_JUDGE_THRESHOLD`), reusing the vLLM
  endpoint/model; fail-fast wiring and a Helm chart guard refuse a
  judge-on-without-vLLM configuration. Covered by unit tests and a
  provider-backed E2E.
- Ceremony engine: `RunCeremony` executes YAML-defined ceremonies as
  finite-state machines (states, steps with pluggable handlers, guarded
  transitions, roles), with multi-agent panels, a run-time context brief
  injected into each agent's task, and a Mermaid sequence diagram in the
  response. Catalog ceremonies (daily standup, technical debate, sprint
  planning, speaker + Q&A) run end-to-end in CI, driven by the
  `made-run-ceremony` operator tool.
- Helm persistence for the judge + vLLM provider env in the
  `underpass-runtime` overlay, guarded by a CI marker and a chart `fail`
  assertion enforcing the judge↔vLLM coupling.
- Product usability and publication planning:
  `docs/product-usability-publication-plan.md` and
  `docs/product-publication-checklist.md`.
- Explicit documentation that MADE is agnostic and
  independently usable; KMP, PIR, Runtime, and other projects are study
  cases or optional integrations, not required dependencies.
- Local no-external-service quickstart:
  `MADE_NATS_ENABLED=false just run`.
- MCP fixture and live-gRPC quickstarts, plus examples for
  `CreateCouncil`, `RegisterAgent`, `RegisterContract`,
  `RunCouncilDecision`, and `Orchestrate`.
- Repo-owned compose E2E guide covering the compose scenarios, stubs,
  Report schema, and provider-shaped OpenAI/vLLM paths.
- E2E runner scenario selection through `MADE_E2E_SCENARIOS`, with
  groups for `compose`, `cluster-connectivity`, `runtime-stub`, and
  `structured-output`.
- Consumer smoke `positive-path`, including Report contract
  registration, Strict-mode `RunCouncilDecision`, provider-shaped
  OpenAI/vLLM agents, and optional NATS causality assertions.
- Helm install profiles for minimal standalone, embedded NATS,
  Postgres DSN from Secret, provider environment Secret wiring, and the
  Underpass Runtime executor profile.
- Kubernetes deployment guide covering minimal install, embedded NATS,
  gRPC TLS/mTLS, Postgres secret sourcing, provider environment
  secrets, Runtime executor TLS, and operator smokes.
- Support matrix covering Rust toolchain, image tags, chart versions,
  provider adapters, and Kubernetes posture.
- Upgrade, rollback, and operator deploy verification runbooks for
  pinned images, Secret references, OCI chart installs, and smoke
  checks.
- Security policy covering supported scope, private vulnerability
  reporting, coordinated disclosure, deployment hardening, and secret
  containment.

### Changed

- `made_list_ceremony_instances` no longer fails a whole listing because one
  stored instance cannot be rehydrated. An instance whose definition was never
  published — the documented published-definition restart boundary — comes
  back as `{"ceremony_id": …, "rehydratable": false, "reason": …}` beside the
  instances that did recover. Reading that instance by id still fails: the
  listing degrades, the direct read does not pretend.

- **Renamed: Underpass Choreographer is now MADE by Underpass** — the
  Multi-Agent Deliberation Engine. The repository moved to
  `underpass-ai/made`. Every naming surface moved with it, and all of these
  are breaking for existing callers and deployments:
  - crates `choreo-*` → `made-*`, and the server binary `choreo` → `made`;
  - proto package `underpass.choreo.v1` → `underpass.made.v1`, service
    `ChoreographerService` → `MadeService`;
  - MCP tools `choreo_*` → `made_*`;
  - environment variables `CHOREO_*` / `CHOREOGRAPHER_*` → `MADE_*`;
  - NATS subjects `choreo.*` → `made.*` and Prometheus metrics
    `choreo_*` → `made_*`;
  - Helm chart `charts/choreographer` → `charts/made`, default namespace
    `choreographer-system` → `made-system`, image
    `ghcr.io/underpass-ai/underpass-choreographer` →
    `ghcr.io/underpass-ai/made`;
  - Codex plugin `plugins/choreographer` → `plugins/made`.

  Behavior is unchanged: this release renames, it does not re-scope. The
  engine still runs councils, ceremonies, contracts and judge scoring
  exactly as before.

- Contract gate validators tolerate Markdown-fenced JSON payloads (#120):
  a proposal that is *purely* a fenced JSON block is unwrapped before
  validation, so the gate measures evidence quality, not transport
  cosmetics. Mixed prose+fence payloads still fail.

- Kubernetes E2E jobs default to cluster-connectivity scenarios instead
  of running fixture-only stub scenarios against real deployments.
- `make e2e-compose` keeps the full compose group as the fixture-backed
  end-to-end path.
- Helm render checks now cover pinned-image enforcement, TLS secret
  validation, embedded NATS wiring, Runtime executor failure modes,
  Postgres Secret rendering, and provider env Secret rendering.

### Validation

- MCP catalog parity is checked against the gRPC proto surface.
- Compose E2E has been validated through all nine scenarios, including
  structured Report output and provider-shaped paths.
- Kubernetes smoke has been validated with the selected
  cluster-connectivity group.
- `made-consumer-smoke` has been validated for rejection-path and
  positive-path behavior against local MADE, NATS, and
  `made-stub-llm`.

### Security

- Provider credentials, Postgres DSNs, and TLS materials are documented
  as secret-managed inputs, not values-file or descriptor content.
- Chart gates assert hardened pod defaults and prevent accidental
  rendering of literal Postgres DSNs in the Secret-backed profile.

### Known Limits

- No public immutable `v*` tag, release image, OCI chart, or crates.io
  package has been cut yet; current published `sha-*` images are RC
  smoke artifacts, not stable release artifacts.
- `made-mcp` can only be published after `made-mcp-proto v0.1.0`
  is available in crates.io.
- Provider-backed positive smokes are validated with deterministic
  OpenAI-compatible stubs unless a real provider is explicitly wired by
  the operator.
- The Helm chart does not manage Ingress, provider egress allow-lists,
  or multi-replica/state coordination beyond the documented single
  replica posture.
