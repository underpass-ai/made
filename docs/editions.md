# Editions: embedded and cluster

MADE ships as **two editions**. This page is the canonical answer to "which one
do I run, what does it actually give me, and what does it not prove".

Both editions share `made-core`, `made-app`, the domain invariants and the
workspace release version. The embedded crate is not a second ceremony engine
and does not duplicate domain behaviour: it calls the same application use cases
the deployable composition calls.

```text
                  made-core
            domain + ports + invariants
                       ^
                       |
                   made-app
                  use cases
                 /         \
       made-embedded      made
       host callbacks     gRPC / NATS / HTTP
       injected ports     deployment config
```

Ceremony definitions keep their own independent `CeremonyVersion`. Release
version and definition version solve different compatibility problems.

## The short version

| | **Embedded edition** | **Cluster edition** |
|:--|:--|:--|
| Who it is for | one developer running a working session | a team whose deliberations must outlive a process |
| Entry point | `made-mcp` (stdio MCP) or the `made-embedded` library | the `made` binary |
| Surface today | the **ceremony engine** | the full `underpass.made.v1` gRPC contract |
| Persistence | one local redb state file | Postgres, or in-memory |
| Messaging | none | optional NATS |
| Agents | whatever the host injects | provider-backed, feature-gated at build, credentialed at boot |
| Judge | host's choice | opt-in `MADE_JUDGE_ENABLED`, fail-fast on misconfiguration |
| Observability | host's choice | Prometheus at `/metrics`, OTLP traces |
| Requires | nothing | a cluster, or at least a running binary |
| Select with | `MADE_MCP_BACKEND=embedded` | `MADE_MCP_GRPC_ENDPOINT=…` (backend defaults to `grpc`) |

There is also a **fixture backend** (`MADE_MCP_BACKEND=fixture`) that returns
deterministic canned responses. It is for wiring an MCP client and validating
tool choice; it is not an edition and must be selected explicitly.

## Embedded edition

Status: implemented first slice. The embedded surface currently covers the
**ceremony engine**. Native embedded facades for the broader council and
deliberation APIs are **not claimed**.

### Install and run

```bash
cargo install made-mcp
```

The embedded backend **requires** `MADE_MCP_REDB_PATH`. This is deliberate:
where ceremony state survives a restart is an operator decision, never a
default this crate invents. Starting without it fails fast with
`MADE_MCP_REDB_PATH is required when MADE_MCP_BACKEND=embedded`.

```bash
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made"
MADE_MCP_BACKEND=embedded \
MADE_MCP_REDB_PATH="${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb" \
  made-mcp
```

