# Deploying MADE to Kubernetes

The chart at `charts/made/` supports four checked-in install
profiles:

- `charts/made/values.minimal.yaml` - standalone install,
  no external dependencies, noop executor, in-memory persistence, NATS
  disabled, and plaintext in-cluster gRPC.
- `charts/made/values.embedded-nats.yaml` - standalone
  install with a release-local NATS bus, noop executor, and in-memory
  persistence.
- `charts/made/values.postgres-secret.yaml` - standalone
  install with embedded NATS and Postgres persistence sourced from a
  Kubernetes secret.
- `charts/made/values.underpass-runtime.yaml` - deployment
  profile for the `underpass-runtime` namespace with a release-local
  NATS and the Runtime executor.

The wrapper script `scripts/ci/deploy-kubernetes.sh` runs
`helm upgrade --install` with pinned image enforcement and the same
defaults CI uses.

## Minimal standalone install

Use this path when you want the smallest Kubernetes install that
proves the product surface starts and accepts gRPC calls. It does not
need KMP, PIR, Runtime, Postgres, NATS, provider credentials, TLS
secrets, or a NetworkPolicy-capable CNI.

The profile seeds a `triage` NoopAgent council so the deployment is
immediately smoke-testable.

### Prerequisites

1. A Kubernetes cluster with Helm 3.
2. An image reference for MADE. Prefer a digest:
   `IMAGE_DIGEST=sha256:REPLACE_ME`. Use `IMAGE_TAG=sha-COMMIT` only
   when that tag is immutable in your registry.
3. If the image is private, an image pull secret in the namespace,
   for example `ghcr-pull`.

### Install from the checkout

```bash
NAMESPACE=made-system \
RELEASE_NAME=MADE \
IMAGE_DIGEST=sha256:REPLACE_ME \
VALUES_FILE=charts/made/values.minimal.yaml \
./scripts/ci/deploy-kubernetes.sh
```

For a tag-based install:

```bash
NAMESPACE=made-system \
RELEASE_NAME=MADE \
IMAGE_TAG=sha-COMMIT \
VALUES_FILE=charts/made/values.minimal.yaml \
./scripts/ci/deploy-kubernetes.sh
```

With a private registry pull secret:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.minimal.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set imagePullSecrets[0].name=ghcr-pull \
  --wait --timeout 10m --atomic
```

For local `kind` or `k3d` development, build/load the image into the
cluster and use a local tag only with the explicit development escape
hatch:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.minimal.yaml \
  --set image.repository=made \
  --set image.tag=dev \
  --set development.allowMutableImageTags=true \
  --wait --timeout 10m --atomic
```

### Minimal smoke

```bash
kubectl -n made-system rollout status deploy/made

kubectl -n made-system port-forward svc/made 8080:8080 &
curl -fsS localhost:8080/healthz
curl -fsS localhost:8080/readyz
```

With `values.minimal.yaml`, readiness reports NATS as healthy but
`not wired (noop messaging)`, because NATS is intentionally disabled.
That is expected for the minimal profile.

To smoke the gRPC surface from this repository:

```bash
kubectl -n made-system port-forward svc/made 50055:50055 &
cargo run -p made-consumer-smoke --locked -- \
  --endpoint http://127.0.0.1:50055 \
  --chain all
```

The default `--chain all` run registers the Report contract, proves
Strict-mode rejection against the seeded NoopAgent, then proves
Warn-mode `RunCouncilDecision` returns a winner. NATS assertions are
reported as `Skipped` because no bus is configured in this profile.

### Render check

Before applying changes, operators can render the minimal manifest
without touching a cluster:

```bash
helm template MADE charts/made \
  -f charts/made/values.minimal.yaml \
  --set image.tag=sha-COMMIT
```

The chart refuses to render if neither `image.tag` nor `image.digest`
is set. It also refuses `image.tag=latest` unless
`development.allowMutableImageTags=true` is set for a local
development install.

## Standalone install with embedded NATS

Use `values.embedded-nats.yaml` when you want MADE's
event surface enabled but do not want to operate a separate NATS
release. This keeps the install independent from KMP, PIR, Runtime,
Postgres, provider credentials, and gRPC TLS while still exercising:

- outbound `made.task.*`, `made.phase.changed`, and
  `made.deliberation.completed` events;
- inbound triggers on `made.trigger.>`;
- readiness checking of the NATS connection;
- consumer-smoke assertions for `correlation_id` and `causation_id`
  propagation.

### Install

```bash
NAMESPACE=made-system \
RELEASE_NAME=MADE \
IMAGE_DIGEST=sha256:REPLACE_ME \
VALUES_FILE=charts/made/values.embedded-nats.yaml \
./scripts/ci/deploy-kubernetes.sh
```

For local `kind` or `k3d` development:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.repository=made \
  --set image.tag=dev \
  --set development.allowMutableImageTags=true \
  --wait --timeout 10m --atomic
