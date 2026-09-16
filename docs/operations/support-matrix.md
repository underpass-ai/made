# Support Matrix

This page records what the repository actually supports today. A
support claim belongs here only when the source of truth and the
enforcement gate are both named.

MADE remains independently usable. KMP, PIR, Runtime, provider
endpoints, and downstream products may be useful integration cases, but
they do not define this repository's support matrix unless this chart,
binary, or API requires them.

## Editions

MADE ships two editions ([`editions.md`](../editions.md)), and the ceremony
engine is reachable through four surfaces: the `underpass.made.v1` proto
contract, the MCP server on the gRPC backend, the MCP server on the embedded
backend, and the `EmbeddedMade` Rust facade. Which surface serves what is not
prose: the source of truth is
[`../architecture/parity.tsv`](../architecture/parity.tsv), one row per
capability and one column per surface (ADR-014), and the table below is that
file grouped the way `made_discover_capabilities` groups it.

Rows are the capability groups declared in
`crates/made-mcp/src/guidance/capability_group.rs`. A cell says `supported`
when the surface serves **every** capability of the row, and `not supported`
with the reason otherwise; the reason is quoted verbatim from `parity.tsv`,
which is why it carries no markup. A group whose capabilities disagree about a
surface is not written as one row — "partially supported" is not a support
claim — so such a group is split per capability. `ceremony_design` is the only
group split today.

The last column names the gates that keep the row true:

- **F1 set equality** — `crates/made-mcp/src/protocol/parity_tests.rs`:
  `parity.tsv` against the proto RPC list, both MCP catalogs and the public
  methods of `EmbeddedMade`, in both directions. No server, milliseconds.
- **catalog ⇔ proto** — `crates/made-mcp/src/protocol/tests.rs`: one MCP tool
  per RPC, and no catalog entry without one except the two the MCP server
  answers itself.
- **F4 session parity** —
  `crates/made-tests-integration/tests/mcp_parity_session.rs`: one working
  session driven through every shared tool on both MCP backends, the two
  answers compared field for field.

The table itself is checked against `parity.tsv` by
`crates/made-mcp/src/protocol/editions_matrix_tests.rs`, which needs no server
and runs in `cargo test -p made-mcp`: a cell that claims what the TSV denies
fails by name, in either direction.

<!-- editions:begin -->

