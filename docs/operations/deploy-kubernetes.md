# Deploy on Kubernetes

The `made` service exposes gRPC on 50055 and HTTP health/metrics on 8080 by
default. Its Helm chart supports a standalone deployment, optional NATS,
shared Postgres persistence, providers and an optional Runtime executor.
None of those integrations makes KMP a dependency.

## Choose a profile

Read [values.yaml](../../charts/made/values.yaml) for all chart settings.
The shipped overlay files define usable starting points:

| Overlay | Purpose |
|:--|:--|
| `values.minimal.yaml` | Service smoke without NATS, Postgres, Runtime or provider credentials |
| `values.embedded-nats.yaml` | Release-local NATS |
| `values.postgres-secret.yaml` | Postgres DSN from an existing Secret |
| `values.provider-env-secrets.yaml` | Provider environment from an existing Secret |
| `values.underpass-runtime.yaml` | Optional external Runtime executor integration |

Minimal/no-op profiles test wiring. Configure real agents/providers and a real
executor before claiming external work. Provider features must be compiled in;
runtime credentials/configuration and registered agent kinds must also agree.
The default Dockerfile includes OpenAI and vLLM adapters; Anthropic is a
separate build feature. Keep credentials in Secrets, not agent descriptors.

## Render and install

Select an actually published image digest. The chart requires a tag or digest,
prefers `image.digest`, and rejects `latest` unless the explicit development
escape hatch is enabled. From a checked-out release:

```bash
export IMAGE_DIGEST=sha256:REPLACE_WITH_PUBLISHED_DIGEST
helm template made charts/made \
  -f charts/made/values.minimal.yaml \
  --set image.digest="$IMAGE_DIGEST" > made-rendered.yaml

helm upgrade --install made charts/made \
  --namespace made-system --create-namespace \
  -f charts/made/values.minimal.yaml \
  --set image.digest="$IMAGE_DIGEST" \
  --wait --timeout 10m --atomic
```

Inspect the rendered resources before installation. For a published chart,
replace `charts/made` with `oci://ghcr.io/underpass-ai/charts/made` and add
`--version X.Y.Z` matching a successfully published release. Chart version,
app version and binary version move together. The chart declares its
Kubernetes floor in [Chart.yaml](../../charts/made/Chart.yaml).

## Configure durability explicitly

`persistence.ceremonies.enabled=true` mounts a SQLite ceremony store on a PVC.
The chart enforces one replica for that posture. It does not offer a clustered
SQLite service or automatic failover. Published definitions, sealed events,
snapshots and session memory belong to this store.

`persistence.postgres.enabled=true` selects one shared backend for ceremony
events, snapshots, publications, feed cursors, session memory, execution
receipts, councils, deliberations, agents, statistics and artifacts. Artifact
metadata and bounded chunks share the database transaction boundary, so
replicas can resume the same upload. Supply the DSN through `urlFromSecret`;
migrations run at startup. Do not also enable either local persistence mode.

Set `replicaCount` above one only with Postgres enabled. The chart rejects
multi-replica in-memory or SQLite configurations, rejects mixed SQLite and
Postgres ceremony stores, and keeps local artifact storage at one replica.
Postgres recovery reads the journals and durable cursors directly; NATS may be
disabled or lose a notification without losing accepted work.

`persistence.artifacts.enabled=true` instead mounts the local artifact store
and sets `MADE_ARTIFACT_STORE_PATH`. The chart restricts this mode to one
replica and refuses combining it with Postgres. Back up its whole directory,
including manifests and tombstones, and verify restore before depending on it.
Backup and restore are host operations and are deliberately absent from the
remote gRPC and MCP surfaces. Back up each configured store consistently
before upgrading.

## Transport and access

Use `tls.mode=server` or `mutual` when crossing an untrusted boundary, with
`tls.existingSecret` supplying `tls.crt`, `tls.key` and, for mutual TLS,
`ca.crt`. Match the MCP client's TLS settings. Runtime client TLS is separate
from the service's inbound gRPC TLS.

The default container runs without root, with a read-only root filesystem,
capabilities dropped and no service-account token. Keep those defaults.
Enable and tailor NetworkPolicy where the cluster supports it; provider,
Runtime and observability egress must match the selected integrations.

NATS uses core pub/sub. Broker-side JetStream configuration alone does not
supply replay or acknowledged delivery to the existing NATS adapter. Use the
ceremony feed/cursor APIs where durable consumption is required.

## Verify and upgrade

```bash
kubectl -n made-system rollout status deployment/made
kubectl -n made-system port-forward service/made 8080:8080
```

From another terminal:

```bash
curl -fsS http://127.0.0.1:8080/healthz
curl -fsS http://127.0.0.1:8080/readyz
```

Then port-forward 50055 and run the [consumer smoke](consumer-smoke.md).
Check the deployed digest, active provider kinds and persistence settings.
A rollout plus health response establishes process readiness, not model
quality or successful execution of a real ceremony.

Upgrade by rendering the new pinned version, checking migrations and repeating
`helm upgrade --install ... --atomic`. Inspect `helm history made -n
made-system` before selecting a rollback revision. Helm rollback restores
Kubernetes resources, not external databases, Secrets or provider state.
Older readers may not understand newer event schemas; use the
[migration guide](../migrations/README.md) before reverting binaries.