```

With release name `MADE`, the embedded bus is exposed as the
ClusterIP Service `made-nats` and the application receives:

```text
MADE_NATS_ENABLED=true
MADE_NATS_URL=nats://made-nats:4222
MADE_TRIGGER_SUBJECT=made.trigger.>
MADE_PUBLISH_PREFIX=made
```

For a different release name, inspect the generated Service:

```bash
kubectl -n made-system get svc \
  -l app.kubernetes.io/component=nats
```

### Smoke

```bash
kubectl -n made-system rollout status deploy/made
kubectl -n made-system rollout status deploy/made-nats

kubectl -n made-system port-forward svc/made 8080:8080 &
curl -fsS localhost:8080/readyz
```

`/readyz` should report the `nats` check as healthy and include the
configured URL.

To verify the gRPC surface plus event propagation from this checkout:

```bash
kubectl -n made-system port-forward svc/made 50055:50055 &
kubectl -n made-system port-forward svc/made-nats 4222:4222 &

cargo run -p made-consumer-smoke --locked -- \
  --endpoint http://127.0.0.1:50055 \
  --nats-url nats://127.0.0.1:4222 \
  --chain all
```

Expected result:

- `chain2 report_contract_rejects_freeform_text PASS`;
- `chain1 rpc_returned_winner PASS`;
- `chain1 trigger_envelope_observed PASS`;
- `chain1 causal_metadata_propagated PASS`.

### Render check

```bash
helm template MADE charts/made \
  -f charts/made/values.embedded-nats.yaml \
  --set image.tag=sha-COMMIT
```

The rendered manifest should include both Deployments:
`MADE` and `made-nats`. With JetStream disabled, the
embedded NATS pod has no PVC; current MADE events are
fire-and-forget and do not require stream storage.

## gRPC server TLS and mTLS

The chart has two independent TLS surfaces:

- `tls.*` protects MADE's own gRPC server on port `50055`.
- `executor.runtime.tls.*` protects the outbound client connection
  from MADE to Runtime.

This section covers `tls.*`. HTTP health probes on port `8080` remain
plain in-cluster endpoints for Kubernetes liveness/readiness.

### Server TLS

Use `tls.mode=server` when clients should verify the MADE
server identity but do not need to present client certificates.

Create a secret with `tls.crt` and `tls.key`:

```bash
kubectl -n made-system create secret generic made-grpc-tls \
  --from-file=tls.crt=./server.crt \
  --from-file=tls.key=./server.key
```

Install or upgrade with:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set tls.mode=server \
  --set tls.existingSecret=made-grpc-tls \
  --wait --timeout 10m --atomic
```

The pod receives:

```text
MADE_GRPC_TLS_MODE=server
MADE_GRPC_TLS_CERT_PATH=/etc/made/tls/tls.crt
MADE_GRPC_TLS_KEY_PATH=/etc/made/tls/tls.key
```

### Mutual TLS

Use `tls.mode=mutual` when MADE must also authenticate
gRPC clients. The server secret needs the server identity plus the CA
bundle used to validate client certificates:

```bash
kubectl -n made-system create secret generic made-grpc-mtls \
  --from-file=tls.crt=./server.crt \
  --from-file=tls.key=./server.key \
  --from-file=ca.crt=./client-ca.crt
```

Install or upgrade with:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls \
  --wait --timeout 10m --atomic
```

The pod receives the same server cert/key env vars plus:

```text
MADE_GRPC_TLS_MODE=mutual
MADE_GRPC_TLS_CLIENT_CA_PATH=/etc/made/tls/ca.crt
```

The chart refuses to render `tls.mode=server` or `tls.mode=mutual`
without `tls.existingSecret`.

### Client configuration

Clients must use a TLS-capable gRPC endpoint, typically
`https://made.made-system.svc:50055`. If you use
`kubectl port-forward`, set the client TLS domain override to the
certificate SAN rather than `127.0.0.1`.

For the MCP adapter in server-TLS mode:

```bash
MADE_MCP_GRPC_ENDPOINT=https://127.0.0.1:50055 \
MADE_MCP_GRPC_TLS_MODE=server \
MADE_MCP_GRPC_TLS_CA_PATH=./server-ca.crt \
MADE_MCP_GRPC_TLS_DOMAIN_NAME=made-grpc \
made-mcp
```

For mTLS:

```bash
MADE_MCP_GRPC_ENDPOINT=https://127.0.0.1:50055 \
MADE_MCP_GRPC_TLS_MODE=mutual \
MADE_MCP_GRPC_TLS_CA_PATH=./server-ca.crt \
MADE_MCP_GRPC_TLS_CERT_PATH=./client.crt \
MADE_MCP_GRPC_TLS_KEY_PATH=./client.key \
MADE_MCP_GRPC_TLS_DOMAIN_NAME=made-grpc \
made-mcp
```