| Capability group | proto | MCP on gRPC | MCP embedded | `EmbeddedMade` facade | Proved by |
|---|---|---|---|---|---|
| `self_description` | not supported — server tool: the MCP server answers it itself on every backend, so it has no RPC and no parity.tsv row | supported | supported | not supported — server tool: it describes the running server rather than the engine, so the facade has no method for it | catalog ⇔ proto (`protocol/tests.rs`), which pins these as the only catalog entries with no RPC |
| `council_deliberation` | supported | supported | not supported — council surface: cluster-only until B3 (ADR-014) | not supported — council surface: cluster-only until B3 (ADR-014) | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`) |
| `council_configuration` | supported | supported | not supported — council configuration: cluster-only until B3 (ADR-014); contract registry: cluster-only until B3 (ADR-014) | not supported — council configuration: cluster-only until B3 (ADR-014); contract registry: cluster-only until B3 (ADR-014) | F1 set equality (`protocol/parity_tests.rs`); catalog ⇔ proto (`protocol/tests.rs`) |
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

Two capabilities are in no group, because no MCP tool serves them:
`list_ceremony_definitions` and `mount_definition` are `EmbeddedMade` host
affordances — the definition catalog a host mounts into is local to its
process — and `parity.tsv` carries the reason for each.

<!-- editions:end -->

### Supported

- Every cell above that says `supported`, on the surface that carries it. The
  cluster edition serves the proto contract and MCP on the gRPC backend; the
  embedded edition serves MCP on the embedded backend and the `EmbeddedMade`
  facade.
- `tools/list` on the running executable as the authority for what that build
  serves. `made_discover_capabilities` filters the catalog by backend and
  answers with these same group ids.
- `made-api`'s `CeremonyEngineApi` as a versioned, read-mostly subset
  (ADR-004), not a fifth surface. `parity.tsv` carries it in an `api` column
  that is allowed to be smaller without a reason.

### Not Supported

- Every cell above that says `not supported`, for the reason the cell gives:
  the council surface — deliberation, and council, agent and contract
  configuration — outside the cluster edition; `made_validate_ceremony_draft`
  and `made_explain_ceremony_draft` as facade methods;
  `made_discover_capabilities` and `made_get_help` as an RPC or a facade
  method.
- A capability claimed on a surface with no row in `parity.tsv` behind it, or
  with a row that says otherwise. The gates above fail it; this page does not
  arbitrate.
- "Partially supported" as a row. A group whose capabilities disagree is split
  per capability, so that no claim covers a capability it is not true of.
- "MADE supports X" without naming the edition, the surface, and the build
  that serves it. See
  [capability-verification.md](./capability-verification.md).

### Change Rule

A capability changes surface only with its `parity.tsv` row and this table in
the same PR. That PR updates together:

- the code on the surface that gains or loses the capability;
- its row in `docs/architecture/parity.tsv`;
- this table, including the gate column;
- `docs/editions.md`, wherever its prose names the gap;
- `CHANGELOG.md`.

Nothing here is trusted to a reviewer's memory: `parity_tests.rs` fails when
the code and `parity.tsv` disagree, and `editions_matrix_tests.rs` fails when
`parity.tsv` and this table disagree. Both run in
`cargo test -p made-mcp --locked` and in the `test` job of
`.github/workflows/quality-gate.yml`.

## Rust Toolchain

Current support is exact-version support, not a broad Rust range.

| Surface | Supported | Source of truth | Enforcement |
|---|---:|---|---|
| Workspace MSRV | `1.97` | `Cargo.toml` `[workspace.package].rust-version` | Cargo package metadata |
| Local toolchain | `1.97.1` | `rust-toolchain.toml` | `cargo`, `clippy`, `rustfmt` use the pinned toolchain |
| PR CI toolchain | `1.97.1` | `.github/workflows/quality-gate.yml` | `rustfmt`, `clippy`, tests, benches compile |
| Container-backed CI toolchain | `1.90.0` | `.github/workflows/integration.yml` | NATS and Postgres integration workflows |
| Developer setup | `1.90.0` | `docs/dev-loop.md` | Manual local setup and `just` recipes |
| Dependency resolution | `Cargo.lock` committed | `Cargo.lock` | CI and local gates use `--locked` |

### Supported

- Rust `1.90.0` with the components in `rust-toolchain.toml`:
  `clippy` and `rustfmt`.
- The locked dependency graph in `Cargo.lock`.
- The provider-feature matrix used by CI:
  `made-adapters/agent-anthropic`,
  `made-adapters/agent-openai`, and
  `made-adapters/agent-vllm`.

### Not Supported

- Older Rust versions.
- Nightly-only compiler features.
- "Latest stable" as a moving target.
- Builds that require dependency resolution different from the
  committed `Cargo.lock`.

### Change Rule

Changing the supported Rust version requires one PR that updates all of
these together:

- `Cargo.toml`;
- `rust-toolchain.toml`;
- `.github/workflows/quality-gate.yml`;
- `.github/workflows/integration.yml`;
- `docs/dev-loop.md`;
- this support matrix;
- `CHANGELOG.md`.

That PR must run:

```bash
just check
just helm-lint
just integration
```

Release-candidate work should also run:

```bash
make e2e-compose
make e2e-kubernetes
```

Do not merge a Rust version bump based only on local `cargo check`.

## Container Image Tags

Published image repositories:

| Image | Purpose | Build source | Publish path |
|---|---|---|---|
| `ghcr.io/underpass-ai/made` | Product runtime image | `Dockerfile` | `.github/workflows/publish-distribution.yml` |
| `ghcr.io/underpass-ai/made-e2e-runner` | Release/E2E runner image | `tests/e2e/runner.Dockerfile` | `.github/workflows/publish-distribution.yml` |

The provider-E2E image built by `scripts/ci/build-provider-image.sh`
is operator-pushed test tooling. It is not a supported production
runtime image.

| Reference form | Produced by | Support status | Production use |
|---|---|---|---|
| `image@sha256:<digest>` | Registry content digest | Supported and preferred | Yes. Use for normal Helm installs. |
| `image:vX.Y.Z` | `v*` tag publish workflow | Supported release label after the tag exists | Acceptable when release immutability is controlled; digest is still preferred. |
| `image:sha-<short>` | publish workflow commit tag | Supported for CI, release-candidate smoke, and cluster verification | Use only when the rollout records the source commit; digest is preferred. |
| `image:main` | default-branch publish workflow | Moving branch pointer | No. Development or smoke only. |
| `image:latest` | default-branch publish workflow | Moving branch pointer | No. The chart rejects it unless `development.allowMutableImageTags=true`. |
| `image:e2e-latest` | E2E runner publish workflow | Moving E2E runner pointer | No. Test runner only. |
| `image:dev`, `image:ci`, `image:e2e` | local scripts and test manifests | Local-only | No. Local compose/kind/k3d only. |

### Helm Image Rules

- `image.digest` wins over `image.tag`.
- Either `image.digest` or `image.tag` must be set; the chart has no
  production default image reference.
- `image.tag=latest` fails chart rendering unless
  `development.allowMutableImageTags=true`.
- `docs/operations/deploy-kubernetes.md` uses digests for normal
  installs and local tags only with an explicit development escape
  hatch.
- `scripts/ci/helm-lint.sh` asserts the missing-image and mutable-tag
  failure paths.

### Change Rule

Changing image tag support requires updating:

- `.github/workflows/publish-distribution.yml`;
- `scripts/ci/container-image.sh`, if local build semantics change;
- `charts/made/templates/_helpers.tpl`, if chart acceptance
  changes;
- `scripts/ci/helm-lint.sh`;
- `docs/operations/deploy-kubernetes.md`;
- this support matrix;
- `CHANGELOG.md`.

## Helm Chart Versions

Chart source and release registry:

| Surface | Current value | Source of truth | Enforcement |
|---|---:|---|---|
| Chart path | `charts/made` | repository layout | `scripts/ci/helm-lint.sh` |
| Chart name | `MADE` | `charts/made/Chart.yaml` | `helm lint` |
| Chart version | `0.1.0` | `charts/made/Chart.yaml` `version` | `scripts/release.sh release` |
| App version | `0.1.0` | `charts/made/Chart.yaml` `appVersion` | `scripts/release.sh release` |
| Kubernetes version floor | `>=1.28.0-0` | `charts/made/Chart.yaml` `kubeVersion` | Helm client compatibility check |
| OCI registry | `oci://ghcr.io/underpass-ai/charts/made` | `.github/workflows/publish-distribution.yml` | `helm package` + `helm push` |

