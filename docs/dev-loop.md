# Developer loop

Honest recipes for iterating on MADE. Each
command mirrors a CI gate one-for-one — when CI is red, the same
command produces the same failure locally.

## Setup

```bash
# Rust toolchain pinned at the workspace minimum.
rustup toolchain install 1.97.1
rustup default 1.97.1

# Optional but recommended — installs command aliases from justfile.
cargo install just --locked

# Contract gate dependencies.
bash scripts/ci/install-buf.sh
bash scripts/ci/install-asyncapi.sh

# Protoc for tonic code generation.
# (Debian/Ubuntu: apt install protobuf-compiler; Fedora: dnf install protobuf-compiler)
protoc --version

# Container runtime for integration / E2E suites. Either docker or
# podman works. testcontainers-rs auto-detects DOCKER_HOST.
docker version  # or: podman version
```

Podman users need the user-level socket running:

```bash
systemctl --user start podman.socket
export DOCKER_HOST=unix://$(podman info --format '{{.Host.RemoteSocket.Path}}')
test -S "$(podman info --format '{{.Host.RemoteSocket.Path}}')"
```

If that preflight check fails, `testcontainers` will not be able to
start NATS or Postgres. In that case either:

```bash
# Preferred: bring up the user socket through systemd.
systemctl --user start podman.socket

# Fallback: run an explicit API service on a temporary Unix socket.
mkdir -p "${TMPDIR:-/tmp}/podman"
podman system service --time=0 unix://${TMPDIR:-/tmp}/podman/podman.sock
export DOCKER_HOST=unix://${TMPDIR:-/tmp}/podman/podman.sock
```

The integration scripts fail fast with this guidance when no live
Docker-compatible socket is available, instead of letting Rust tests die
later with `SocketNotFoundError`.

## Daily commands

```bash
just                 # list every recipe
just dev             # the draft loop: fmt + clippy + test for DEV_PACKAGES + gates
just check           # workflow-contract + contract + fmt-check + clippy + test + bench-compile
just fmt             # apply rustfmt in-place
just contract        # proto + AsyncAPI gate
just clippy          # warnings-as-errors on the full provider matrix
just test            # unit + in-process integration tests
just helm-lint       # helm lint + chart hardening assertions
```

Before opening a PR:

```bash
just check && just helm-lint
```

Every recipe in the cascade runs the script its CI job runs, so a failure
here is the failure CI would report. It is not the whole gate: `just check`
leaves out coverage (`just coverage`), the chart (`just helm-lint`) and the
container image, and a ready pull request runs all three when the impact
planner routes them. `just check && just coverage && just helm-lint` is the
closest a machine without Docker or podman gets.

## Draft and ready

Two loops, one handover.

**Open the pull request as a draft.** While it is a draft, the full gate
stands down and `dev-loop.yml` answers instead: formatting, clippy and
tests for the crates named in `DEV_PACKAGES`, the three architecture
gates that cost seconds, and a `made-mcp` embedded binary built for
linux-arm64 and uploaded as a workflow artifact. Minutes, not the full
matrix.

**Mark it ready for review** (`gh pr ready`) and the handover happens on
that event, not on the next push: `quality-gate.yml`, `integration.yml`
and `plugin-package.yml` all list `ready_for_review` among their trigger
types, and every job in them wakes with the `impact` job.

**Nothing merges on the dev loop's word.** The loop is feedback, not
proof. The `gate` job in `quality-gate.yml` exists for exactly that: a
skipped required check counts as satisfied on GitHub, so `gate` fails on
purpose while the pull request is a draft, and succeeds only when the
full gate has actually run and nothing required failed. Protect it on
`main` and the draft/ready handover is enforced rather than merely
documented.

### Running the same loop locally

```bash
just dev                            # every stage
just dev lint                       # fmt + clippy only
just dev test                       # tests only
just dev gates                      # the three seconds-long gates
DEV_PACKAGES="-p made-core" just dev  # narrow it for one run
```

`just dev` and the workflow are not two lists that mirror each other:
both run `scripts/ci/dev-loop.sh`. `scripts/ci/dev-loop-workflow-contract.py`
fails the build if they ever name different crates.

### What the ready gate actually runs

Marking a pull request ready does not run every job unconditionally. The
first job of `quality-gate.yml`, `impact`, plans the smallest honest gate
for the change and every other job reads its outputs:

- **Rust gates follow the reverse workspace dependency closure.** A change
  inside a crate affects that crate and everything that depends on it,
  read from the manifests by `scripts/ci/quality-gate-plan.py` rather than
  guessed. `made-core` reaches every crate, so a change there runs the whole
  Rust matrix.