See [`operations/mcp-stdio.md`](./mcp-stdio.md) for full MCP smoke
commands. The `made-consumer-smoke` binary intentionally keeps a
narrow plain-gRPC surface today; use MCP or another TLS-capable gRPC
client for hardened transport checks.

### Render checks

```bash
helm template MADE charts/made \
  -f charts/made/values.embedded-nats.yaml \
  --set image.tag=sha-COMMIT \
  --set tls.mode=server \
  --set tls.existingSecret=made-grpc-tls

helm template MADE charts/made \
  -f charts/made/values.embedded-nats.yaml \
  --set image.tag=sha-COMMIT \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls
```

`scripts/ci/helm-lint.sh` pins both renders and the missing-secret
failure path.

## Postgres persistence from a secret

By default, MADE uses in-memory persistence. Enable Postgres
when councils, agents, deliberations, and statistics must survive pod
restarts or replica replacement.

Current scope is deliberately narrow: Postgres backs deliberations,
councils, agents, and statistics. Output contracts are still managed
by the in-memory contract registry and should be registered after
startup or seeded by the deployment workflow.

### Secret shape

Create a Kubernetes secret containing a single key, `url`, whose value
is the Postgres DSN. Prefer your normal secret manager, External
Secrets operator, SealedSecret, or SOPS flow. For a direct Kubernetes
example:

```bash
kubectl -n made-system create secret generic made-postgres-dsn \
  --from-file=url=./postgres-url.txt
```

`postgres-url.txt` should contain a DSN in this shape:

```text
postgres://USER:PASSWORD@postgresql.made-system.svc:5432/made?sslmode=require
```

The database user needs enough privileges for startup migrations to
create and alter the MADE schema. Migrations are embedded in
the binary and run on startup; if the database is unreachable or the
user lacks migration privileges, the pod fails before serving gRPC.

### Install

The checked-in profile uses embedded NATS and reads the DSN from
`made-postgres-dsn/url`:

```bash
NAMESPACE=made-system \
RELEASE_NAME=MADE \
IMAGE_DIGEST=sha256:REPLACE_ME \
VALUES_FILE=charts/made/values.postgres-secret.yaml \
./scripts/ci/deploy-kubernetes.sh
```

Equivalent direct Helm command:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set persistence.postgres.enabled=true \
  --set persistence.postgres.urlFromSecret.name=made-postgres-dsn \
  --set persistence.postgres.urlFromSecret.key=url \
  --wait --timeout 10m --atomic
```

The rendered pod receives only a secret reference, not the literal DSN:

```yaml
- name: MADE_POSTGRES_URL
  valueFrom:
    secretKeyRef:
      name: "made-postgres-dsn"
      key: "url"
```

The chart refuses to render
`persistence.postgres.enabled=true` unless either
`persistence.postgres.urlFromSecret.{name,key}` or
`persistence.postgres.url` is set. `urlFromSecret` is the recommended
production path; `url` is retained for ephemeral local testing only.

### NetworkPolicy

If `networkPolicy.enabled=true`, make sure
`networkPolicy.egress.postgres` matches the labels and port of your
Postgres Service. The chart emits the Postgres egress block only when
`persistence.postgres.enabled=true`.

### Smoke

```bash
kubectl -n made-system rollout status deploy/made
kubectl -n made-system logs deploy/made | grep 'postgres persistence wired'

kubectl -n made-system port-forward svc/made 50055:50055 &
grpcurl -plaintext \
  -import-path crates/made-proto/proto \
  -proto underpass/made/v1/made.proto \
  -d '{"specialty":"persistence-smoke","numAgents":1,"agentConfig":{"kind":"noop"}}' \
  127.0.0.1:50055 underpass.made.v1.MadeService/CreateCouncil

kubectl -n made-system rollout restart deploy/made
kubectl -n made-system rollout status deploy/made

grpcurl -plaintext \
  -import-path crates/made-proto/proto \
  -proto underpass/made/v1/made.proto \
  -d '{"includeAgents":true}' \
  127.0.0.1:50055 underpass.made.v1.MadeService/ListCouncils