| Chart reference | Support status | Notes |
|---|---|---|
| Checkout chart at `charts/made` | Supported for development, PR review, and release-candidate validation | Must pass `bash scripts/ci/helm-lint.sh`. |
| `oci://ghcr.io/underpass-ai/charts/made:0.1.0` | Pending | `0.1.0` is present in metadata but no public `v0.1.0` tag exists in this checkout yet. |
| `oci://ghcr.io/underpass-ai/charts/made:X.Y.Z` | Supported after the matching `vX.Y.Z` release tag publishes successfully | Chart `version`, `appVersion`, and workspace version must match. |
| Older chart versions | Not currently supported | No stable-release support window has been declared yet. |
| Unversioned or moving chart references | Not supported | Use an explicit chart version. |

### Version Lockstep

The release helper keeps these values in lockstep:

- `Cargo.toml` `[workspace.package].version`;
- `charts/made/Chart.yaml` `version`;
- `charts/made/Chart.yaml` `appVersion`;
- Git tag `vX.Y.Z`;
- published product image tag `vX.Y.Z`;
- published E2E runner image tag `vX.Y.Z`;
- OCI chart version `X.Y.Z`.

Do not publish or document a chart version whose `appVersion` does not
match the binary/image version it is meant to deploy.

### Change Rule