- **The independent contracts follow path routing**: proto and AsyncAPI
  (`contract`), the embedded boundaries, the plugin bundle, the chart
  (`charts/**`), the container image (`Dockerfile`), coverage and the
  publication dry run. `helm` and `contract` are the two gates no Rust
  source can reach; nothing routes them from a crate change.
- **Documentation the workspace compiles is source.**
  `docs/architecture/parity.tsv`, `docs/operations/support-matrix.md` and
  the ceremony definitions under `tests/e2e/ceremonies/` are `include_str!`'d
  into `made-mcp`, `made-tests-integration`, `made-e2e-runner` and
  `made-adapters`, so editing one runs `clippy` and `test`. The planner's
  self-test walks every `include_str!` / `include_bytes!` under `crates/**`
  and fails if a target outside its own crate would route to no Rust job,
  so that list cannot go stale.
- **The change boundary keeps deletions and both sides of a rename.**
  `git diff -M --name-status`, not `--diff-filter=ACMR`: removing a crate
  source is a change to what compiles, and a rename's old path is where the
  routed file used to live.
- **It fails closed.** A path the router does not recognise, a change to
  `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `quality-gate.yml`,
  the router itself or the tree proof, and every `workflow_dispatch` run
  return the full matrix. Being wrong costs time, never safety.

Ask it what a change would run before pushing:

```bash
python3 scripts/ci/quality-gate-plan.py --path crates/made-core/src/lib.rs
python3 scripts/ci/quality-gate-plan.py --base origin/main --head HEAD
just workflow-contract     # the router's self-test + the workflow contract
```

A docs-only pull request runs no Rust job — unless the docs are compiled
into a crate, which the planner knows.

### Never twice for the same tree

A merge to `main` usually lands a tree byte-identical to the pull request
head that was just proved green: same tree hash, different commit. The
`tree-proof` job asks `scripts/ci/tree-already-proved.sh` whether a
successful `quality-gate` run already covered this exact tree, and skips the
gate when one did.

"Already covered" is narrow, because a proof accepted wrongly makes the hole
it came from permanent. A run proves its tree only when all of these hold:

- it ran in **this** repository — `head_repository.full_name` equals
  `GITHUB_REPOSITORY` — so a fork's run proves nothing here;
- it is younger than 14 days (`TREE_PROOF_MAX_AGE_DAYS`): a tree proved
  against a months-old toolchain, action pin or dependency graph is not
  proved against today's;
- **every job of the full matrix concluded `success`** — not skipped, not
  cancelled. That single test is also the plan check: a partial plan skips
  at least one gate, so a docs-only run cannot prove a tree, and neither can
  a push that itself skipped the gate on an earlier proof.

The script reads the run's job conclusions rather than the plan the run
recorded, because on a `pull_request` the planner runs from the pull
request's own head and its word for "full" is the pull request's word. The
`impact` job records the plan — `full`, the gates it selected, the event —
in the run summary all the same: it is the same statement in the form a
person reads.

The rule is "skip when this tree was already proved", never "trust the pull
request". A merge from an out-of-date branch, a conflict resolved in the
GitHub UI and a direct push each produce a tree nobody tested, and each
still runs the full gate — as does any doubt at all: no green run to compare
against, an API that will not answer, a commit that cannot be resolved.

`bash scripts/ci/tree-already-proved.sh --self-test` drives the same
decision the live path drives, with fixtures standing in for the API
answers: a partial pull-request proof refused, a full one accepted, a push
accepted, a fork refused, a stale proof refused, a skipped job refused, a
cancelled one refused. It runs in the `tree-proof` job on every event, and
in `just workflow-contract`.

### Changing `DEV_PACKAGES`

`DEV_PACKAGES` names the crates the current phase iterates on. It lives in
two places that must stay identical, and the contract script enforces that:

- `env.DEV_PACKAGES` in `.github/workflows/dev-loop.yml`
- the `DEV_PACKAGES="${DEV_PACKAGES:-…}"` default in `scripts/ci/dev-loop.sh`

Edit both, then run `just workflow-contract`. Widening it costs draft
minutes; narrowing it costs nothing in safety, because the full gate on
ready-for-review still proves the whole workspace.

## Container-backed checks

```bash
just integration     # integration-nats + integration-postgres
just integration-nats
just integration-postgres
```

Each spins testcontainers for the real service (NATS 2, Postgres 16)
via the system container runtime.

## End-to-end (manual only)

E2E is run manually from the repository before cutting a release:

```bash
make e2e-compose     # full stack via docker compose + runner
make e2e-kubernetes  # Kubernetes cluster + Helm chart + runner Job
```

For an existing cluster, the standard path matches sibling repos:
push the MADE image and runner image to `ghcr.io`, create
an `imagePullSecrets` named `ghcr-pull` in the target namespace, and
point the script at that registry.

```bash
podman login ghcr.io