```

The final `ListCouncils` response should still include
`persistence-smoke`. If the deployment is using gRPC TLS/mTLS, use
the matching `grpcurl` TLS flags or run the same create/list flow via
a configured MCP client.

## Provider environment secrets

Provider-backed agent kinds are optional. The chart can inject the
required `MADE_*` provider env vars from a Secret, but the binary
must also be built with the matching Cargo feature:

- `agent-openai` for `kind=openai`;
- `agent-vllm` for `kind=vllm`;
- `agent-anthropic` for `kind=anthropic`.

The startup log includes `agent_kinds=...`; only kinds listed there
will be accepted by `RegisterAgent`.

### Env vars

| Provider | Required env | Optional env |
|---|---|---|
| `openai` | `MADE_OPENAI_API_KEY` | `MADE_OPENAI_MODEL`, `MADE_OPENAI_ENDPOINT`, `MADE_OPENAI_MAX_TOKENS` |
| `vllm` | `MADE_VLLM_MODEL`, `MADE_VLLM_ENDPOINT` | `MADE_VLLM_BEARER_TOKEN`, `MADE_VLLM_MAX_TOKENS`, `MADE_VLLM_TIMEOUT_SECS` |
| `anthropic` | `MADE_ANTHROPIC_API_KEY` | `MADE_ANTHROPIC_MODEL`, `MADE_ANTHROPIC_ENDPOINT`, `MADE_ANTHROPIC_MAX_TOKENS` |

`provider.model`, `provider.endpoint`, and `provider.max_tokens` can
still be supplied as per-agent descriptor attributes on
`RegisterAgent`. Credentials stay in environment only; agent
descriptors may be persisted in Postgres and must not carry secrets.

The service's vLLM factory currently loads endpoint/model/bearer from
env. The `MADE_VLLM_CLIENT_CERT_PATH` and
`MADE_VLLM_CLIENT_KEY_PATH` variables documented for the
provider-E2E runner are not part of the service factory's Helm path
yet.

### Secret shape

One Secret can expose all provider variables. Keep the env file out
of Git:

```text
MADE_OPENAI_API_KEY=REPLACE_ME
MADE_OPENAI_MODEL=gpt-4o-mini
MADE_OPENAI_ENDPOINT=https://api.openai.com
MADE_VLLM_MODEL=stub-report-vllm-v1
MADE_VLLM_ENDPOINT=http://vllm-server:8000
MADE_VLLM_BEARER_TOKEN=REPLACE_ME
MADE_ANTHROPIC_API_KEY=REPLACE_ME
MADE_ANTHROPIC_MODEL=claude-haiku-4-5-20251001
```

Create the Secret from that file:

```bash
kubectl -n made-system create secret generic made-provider-env \
  --from-env-file=./provider.env
```

Only include providers you intend to enable. Empty strings count as
unset.

### Install with envFrom

The checked-in overlay
`charts/made/values.provider-env-secrets.yaml` references
`made-provider-env` through `envFrom.secretRef`:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  -f charts/made/values.provider-env-secrets.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --wait --timeout 10m --atomic
```

Rendered shape:

```yaml
envFrom:
  - secretRef:
      name: made-provider-env
```

### Install with per-key references

For tighter ownership, reference individual keys instead of importing
the full Secret:

```yaml
providerEnv:
  - name: MADE_OPENAI_API_KEY
    valueFrom:
      secretKeyRef:
        name: made-openai
        key: api-key
  - name: MADE_OPENAI_MODEL
    value: gpt-4o-mini
  - name: MADE_OPENAI_ENDPOINT
    value: https://api.openai.com
```

Apply that values file together with the install profile:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  -f ./provider-env.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --wait --timeout 10m --atomic
```

### Smoke

First check that the provider kind is enabled at boot:

```bash
kubectl -n made-system logs deploy/made | grep 'agent_kinds='
```

Then register a provider-backed agent. This validates that the binary
was compiled with the feature and that required env vars were present;
it does not call the provider yet.

```bash
kubectl -n made-system port-forward svc/made 50055:50055 &

grpcurl -plaintext \
  -import-path crates/made-proto/proto \
  -proto underpass/made/v1/made.proto \
  -d '{"specialty":"provider-smoke","agent":{"agentId":"agent-provider-smoke-0","specialty":"provider-smoke","kind":"openai","attributes":{}}}' \
  127.0.0.1:50055 underpass.made.v1.MadeService/RegisterAgent
```

For an end-to-end provider call, run `made-consumer-smoke --chain
positive-path` against a provider or OpenAI-compatible stub that
returns the Report JSON shape.

## Enabling the Runtime executor

The Runtime executor is optional. MADE can run with
`executor.kind: noop` for local installs, consumer smoke, and
structured-output validation. Set `executor.kind: runtime` only when
you want `Orchestrate` to hand the winning proposal to an
`underpass.runtime.v1` service.

`RunCouncilDecision` does not call Runtime; it returns the validated
winner and candidate breakdown to the caller. Runtime integration is
therefore proved through `Orchestrate`, not through the consumer-smoke
`RunCouncilDecision` chains.

### Plain in-cluster Runtime

Use this for private clusters or local fixture environments where the
Runtime endpoint is plain HTTP/2:

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set executor.kind=runtime \
  --set executor.runtime.endpoint=http://underpass-runtime:50053 \
  --set executor.runtime.principal.tenantId=underpass \
  --set executor.runtime.principal.actorId=MADE \
  --set executor.runtime.principal.roles=orchestrator \
  --wait --timeout 10m --atomic
```