Host wiring for Claude Code and Codex CLI is in the
[README](../README.md#start-here--pick-an-edition). The
[MADE plugin](../plugins/made/README.md) picks the state path for you, ships
the `design-ceremony` and `run-ceremony` skills, and imports a pre-rename
Choreographer store on first start. Its launcher executes `bin/made-mcp` inside
the plugin directory, so install it from a release package rather than a bare
checkout.

### Embedding it in your own Rust host

`CallbackCeremonyStepHandler` turns an async Rust callback into a
`CeremonyStepHandlerPort`. It is the smallest useful boundary for a host that
owns its own agent runtime, tool system, or human interaction.

```rust,no_run
let made = EmbeddedMade::builder()
    .with_step_handler_callback(|request| async move {
        let _kind = request.handler_kind();
        // Delegate to the host's own agent / tool / human subsystem here.
        StepResult::completed(StepOutput::empty())
    })
    .build();
```

The builder also accepts `Arc<dyn …Port>` for the definition repository,
instance repository, transcript store, step handler, clock, and metrics
recorder. The host keeps ownership of its async runtime and the lifecycle of
everything it injects. Details: [embedded-made.md](embedded-made.md).

### What it guarantees

- **The real engine.** Same use cases, same domain invariants, same FSM as the
  deployable binary.
- **Durable ceremony state** with the `redb` feature: ceremony snapshots,
  unit-of-work state, the audit journal, outbox rows and published definitions
  survive process restarts. Crash/reopen behaviour is exercised by
  `crates/made-embedded/tests/redb_engine_api.rs`.

### What it explicitly does not prove

These three are the reason this repo ships a
[capability-verification runbook](operations/capability-verification.md).

1. **Durable is not authorized.** `EmbeddedMade::open_redb(path)` makes the
   ceremony store durable. It does **not** silently make every port durable:
   mounted definition repositories and transcripts keep their in-memory
   defaults, and step execution and evidence collection keep their no-op
   defaults, unless the host injects real implementations. A terminal step from
   a `NoopCeremonyStepHandler` proves ceremony protocol and state-machine
   behaviour — not that an agent, tool, API, or human performed the requested
   work.
2. **Only published definitions rehydrate.** An instance started from a mounted
   (unpublished) definition persists its state but cannot be loaded after the
   store reopens — it fails with `not found: ceremony_definition`. The listing
   reports those as `"rehydratable": false`. Publish the definition first if you
   need to resume across restarts.
3. **Discovery is not configuration.** `made_discover_capabilities` returns a
   backend-filtered catalog that is authoritative for the *installed executable
   surface*. It does not prove a real step handler, durable store, credentials,
   or external authority are wired.

"MADE supports X" is an incomplete sentence until you name the edition, the
backend, the tools the running executable exposes, who performs the external
work, and what survives a restart.

## Cluster edition

The `made` binary reads `MADE_*` configuration from the environment and serves
the full `underpass.made.v1` contract. Every RPC is backed by a use case; none
returns `UNIMPLEMENTED`.

### Run it locally first

```sh
MADE_NATS_ENABLED=false MADE_SEED_SPECIALTIES=triage just run
```

In-memory persistence, noop messaging, the default noop executor, gRPC on
`localhost:50055`, and one exercisable council per seeded specialty.

### Deploy it

Helm chart under `charts/made/`, with checked-in profiles: minimal (noop,
in-memory), embedded NATS, Postgres-secret, and a runtime profile wiring mTLS
to the execution plane, a vLLM endpoint, and the judge at a 0.5 threshold.
Guide: [operations/deploy-kubernetes.md](operations/deploy-kubernetes.md).

The hardening is enforced by a chart-render CI gate (`scripts/ci/helm-lint.sh`)
that refuses a manifest which drops any of it:

- pinned images only — a `latest` tag is refused unless
  `development.allowMutableImageTags` is set;
- non-root pod and container security contexts, read-only root filesystem,
  `ALL` capabilities dropped, `seccompProfile: RuntimeDefault`;
- `automountServiceAccountToken: false` — the binary never calls the Kubernetes
  API;
- opt-in NetworkPolicy restricting inbound to declared ports and outbound to
  DNS, NATS, Postgres and OTLP;
- `MADE_POSTGRES_URL` sourced via `valueFrom.secretKeyRef`;
- optional PodDisruptionBudget.

### What it guarantees

- **Persistence is all-or-nothing.** With `MADE_POSTGRES_URL` set,
  deliberations, councils, the agent registry and operational statistics all
  persist; otherwise all of them are in-memory. No replica ever reads from a
  split source of truth. Migrations apply on startup.
- **Concurrent replicas accumulate correctly.** Statistics counters use
  `INSERT … ON CONFLICT DO UPDATE … x = x + 1`, verified by a 50-concurrent-record
  integration test.
- **No pickled provider state crosses the database boundary.** Agents persist as
  descriptors; live handles are rehydrated through the wired factory on resolve.
- **Provider wiring fails loud.** The dispatching factory materializes `noop`
  unconditionally plus any provider whose Cargo feature is compiled in *and*
  whose credentials are present at boot. An unsupported kind is an error, never
  a silent no-op. Startup logs `agent_kinds=`.
- **The judge cannot degrade silently.** Enabled without an endpoint, model or
  threshold, composition fails rather than falling back at runtime.

### Current limits, stated plainly

- `StreamDeliberation` emits phase transitions and a final `DeliberationResult`
  frame — **not** per-proposal, per-critique or per-revision events. That
  arrives in a later slice.
- Provider-backed `RegisterAgent` kinds require the matching Cargo feature and
  boot-time credentials; `noop` is always available.
- Deferred observability: gRPC front-door RED (already covered by request
  traces) and per-query Postgres latency.

## Moving between them

Switching which engine the MCP adapter talks to is configuration:

```sh
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 made-mcp
```

Two caveats that make this less symmetric than the KMP equivalent:

- The **tool surfaces differ by design**. Embedded-only ceremony controls and
  read-only Markdown reports exist where no remote RPC does. `tools/list` on the
  running executable is the authority, and
  `made_discover_capabilities` filters the catalog by backend for exactly this
  reason.
- **Ceremony state does not migrate itself.** A local redb store is not a
  Postgres deployment. Republish the definitions you need and start fresh
  instances; treat it as a migration, not a config flip.

## Choosing

**Start embedded when** you want to run a structured working session in the
terminal you already work in, the participants are the agents your host already
has, and nothing about the outcome has to be defensible to someone who was not
there.

**Move to the cluster when** deliberations must survive process loss and scale
across replicas, when agents must be provider-backed and centrally
credentialed, when the judge must run, or when a past decision has to be
replayable by trace ID months later.