kubectl create secret docker-registry ghcr-pull \
  --docker-server=ghcr.io \
  --docker-username=<github-user> \
  --docker-password=<github-pat> \
  -n <namespace>

E2E_NAMESPACE=<namespace> \
E2E_IMAGE_REPOSITORY_PREFIX=ghcr.io/underpass-ai \
E2E_IMAGE_PULL_SECRET=ghcr-pull \
E2E_IMAGE_TAG=dev-$(git rev-parse --short HEAD) \
make e2e-kubernetes
```

If `kind` is installed the script still supports `kind load
docker-image`; otherwise it can fall back to a cluster-local
registry, but that path is cluster-specific and not the default
operator story.

### Provider-E2E (vLLM)

Exercises the `agent-vllm` adapter directly against a real vLLM
endpoint — not the full MADE — so it pins the provider
wire contract. The Kubernetes Job mounts a client certificate and
hits the endpoint via mTLS.

```bash
# Build the runner image and push it where the cluster can pull it.
IMAGE_TAG=<registry>/made-e2e-provider:dev \
    make build-provider-image

# Edit tests/e2e/kubernetes/provider-vllm-job.yaml to use that tag,
# then:
NAMESPACE=<ns-holding-e2e-client-tls> make e2e-provider-vllm
```

Env vars consumed by the runner (set in the Job's `env` block,
edit the manifest for a different endpoint):

| Var | Required | Notes |
|---|---|---|
| `MADE_VLLM_ENDPOINT` | yes | base URL, e.g. `https://llm.underpassai.com` |
| `MADE_VLLM_MODEL` | yes | model id, e.g. `google/gemma-4-31B-it` |
| `MADE_VLLM_CLIENT_CERT_PATH` | with key | PEM file for mTLS client cert |
| `MADE_VLLM_CLIENT_KEY_PATH` | with cert | PEM file for mTLS client key |
| `MADE_VLLM_BEARER_TOKEN` | optional | bearer auth |
| `MADE_VLLM_MAX_TOKENS` | optional | default `1024`; bump when the model uses a reasoning parser that burns budget before `content` |

The runner validates `generate` + `critique` + `revise` each
return text of at least 20 characters; a failing assertion exits
the Job non-zero and the script surfaces the pod status.

## Benchmarks

```bash
just bench-compile           # keep criterion benches compiling (CI gate)
just bench-trace             # TraceContext parse / format / generate
just bench-deliberate        # DeliberateUseCase end-to-end
just bench-experiment-001    # reproduce docs/experiments/001
```

Criterion is compile-gated on every PR but not run — the signal-to-
noise is wrong for a per-PR check. Record numbers under
`docs/experiments/NNN-*/results/` when running intentionally (see
[`docs/experiments/README.md`](experiments/README.md) for the
lab-notebook contract).

## Running the binary

The smallest local run does not need NATS, Postgres, Runtime, KMP,
PIR, or provider credentials. It uses in-memory persistence, noop
messaging, and the default noop executor:

```bash
# With no external services.
MADE_NATS_ENABLED=false just run

# Same command without just.
MADE_NATS_ENABLED=false cargo run --locked -p made

# Optional: seed a demo council so ListCouncils/Deliberate are
# immediately exercisable after boot.
MADE_NATS_ENABLED=false MADE_SEED_SPECIALTIES=triage just run

# Same seeded run without just.
MADE_NATS_ENABLED=false MADE_SEED_SPECIALTIES=triage \
  cargo run --locked -p made

# With defaults, the binary expects NATS at nats://nats:4222 and uses
# in-memory persistence unless MADE_POSTGRES_URL is set.
just run

# With the OTLP exporter compiled in. At runtime, set
# MADE_OTLP_ENDPOINT to actually ship spans somewhere.
MADE_OTLP_ENDPOINT=http://localhost:4317 just run-otel
```

Full configuration surface — see the table in
[`crates/made-adapters/src/config.rs`](../crates/made-adapters/src/config.rs)
and
[`charts/made/values.yaml`](../charts/made/values.yaml).

## Running the MCP adapter

Fixture mode is the quickest MCP client-wiring path because it needs no
running MADE:

```bash
# Installed binary.
MADE_MCP_BACKEND=fixture made-mcp

# Same mode from a checkout.
MADE_MCP_BACKEND=fixture cargo run -p made-mcp --locked

# One-shot terminal smoke.
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Live mode points at a running MADE:

```bash
# Installed binary against the local server from `just run`.
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 made-mcp