The rendered pod receives:

```text
MADE_EXECUTOR_KIND=runtime
MADE_RUNTIME_GRPC_ENDPOINT=http://underpass-runtime:50053
MADE_RUNTIME_PRINCIPAL_TENANT_ID=underpass
MADE_RUNTIME_PRINCIPAL_ACTOR_ID=MADE
MADE_RUNTIME_PRINCIPAL_ROLES=orchestrator
```

The chart fails render if `executor.kind=runtime` and
`executor.runtime.endpoint` is empty.

### Runtime with server TLS

When Runtime serves HTTPS and MADE only needs to verify the
server identity, create a secret containing `ca.crt` and enable
server TLS:

```bash
kubectl -n made-system create secret generic runtime-server-ca \
  --from-file=ca.crt=./runtime-ca.crt

helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set executor.kind=runtime \
  --set executor.runtime.endpoint=https://underpass-runtime:50053 \
  --set executor.runtime.principal.tenantId=underpass \
  --set executor.runtime.principal.actorId=MADE \
  --set executor.runtime.principal.roles=orchestrator \
  --set executor.runtime.tls.mode=server \
  --set executor.runtime.tls.domainName=underpass-runtime \
  --set executor.runtime.tls.existingSecret=runtime-server-ca \
  --wait --timeout 10m --atomic
```

The chart mounts that secret at `/etc/made/runtime-tls` and
sets `MADE_RUNTIME_TLS_CA_PATH=/etc/made/runtime-tls/ca.crt`.

### Runtime with mutual TLS

When Runtime requires client authentication, the secret must contain
`ca.crt`, `tls.crt`, and `tls.key`. Cert-manager users can apply
[`tests/cluster/made-runtime-client-cert.yaml`](../../tests/cluster/made-runtime-client-cert.yaml)
as an example of the expected secret shape.

```bash
helm -n made-system upgrade --install MADE \
  charts/made \
  --create-namespace \
  -f charts/made/values.embedded-nats.yaml \
  --set image.digest=sha256:REPLACE_ME \
  --set executor.kind=runtime \
  --set executor.runtime.endpoint=https://underpass-runtime:50053 \
  --set executor.runtime.principal.tenantId=underpass \
  --set executor.runtime.principal.actorId=MADE \
  --set executor.runtime.principal.roles=orchestrator \
  --set executor.runtime.tls.mode=mutual \
  --set executor.runtime.tls.domainName=underpass-runtime \
  --set executor.runtime.tls.existingSecret=made-runtime-client-tls \
  --wait --timeout 10m --atomic
```

The chart refuses to render any non-disabled Runtime TLS mode without
`executor.runtime.tls.existingSecret`.

### Runtime smoke

Basic readiness only proves that MADE booted. To prove the
Runtime executor path, run an `Orchestrate` request whose task carries
a Runtime tool name accepted by the target Runtime:

```text
task.attributes["runtime.tool_name"] = "YOUR_RUNTIME_TOOL"
```

In this repository, `make e2e-compose` proves that path against the
`stub-runtime` service with `runtime.tool_name=stub.echo`. In
Kubernetes, run the `runtime-stub` E2E scenario only when the namespace
provides that fixture or an equivalent tool contract:

```bash
sed 's/value: cluster-connectivity/value: runtime-stub/' \
  tests/e2e/kubernetes/runner-job.yaml \
  | kubectl -n made-system apply -f -
kubectl -n made-system logs -f job/made-e2e-runner
```

For a real Runtime deployment, replace the fixture tool name with a
tool that exists in the Runtime catalog and validate the returned
`execution_id` against Runtime logs or audit records.

## Underpass Runtime profile

The profile reflects the agreed Underpass topology: every plane
(KMP, Runtime, MADE) owns its **own NATS bus** — the planes
don't share subjects and there is no cross-plane NATS subscriber, so
collocating buses would couple deploys without sharing data. The
MADE's chart therefore deploys a release-local NATS via
`messaging.nats.embedded.enabled: true`.

### Prerequisites

1. **Image pull secret.** A namespace secret `ghcr-pull`
   (`kubernetes.io/dockerconfigjson`) with credentials that can pull
   from `ghcr.io/underpass-ai/`.
2. **Runtime mTLS client cert** (only when `executor.kind: runtime`).
   Apply
   [`tests/cluster/made-runtime-client-cert.yaml`](../../tests/cluster/made-runtime-client-cert.yaml)
   first. It mints a `kubernetes.io/tls` secret named
   `made-runtime-client-tls` via cert-manager, signed by the
   same CA that signs `underpass-runtime`'s server cert.
3. **Reachable runtime** — `underpass-runtime` Service must exist in
   the namespace and the chart values (or the override) must point at
   it (`executor.runtime.endpoint: https://underpass-runtime:50053`).

