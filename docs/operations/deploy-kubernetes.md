# Deploy on Kubernetes

The `made` service exposes gRPC on 50055 and HTTP health/metrics on 8080 by
default. Its Helm chart supports a standalone deployment, optional NATS,
shared Postgres persistence, providers and an optional Runtime executor.
The public gRPC service always requires mutual TLS, an explicit certificate
principal map and a durable authorization policy opened before startup.
None of those integrations makes KMP a dependency.

## Choose a profile

Read [values.yaml](../../charts/made/values.yaml) for all chart settings.
The shipped overlay files define usable starting points:

| Overlay | Purpose |
|:--|:--|
| `values.minimal.yaml` | Single-replica service smoke without NATS, Postgres or Runtime; auth Secrets remain required |
| `values.embedded-nats.yaml` | Release-local NATS |
| `values.postgres-secret.yaml` | Three-replica Postgres profile with DSN, mTLS, principal-map and cursor-key Secret references |
| `values.provider-env-secrets.yaml` | Provider environment from an existing Secret |
| `values.underpass-runtime.yaml` | Optional external Runtime executor integration |

Minimal/no-op profiles test wiring. Configure real agents/providers and a real
executor before claiming external work. Provider features must be compiled in;
runtime credentials/configuration and registered agent kinds must also agree.
The default Dockerfile includes OpenAI and vLLM adapters; Anthropic is a
separate build feature. Keep credentials in Secrets, not agent descriptors.

## Render the candidate

Select an actually published image digest. The chart requires a tag or digest,
prefers `image.digest`, and rejects `latest` unless the explicit development
escape hatch is enabled. From a checked-out release:

```bash
export IMAGE_DIGEST=sha256:REPLACE_WITH_PUBLISHED_DIGEST
helm template made charts/made \
  -f charts/made/values.minimal.yaml \
  --set image.digest="$IMAGE_DIGEST" > made-rendered.yaml
```

Inspect the rendered resources before installation, then bootstrap the policy
as described below. For a published chart,
replace `charts/made` with `oci://ghcr.io/underpass-ai/charts/made` and add
`--version X.Y.Z` matching a successfully published release. Chart version,
app version and binary version move together. The chart declares its
Kubernetes floor in [Chart.yaml](../../charts/made/Chart.yaml).

## Establish identity and bootstrap the policy

The chart never generates a CA, client identity, principal map, policy owner or
business grant. Create these through the cluster's normal PKI and secret
management path. `tls.existingSecret` must contain `tls.crt`, `tls.key` and
`ca.crt`. `authorization.principals.existingSecret` must contain the key named
by `authorization.principals.key` (default `principals.json`). The map binds the
SHA-256 digest of each client certificate's DER bytes to one typed principal:

```json
[
  {
    "certificate_sha256": "REPLACE_WITH_64_LOWERCASE_HEX",
    "principal_id": "made-platform-admin",
    "principal_kind": "trusted_host"
  }
]
```

Compute the fingerprint from the public client certificate without copying its
private key:

```bash
openssl x509 -in admin-client.crt -outform DER | sha256sum
kubectl -n made-system create secret generic made-auth-principals \
  --from-file=principals.json
kubectl -n made-system create secret generic made-cursor-hmac \
  --from-file=key=/secure/path/to/cursor-hmac-key
```

Create the TLS Secret through the same PKI workflow. Do not commit the rendered
Secret, private key, Postgres DSN or cursor HMAC key. The owner id passed to
bootstrap must exactly match a `trusted_host` entry in the principal map; the
runtime will authenticate that owner with `MutualTls`.

For Postgres, run the pinned MADE image once before installing or starting the
Deployment. This Job uses the same DSN Secret as the service and opens only an
absent policy. An exact rerun returns `existing: true`; a different owner is a
conflict.

Save the environment-specific references in `made-production-values.yaml`:

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: made-authorization-bootstrap
  namespace: made-system
spec:
  backoffLimit: 3
  template:
    spec:
      restartPolicy: Never
      automountServiceAccountToken: false
      securityContext:
        runAsNonRoot: true
        runAsUser: 65532
        runAsGroup: 65532
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: bootstrap
          image: ghcr.io/underpass-ai/made@sha256:REPLACE_WITH_PUBLISHED_DIGEST
          args:
            - bootstrap-authorization
            - --policy-id
            - made-production-policy
            - --trusted-host-id
            - made-platform-admin
          env:
            - name: MADE_POSTGRES_URL
              valueFrom:
                secretKeyRef:
                  name: made-postgres-dsn
                  key: url
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
```

Apply and wait for the Job, inspect its JSON receipt, then install the chart
with the same policy id and Postgres Secret. After the service is reachable,
use the owner certificate and the public authorization API to issue only the
business actions and scopes each client needs. Bootstrap does not create an
allow-all grant.

```yaml
authorization:
  policyId: made-production-policy
  principals:
    existingSecret: made-auth-principals
    key: principals.json
tls:
  mode: mutual
  existingSecret: made-grpc-mtls
ceremonySearch:
  storeId: made-production
  cursorHmacKeyFromSecret:
    name: made-cursor-hmac
    key: key
```

```bash
helm upgrade --install made charts/made \
  --namespace made-system --create-namespace \
  -f charts/made/values.postgres-secret.yaml \
  -f made-production-values.yaml \
  --set image.digest="$IMAGE_DIGEST" \
  --wait --timeout 10m --atomic
```

For local SQLite, run the same `bootstrap-authorization` command against
`MADE_CEREMONY_STORE_PATH` on the exact database that the single pod will
mount. A two-stage install can create the PVC with `replicaCount=0`, run a
one-shot Job that mounts `<release>-ceremonies` at the configured path, and
then upgrade to one replica. Do not start the service before that Job succeeds.
SQLite remains a one-process posture.

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
It also requires one shared `ceremonySearch.storeId` and one HMAC key sourced
from a Secret. Every replica receives the same values, so a cursor minted by
one pod verifies on another without exposing the key in the Deployment.
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

The chart accepts only `tls.mode=mutual` for the public service. The referenced
Secret supplies `tls.crt`, `tls.key` and `ca.crt`; its files are mounted
read-only for the pod's non-root group. Match the MCP or gRPC client's TLS
settings and keep its certificate in the principal map. Runtime client TLS is
separate from the service's inbound gRPC TLS.

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