Changing chart version support requires updating:

- `charts/made/Chart.yaml`;
- `scripts/release.sh`;
- `.github/workflows/publish-distribution.yml`;
- `docs/release.md`;
- `docs/operations/deploy-kubernetes.md`, if install commands change;
- this support matrix;
- `CHANGELOG.md`.

## Provider Adapters

Provider support has three separate gates:

1. The binary must be compiled with the provider feature.
2. Required `MADE_*` environment variables must be present at boot.
3. The registered agent must use a kind listed in the startup
   `agent_kinds=...` log field.

Credentials must stay in environment or secret-managed files. Agent
descriptors may be persisted and must not carry credentials.

| Kind | Cargo feature | Default product image | Required env | Optional env | Repo validation | Support status |
|---|---|---:|---|---|---|---|
| `noop` | none | yes | none | none | unit tests, local smoke, minimal Helm smoke | Always supported. |
| `openai` | `made-adapters/agent-openai` | yes | `MADE_OPENAI_API_KEY` | `MADE_OPENAI_MODEL`, `MADE_OPENAI_ENDPOINT`, `MADE_OPENAI_MAX_TOKENS` | CI compile/test, compose scenario with OpenAI-compatible stub, consumer positive-path | Supported adapter shape. Real provider credentials, quotas, endpoint policy, and model behavior are operator-owned. |
| `vllm` | `made-adapters/agent-vllm` | yes | `MADE_VLLM_MODEL`, `MADE_VLLM_ENDPOINT` | `MADE_VLLM_BEARER_TOKEN`, `MADE_VLLM_MAX_TOKENS`, `MADE_VLLM_TIMEOUT_SECS` | CI compile/test, compose scenario with OpenAI-compatible stub, provider-E2E runner for real vLLM, gRPC council runner, MCP council runner (operator-run real execution pending) | Supported adapter shape. Service factory Helm path supports endpoint/model/bearer; vLLM client cert envs are provider-E2E runner only today. |
| `anthropic` | `made-adapters/agent-anthropic` | no | `MADE_ANTHROPIC_API_KEY` | `MADE_ANTHROPIC_MODEL`, `MADE_ANTHROPIC_ENDPOINT`, `MADE_ANTHROPIC_MAX_TOKENS` | CI compile/test | Implemented feature, not included in the default Dockerfile, and not covered by repo-owned E2E yet. Operators need a downstream image that enables the feature. |

Per-agent descriptor attributes supported by provider adapters:

| Attribute | Type | Meaning |
|---|---|---|
| `provider.model` | string | Override the env/default model for this agent. |
| `provider.endpoint` | string | Override the env/default endpoint for this agent. |
| `provider.max_tokens` | number, non-negative `u32` | Override token budget for this agent. |

### Image Feature Matrix

| Artifact | Provider features enabled |
|---|---|
| Default product `Dockerfile` | `agent-openai`, `agent-vllm` |
| `just check` / `scripts/ci/quality-gate.sh` | `agent-anthropic`, `agent-openai`, `agent-vllm` |
| Compose E2E stack | OpenAI-compatible and vLLM-compatible stub paths |
| Provider-E2E runner image | real `agent-vllm` runner path |
| MCP council runner | real `agent-vllm` path through `made-mcp` stdio; operator-run execution pending |

### Change Rule

Changing provider support requires updating:

- `crates/made-adapters/Cargo.toml`;
- `crates/made-adapters/src/agents/factory.rs`;
- `Dockerfile`, if the default product image feature set changes;
- `justfile` and `scripts/ci/quality-gate.sh`, if CI feature coverage
  changes;
- `docs/operations/deploy-kubernetes.md`;
- this support matrix;
- `CHANGELOG.md`.

If a provider is described as end-to-end supported, add or point to a
repo-owned smoke path that proves the provider shape without leaking
credentials.

## Kubernetes Posture

The Helm chart supports multiple deployment postures. "Supported" here
means the chart renders the posture, the binary has the corresponding
code path, and `scripts/ci/helm-lint.sh` or an operator smoke covers
the shape.

| Posture | Values / knobs | Status | Notes |
|---|---|---|---|
| Minimal standalone | `values.minimal.yaml` | Supported first-install smoke | No NATS, no Postgres, no Runtime, no provider credentials, no gRPC TLS. Not a hardened internet-facing posture. |
| Standalone with embedded NATS | `values.embedded-nats.yaml` | Supported standalone event bus | Release-local core NATS, no JetStream storage by default, no external NATS operation required. |
| Postgres persistence from Secret | `values.postgres-secret.yaml` | Supported for current persistent repositories | Persists deliberations, councils, agents, and statistics. Output contracts are still registered/seeded separately. |
| Provider env secrets | `values.provider-env-secrets.yaml`, `providerEnv`, `providerEnvFrom` | Supported env wiring | Secret injection only. Provider features and required env still gate runtime availability. |
| gRPC server TLS | `tls.mode=server` + `tls.existingSecret` | Supported | Secret must contain `tls.crt` and `tls.key`; health endpoints remain plaintext HTTP in-cluster. |
| gRPC mutual TLS | `tls.mode=mutual` + `tls.existingSecret` | Supported | Secret must contain `tls.crt`, `tls.key`, and client CA `ca.crt`. |
| Runtime executor | `executor.kind=runtime` + endpoint | Supported optional executor | Chart fails render without endpoint. Use only when Runtime is actually deployed. |
| Runtime client TLS/mTLS | `executor.runtime.tls.*` | Supported | Chart fails render for non-disabled Runtime TLS without an existing Secret. |
| NetworkPolicy | `networkPolicy.enabled=true` | Supported opt-in | Requires a NetworkPolicy-capable CNI and operator-owned ingress selectors / egress rules. |
| PodDisruptionBudget | `pdb.enabled=true` | Supported opt-in | Meaningful for multi-replica deployments only. Default is single replica and PDB off. |
| Non-root pod hardening | default `podSecurityContext`, `securityContext`, `automountServiceAccountToken=false` | Supported default | Chart lint asserts non-root, read-only root filesystem, dropped capabilities, seccomp, and no service-account token mount. |

### Not Supported As Chart-Owned Posture Today

- Public internet exposure directly from the Service without TLS/mTLS,
  mesh, gateway, or equivalent controls.
- Chart-managed Ingress. `values.yaml` has reserved ingress fields, but
  no Ingress template is shipped yet.
- Embedded NATS as a highly available durable event store. Current
  MADE events are fire-and-forget and the embedded profile
  disables JetStream by default.
- Multi-replica production state without an explicit state plan.
  Postgres covers current persistent repositories, but output-contract
  registration is still process-local and should be registered/seeded
  consistently after rollout.
- Provider egress allow-lists generated automatically by the chart.
  Add provider endpoint rules through `networkPolicy.egress.extra` or
  route providers through an operator-managed egress proxy.

### Change Rule

Changing Kubernetes posture support requires updating:

- `charts/made/values.yaml`;
- any checked-in profile under `charts/made/values.*.yaml`;
- relevant templates under `charts/made/templates/`;
- `scripts/ci/helm-lint.sh`;
- `docs/operations/deploy-kubernetes.md`;
- this support matrix;
- `CHANGELOG.md`.