### Deploy

```bash
NAMESPACE=underpass-runtime \
RELEASE_NAME=MADE \
IMAGE_TAG=sha-COMMIT \
VALUES_FILE=charts/made/values.underpass-runtime.yaml \
./scripts/ci/deploy-kubernetes.sh
```

The wrapper:

- Requires `IMAGE_TAG` **or** `IMAGE_DIGEST` (mutually exclusive).
- Defaults to `--wait --atomic --timeout 10m`. `DRY_RUN=true` falls
  back to `--dry-run=server`.
- Always passes `--create-namespace`.

### Smoke

```bash
# Liveness + readiness via the HTTP sidecar port (NATS readiness
# is part of /readyz).
kubectl -n "$NAMESPACE" port-forward svc/made 8080:8080 &
curl -s localhost:8080/healthz
curl -s localhost:8080/readyz   # {"checks":[{"name":"nats","healthy":true,...}]}
```

### LLM-as-judge scoring

This profile enables the **LLM-as-judge** scorer. By default a deliberation's
winner is whichever proposal clears the most validators (uniform
pass-fraction), so among several valid proposals the winner is effectively
arbitrary. With the judge on, an LLM rates each proposal's intrinsic quality
and that score picks the winner instead.

It is configured through `providerEnv` in
[`values.underpass-runtime.yaml`](../../charts/made/values.underpass-runtime.yaml):

| Var | Meaning |
| --- | --- |
| `MADE_JUDGE_ENABLED` | `true` turns the judge on (opt-in; default off). |
| `MADE_JUDGE_THRESHOLD` | Pass/quality gate `0.0–1.0` (default `0.5`). The *score*, not the gate, drives ranking. |

The judge **reuses the vLLM endpoint/model** (`MADE_VLLM_ENDPOINT`,
`MADE_VLLM_MODEL`) and the binary fails fast when enabled without them —
so the two blocks are kept together in `providerEnv`, and the chart refuses
to render a judge-on-without-vLLM config — a `fail` guard in
`templates/deployment.yaml` — unless the vLLM vars come from a Secret via
`providerEnvFrom`.

Cost: the judge adds **one extra vLLM call per proposal**, run serially
against the single-stream in-cluster gemma — expect deliberations to take
proportionally longer. Turn it off by removing the two `MADE_JUDGE_*`
entries (the vLLM agents keep working without it).

## End-to-end against the deploy

`tests/e2e/kubernetes/runner-job.yaml` runs the `made-e2e-runner`
binary as a Job. The manifest sets
`MADE_E2E_SCENARIOS=cluster-connectivity`, so the default cluster
smoke runs only scenarios 1-4 against the real deploy:

- `ListCouncils` sees the seeded `triage` council.
- `Deliberate` returns a winner.
- `DeleteCouncil` on a missing specialty returns `deleted=false`.
- NATS trigger -> outbound `DeliberationCompleted` preserves
  `correlation_id` and `causation_id`.

Scenarios 5-9 are fixture-shaped and stay opt-in for Kubernetes:
scenario 5 expects a Runtime tool named `stub.echo`, scenarios 8-9
expect the `stub-llm` OpenAI-compatible sidecar, and scenario 6
asserts strict rejection of the NoopAgent's free-form output. Run
`runtime-stub`, `structured-output`, or `compose` only when the
namespace provides equivalent fixtures or real matching services.

```bash
kubectl -n "$NAMESPACE" apply -f tests/e2e/kubernetes/runner-job.yaml
kubectl -n "$NAMESPACE" logs -f job/made-e2e-runner
```

## Operator deploy verification

This is the release-candidate verification path for the product claim:
an operator can deploy a pinned image, use the versioned chart, wire
Secrets by reference, and run a post-deploy smoke.

### Current checkout verification

This checkout has no public `v*` tag yet, so a published OCI chart
cannot honestly be verified from the repository state alone. The local
chart/package and rendered Secret references are verifiable now.

Commands run against this checkout on 2026-05-18:

```bash
mkdir -p /tmp/made-operator-verify

helm template MADE charts/made \
  -f charts/made/values.postgres-secret.yaml \
  -f charts/made/values.provider-env-secrets.yaml \
  --set image.digest=sha256:1111111111111111111111111111111111111111111111111111111111111111 \
  > /tmp/made-operator-verify/pinned-secretrefs.yaml

helm template MADE charts/made \
  -f charts/made/values.postgres-secret.yaml \
  -f charts/made/values.provider-env-secrets.yaml \
  --set image.digest=sha256:1111111111111111111111111111111111111111111111111111111111111111 \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls \
  > /tmp/made-operator-verify/pinned-secretrefs-mtls.yaml

helm package charts/made \
  --destination /tmp/made-operator-verify
```

Expected rendered markers:

