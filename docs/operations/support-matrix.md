# Support matrix

This checkout supports two compositions: local embedded ceremonies and the
service distribution. The four ceremony surfaces are protobuf, MCP over gRPC,
MCP embedded and the Rust facade. The following table is a checked contract:
its cells derive from [parity.tsv](../architecture/parity.tsv), with tests
comparing real methods/catalogues and complete sessions across backends.
The historical ADR labels in reason cells are stable ledger identifiers;
[architecture](../architecture/README.md) explains the current boundaries.

<!-- editions:begin -->

| Capability group | proto | MCP on gRPC | MCP embedded | `EmbeddedMade` facade | Proved by |
|---|---|---|---|---|---|
| `self_description` | not supported — server tool: the MCP server answers it itself on every backend, so it has no RPC and no parity.tsv row | supported | supported | not supported — server tool: it describes the running server rather than the engine, so the facade has no method for it | catalog ⇔ proto (`protocol/tests.rs`), which pins these as the only catalog entries with no RPC |
| `council_deliberation` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `council_configuration` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `council_journal` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_design` / `design_ceremony` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_design` / `validate_ceremony_draft` | supported | supported | supported | not supported — decided in F1: no facade method; analysing a draft is a pure domain call (CeremonyDraft::analyze) the adapter makes directly, and made-api exposes it as analyze_definition | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_design` / `explain_ceremony_draft` | supported | supported | supported | not supported — decided in F1: same as validate_ceremony_draft; the two tools render one analysis for two audiences | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_design` / `publish_ceremony_definition` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_design` / `diff_ceremony_definitions` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_execution` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_recovery` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `human_authorization` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_participation` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `service_observability` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_history` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_reporting` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `ceremony_budgets` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |
| `artifact_transfer` | supported | supported | supported | supported | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`); F4 session parity (`mcp_parity_session.rs`) |

Two capabilities are in no group, because no MCP tool serves them:
`list_ceremony_definitions` and `mount_definition` are `EmbeddedMade` host
affordances — the definition catalog a host mounts into is local to its
process — and `parity.tsv` carries the reason for each.

<!-- editions:end -->

## Build and deployment inputs

| Input | Source and supported use |
|:--|:--|
| Rust | `Cargo.toml` MSRV 1.97; `rust-toolchain.toml` pins 1.97.1; use the committed lockfile |
| Plugin targets | Linux x86_64/arm64, macOS arm64, Windows x86_64; release assets and installer select exact target |
| Image | Pinned digest preferred; release tags only after successful publication; `latest` requires the chart's development override |
| Chart | `charts/made`; explicit version for OCI installs; Kubernetes floor in `Chart.yaml` |
| Ceremony persistence | SQLite when explicitly configured; published definitions required for rehydration |
| Council persistence | Explicit SQLite composition persists councils, agents, contracts, deliberations, statistics and their independent journal; the default builder remains in-memory; the service can use Postgres adapters |
| Artifact persistence | Local durable directory with process coordination, or shared Postgres metadata and chunks for replicas; uploads and reads are bounded and resumable |
| Messaging | Optional core NATS pub/sub; durable ceremony consumption uses the event feed/cursor contract |
| Providers | Build feature + environment + registered kind; default image includes OpenAI and vLLM, Anthropic needs a custom feature-enabled build |

The embedded facade and embedded MCP backend expose council deliberation,
configuration and independent journal operations. The default builder uses
process-local registries. Explicit SQLite composition persists these records
and their cursor leases across processes; the council journal has its own
sequence and never shares ceremony cursor positions. A host can inject
alternative adapters through `EmbeddedMadeBuilder`.
The `made-api` trait remains a smaller consumer interface, not an additional
full transport. See [reference](../reference/README.md) and runtime discovery
before selecting an operation.

Support means the repository implements and checks the boundary; it does not
promise provider availability, model quality, an automatically started worker daemon or
compatibility with every historic release. See [deployment](deploy-kubernetes.md)
and [migrations](../migrations/README.md).

When a capability changes, update the code, parity ledger, this table and
migration/release notes together. `protocol/parity_tests.rs` checks the source
surfaces, `protocol/editions_matrix_tests.rs` checks this table and
`mcp_parity_session.rs` checks shared behavior.

The independent council journal additionally has exact embedded/gRPC MCP parity, lease fencing and durable cursor restart coverage in `council_journal_surfaces.rs`, plus shared store conformance in `council_journal_conformance.rs`.

The recoverable worker host is an explicit Rust composition with bounded
concurrency, deadline checks and cooperative stop. The shipping MCP and service
expose receipt lookup, recovery inspection, completion and adoption. Configuring
a connector and running the host loop remain host responsibilities. See
[recoverable workers](recoverable-workers.md) for the effect and recovery contract.