# Same mode from a checkout.
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
  cargo run -p made-mcp --locked

# One-shot smoke against the local server.
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
MADE_MCP_BIN=target/debug/made-mcp \
  bash scripts/mcp/made-stdio-smoke.sh
```

The canonical end-user guide is
[`docs/operations/mcp-stdio.md`](operations/mcp-stdio.md).

For MCP parity against the real multi-agent vLLM council ceremony,
point MCP at a MADE instance that has `kind=vllm` enabled and provide
the provider endpoint/model:

```bash
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
MADE_VLLM_ENDPOINT=https://vllm.example.com \
MADE_VLLM_MODEL=google/gemma-4-31B-it \
  make e2e-mcp-council-vllm
```

This path intentionally enters through `made-mcp` stdio. Use
`make e2e-council-vllm` when validating the direct gRPC runner instead.

## Adding a new port

1. Define the trait in `made-core/src/ports/<name>.rs` and
   re-export from `ports/mod.rs`. Only domain types; no IO, no
   vendor vocabulary.
2. Add adapter implementations under
   `made-adapters/src/{memory,nats,postgres,grpc,…}/`.
3. Wire the adapter through `made/src/compose.rs` (typically in
   `wire_persistence` / `wire_messaging` or next to them).
4. Add unit tests with an in-process stub.
5. Add an integration test when the adapter has external
   behaviour (e.g. Postgres schema, NATS subjects, gRPC wire
   format). See
   [`crates/made-tests-integration/tests/`](../crates/made-tests-integration/tests/)
   for shape.

## Adding a new use case

1. Create `made-app/src/usecases/<name>.rs` exposing a struct
   with constructor-injected ports + an `async fn execute`.
2. Add `#[tracing::instrument(name = "...", skip_all,
   fields(...))]` on `execute` with the domain fields operators
   will query by.
3. Re-export from `usecases/mod.rs`.
4. Thread through `made/src/compose.rs`.
5. If the use case exposes a gRPC surface, wire the handler in
   `made-adapters/src/grpc/service.rs` and call
   `link_span_to_metadata(&request)` at the top of the handler
   body so W3C tracecontext propagation keeps working.

## Adding a new provider adapter

Provider adapters (LLMs, rule engines, humans-in-the-loop) live
behind their own Cargo feature in `made-adapters/Cargo.toml`. See
`agent-anthropic`, `agent-openai`, `agent-vllm` for the pattern. No
provider is privileged — every one is a peer behind its flag.

## Release

See [`docs/release.md`](release.md).

## What the CI gates actually check

| Gate | Command | Runs on |
|---|---|---|
| `dev-lint` | `bash scripts/ci/dev-loop.sh lint` | draft PRs |
| `dev-test` | `bash scripts/ci/dev-loop.sh test` | draft PRs |
| `dev-architecture` | `bash scripts/ci/dev-loop.sh gates` | draft PRs |
| `dev-binary` | `cargo build --release -p made-mcp --no-default-features --features embedded` | draft PRs |
| `rustfmt` | `cargo fmt --all -- --check` | every ready PR |
| `contract` | proto + AsyncAPI validation + blocking proto breaking check | every ready PR |
| `clippy` | `cargo clippy` with `-D warnings` on full provider matrix | every ready PR |
| `test` | `cargo test` on full provider matrix | every ready PR |
| `benches-compile` | `cargo bench --workspace --no-run` | every ready PR |
| `integration-nats` | testcontainers NATS tests | every ready PR |
| `integration-postgres` | testcontainers Postgres tests | every ready PR |
| `helm-chart` | `helm lint` + hardened-render assertions | every ready PR |
| `container-image` | image builds from `Dockerfile` | every ready PR |
| `dependency-review` | GitHub dependency-review-action | every ready PR |
| `coverage` | `bash scripts/ci/rust-coverage.sh` (80 % workspace) | every ready PR |
| `tree-proof` | `bash scripts/ci/tree-already-proved.sh --self-test`, then the proof itself | self-test every run; the proof on pushes to `main` |
| `impact` | `python3 scripts/ci/quality-gate-plan.py` | every ready PR |
| `gate` | every required job passed and this is not a draft | every ready PR |
| `e2e-compose` | full stack via docker compose + runner | **manual** |
| `e2e-kubernetes` | kubernetes + chart + runner Job | **manual** |

"Every ready PR" is literal twice over: while the pull request is a draft
those rows do not run at all and the four `dev-*` rows answer instead, and
once it is ready the `impact` planner still routes out the rows the change
cannot affect. E2E stays
outside CI entirely — the per-PR gates already cover the compile-and-unit
surface, and E2E is reserved for manual pre-release validation via
`make`.