- image reference uses
  `ghcr.io/underpass-ai/made@sha256:...`;
- `MADE_POSTGRES_URL` is sourced from
  `secretKeyRef.name=made-postgres-dsn`;
- provider env is sourced from
  `envFrom.secretRef.name=made-provider-env`;
- mTLS render mounts `secretName: "made-grpc-mtls"` and sets
  `MADE_GRPC_TLS_MODE=mutual`;
- `helm package` produces `made-0.1.0.tgz` from the current
  chart metadata.

### Post-release OCI verification

After `docs/release.md` has cut a `vX.Y.Z` tag and
`publish-distribution.yml` has pushed the chart, verify the exact OCI
chart version before installing:

```bash
export NAMESPACE=made-system
export RELEASE_NAME=MADE
export CHART_VERSION=0.2.0
export IMAGE_DIGEST=sha256:REPLACE_ME

helm show chart \
  oci://ghcr.io/underpass-ai/charts/made \
  --version "$CHART_VERSION"

helm template "$RELEASE_NAME" \
  oci://ghcr.io/underpass-ai/charts/made \
  --version "$CHART_VERSION" \
  -f charts/made/values.postgres-secret.yaml \
  -f charts/made/values.provider-env-secrets.yaml \
  --set image.digest="$IMAGE_DIGEST" \
  > /tmp/made-oci-render.yaml
```

Confirm required Secrets exist before install:

```bash
kubectl -n "$NAMESPACE" get secret made-postgres-dsn
kubectl -n "$NAMESPACE" get secret made-provider-env
```

Install or upgrade from the OCI chart:

```bash
helm -n "$NAMESPACE" upgrade --install "$RELEASE_NAME" \
  oci://ghcr.io/underpass-ai/charts/made \
  --version "$CHART_VERSION" \
  --create-namespace \
  -f charts/made/values.postgres-secret.yaml \
  -f charts/made/values.provider-env-secrets.yaml \
  --set image.digest="$IMAGE_DIGEST" \
  --wait --timeout 10m --atomic
```

Post-deploy smoke:

```bash
kubectl -n "$NAMESPACE" rollout status deploy/made

kubectl -n "$NAMESPACE" get deploy MADE \
  -o jsonpath='{.spec.template.spec.containers[0].image}{"\n"}'

kubectl -n "$NAMESPACE" port-forward svc/made 8080:8080 &
curl -fsS localhost:8080/healthz
curl -fsS localhost:8080/readyz

kubectl -n "$NAMESPACE" port-forward svc/made 50055:50055 &
kubectl -n "$NAMESPACE" port-forward svc/made-nats 4222:4222 &

cargo run -p made-consumer-smoke --locked -- \
  --endpoint http://127.0.0.1:50055 \
  --nats-url nats://127.0.0.1:4222 \
  --chain all
```

## Upgrading

Use upgrades for a new pinned image digest, chart version, values
profile, or Secret wiring change. Keep `--atomic` on so a failed
upgrade returns the release to the previous revision automatically.

Concrete example: upgrade release `MADE` in namespace
`made-system` to a new image digest with the embedded NATS
profile.

1. Set the target inputs:

   ```bash
   export NAMESPACE=made-system
   export RELEASE_NAME=MADE
   export VALUES_FILE=charts/made/values.embedded-nats.yaml
   export IMAGE_DIGEST=sha256:REPLACE_ME
   ```

2. Render before touching the cluster:

   ```bash
   helm template "$RELEASE_NAME" charts/made \
     -f "$VALUES_FILE" \
     --set image.digest="$IMAGE_DIGEST" \
     > /tmp/made-upgrade.yaml
   ```

3. Upgrade from the checkout:

   ```bash
   ./scripts/ci/deploy-kubernetes.sh
   ```

   Equivalent direct Helm command:

   ```bash
   helm -n "$NAMESPACE" upgrade --install "$RELEASE_NAME" \
     charts/made \
     --create-namespace \
     -f "$VALUES_FILE" \
     --set image.digest="$IMAGE_DIGEST" \
     --wait --timeout 10m --atomic
   ```

4. Upgrade from a published OCI chart after a release:

   ```bash
   export CHART_VERSION=0.2.0

   helm -n "$NAMESPACE" upgrade --install "$RELEASE_NAME" \
     oci://ghcr.io/underpass-ai/charts/made \
     --version "$CHART_VERSION" \
     --create-namespace \
     -f "$VALUES_FILE" \
     --set image.digest="$IMAGE_DIGEST" \
     --wait --timeout 10m --atomic
   ```

   Use the exact chart version published by the release workflow. Do
   not use an unversioned chart reference.

5. Verify rollout and pinned image:

   ```bash
   helm -n "$NAMESPACE" status "$RELEASE_NAME"
   kubectl -n "$NAMESPACE" rollout status deploy/made

   kubectl -n "$NAMESPACE" get deploy MADE \
     -o jsonpath='{.spec.template.spec.containers[0].image}{"\n"}'
   ```

6. Run post-upgrade smoke:

   ```bash
   kubectl -n "$NAMESPACE" port-forward svc/made 8080:8080 &
   curl -fsS localhost:8080/healthz
   curl -fsS localhost:8080/readyz

   kubectl -n "$NAMESPACE" port-forward svc/made 50055:50055 &
   kubectl -n "$NAMESPACE" port-forward svc/made-nats 4222:4222 &

   cargo run -p made-consumer-smoke --locked -- \
     --endpoint http://127.0.0.1:50055 \
     --nats-url nats://127.0.0.1:4222 \
     --chain all
   ```

7. Record the new revision:

   ```bash
   helm -n "$NAMESPACE" history "$RELEASE_NAME"
   ```

Upgrade notes:

- If the upgrade changes Postgres, provider, pull, or TLS secrets,
  create or rotate those Secrets before running Helm:

  ```bash
  kubectl -n "$NAMESPACE" get secret made-postgres-dsn
  kubectl -n "$NAMESPACE" get secret made-provider-env
  kubectl -n "$NAMESPACE" get secret made-grpc-mtls
  ```

- If the upgrade changes database schema, take a Postgres backup first
  and review rollback compatibility.
- If the upgrade changes `tls.mode`, update MCP, consumer, and grpcurl
  clients to the matching TLS posture before switching traffic.
- If the upgrade changes `executor.kind=runtime`, confirm Runtime
  endpoint, Runtime TLS Secret, and Runtime tool catalog first.

## Rolling back

Use Helm rollback when a previously successful release must be restored
after a bad image, chart value, or config rollout. Failed upgrades run
through this guide use `--atomic`, so Helm already rolls back before
returning non-zero; this section is for manual rollback after a release
was accepted.

Concrete example: release `MADE` in namespace
`made-system`, current revision `7`, previous good revision
`6`.

1. Inspect release history:

   ```bash
   helm -n made-system history MADE
   ```

   Example shape:

   ```text
   REVISION  UPDATED                  STATUS      CHART                APP VERSION  DESCRIPTION
   5         2026-05-18 09:10:42      superseded  made-0.1.0  0.1.0        Upgrade complete
   6         2026-05-18 10:42:18      superseded  made-0.1.0  0.1.0        Upgrade complete
   7         2026-05-18 11:03:57      deployed    made-0.1.0  0.1.0        Upgrade complete
   ```

2. Roll back to the known-good revision:

   ```bash
   helm -n made-system rollback MADE 6 \
     --wait --timeout 10m
   ```

3. Confirm rollout and restored image reference:

   ```bash
   kubectl -n made-system rollout status deploy/made

   kubectl -n made-system get deploy MADE \
     -o jsonpath='{.spec.template.spec.containers[0].image}{"\n"}'
   ```

4. Run health and gRPC smoke:

   ```bash
   kubectl -n made-system port-forward svc/made 8080:8080 &
   curl -fsS localhost:8080/healthz
   curl -fsS localhost:8080/readyz

   kubectl -n made-system port-forward svc/made 50055:50055 &
   cargo run -p made-consumer-smoke --locked -- \
     --endpoint http://127.0.0.1:50055 \
     --chain all
   ```

   If NATS is intentionally disabled, consumer smoke reports bus checks
   as skipped. For embedded or external NATS, add:

   ```bash
   kubectl -n made-system port-forward svc/made-nats 4222:4222 &
   cargo run -p made-consumer-smoke --locked -- \
     --endpoint http://127.0.0.1:50055 \
     --nats-url nats://127.0.0.1:4222 \
     --chain all
   ```

5. Record the new deployed revision:

   ```bash
   helm -n made-system status MADE
   helm -n made-system history MADE
   ```

Notes:

- Helm rollback restores the rendered Kubernetes objects owned by the
  release, including the previous image tag or digest.
- Helm does not restore external Secrets, external Postgres state,
  external NATS state, or provider-side configuration. Restore or rotate
  those separately if the failed release changed them.
- Startup migrations run on boot. Before rolling back across a version
  that changed database schema, verify the migration is backward
  compatible or restore from a database backup.

## Topology recap

```
+---------------------------------------------+
|                underpass-runtime ns         |
|                                             |
|   MADE  <----- gRPC mTLS ---->  underpass-runtime
|   |                                          |
|   | NATS                                    | NATS
|   v                                          v
|   made-nats                  underpass-runtime-nats
|                                             |
|   rehydration-kernel  <-- gRPC -->  rehydration-kernel-nats
|                                             |
+---------------------------------------------+
```

Each plane owns its NATS. No subject collisions exist across the
three planes; integration is gRPC (point-to-point) where it matters.
