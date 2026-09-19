#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/made}"
DEV_VALUES="${CHART_PATH}/values.dev.yaml"
MINIMAL_VALUES="${CHART_PATH}/values.minimal.yaml"
EMBEDDED_NATS_VALUES="${CHART_PATH}/values.embedded-nats.yaml"
POSTGRES_SECRET_VALUES="${CHART_PATH}/values.postgres-secret.yaml"
PROVIDER_ENV_VALUES="${CHART_PATH}/values.provider-env-secrets.yaml"
UNDERPASS_RUNTIME_VALUES="${CHART_PATH}/values.underpass-runtime.yaml"
TMP_DIR="${TMPDIR:-/tmp}"
DEFAULT_ERR="${TMP_DIR}/made-helm-default.err"
HARDENED_OUT="${TMP_DIR}/made-helm-hardened.yaml"
HARDENED_ERR="${TMP_DIR}/made-helm-hardened.err"
AUTH_ARGS=(
  --set tls.mode=mutual
  --set tls.existingSecret=made-test-grpc-mtls
  --set authorization.policyId=made-test-policy
  --set authorization.principals.existingSecret=made-test-auth-principals
  --set ceremonySearch.storeId=made-test-store
  --set ceremonySearch.cursorHmacKeyFromSecret.name=made-test-cursor-hmac
  --set ceremonySearch.cursorHmacKeyFromSecret.key=key
)
LOCAL_STORE_ARGS=(--set persistence.ceremonies.enabled=true)

helm lint "${CHART_PATH}" -f "${DEV_VALUES}"
helm lint "${CHART_PATH}" -f "${MINIMAL_VALUES}" --set image.tag=v0
helm lint "${CHART_PATH}" -f "${EMBEDDED_NATS_VALUES}" --set image.tag=v0
helm lint "${CHART_PATH}" -f "${POSTGRES_SECRET_VALUES}" --set image.tag=v0
helm lint "${CHART_PATH}" -f "${EMBEDDED_NATS_VALUES}" -f "${PROVIDER_ENV_VALUES}" --set image.tag=v0
helm lint "${CHART_PATH}" -f "${UNDERPASS_RUNTIME_VALUES}" --set image.tag=v0
helm template made "${CHART_PATH}" -f "${DEV_VALUES}" >/tmp/made-helm-template.yaml

# --- Gate 1: default render refuses to produce a manifest without a
# pinned image. Keeps ":latest" accidents out of production.
if helm template made "${CHART_PATH}" "${AUTH_ARGS[@]}" "${LOCAL_STORE_ARGS[@]}" > /dev/null 2>"${DEFAULT_ERR}"; then
  echo "default chart render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "set image.tag or image.digest" "${DEFAULT_ERR}"

# --- Gate 2: persistence.postgres.enabled without any URL source
# must fail loudly. Mis-configured persistence should never silently
# install a broken pod.
if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set persistence.postgres.enabled=true \
  > /dev/null 2>"${HARDENED_ERR}"; then
  echo "postgres-enabled-without-url render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "persistence.postgres.enabled=true requires" "${HARDENED_ERR}"

# --- Gate 3: full hardened render (every knob turned on) must
# produce a valid manifest that carries every hardening feature.
helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set networkPolicy.enabled=true \
  --set persistence.postgres.enabled=true \
  --set persistence.postgres.urlFromSecret.name=pg-dsn \
  --set persistence.postgres.urlFromSecret.key=url \
  --set pdb.enabled=true \
  > "${HARDENED_OUT}"

# Required items in the hardened manifest. Each assertion pins a
# specific guarantee operators will rely on; a rename anywhere in
# the chart breaks CI.
required_markers=(
  "kind: NetworkPolicy"
  "kind: PodDisruptionBudget"
  "automountServiceAccountToken: false"
  "readOnlyRootFilesystem: true"
  "emptyDir:"
  "mountPath: /tmp"
  "secretKeyRef:"
  'name: "pg-dsn"'
  'key: "url"'
)
for marker in "${required_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${HARDENED_OUT}"; then
    echo "hardened chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# --- Gate 4: authorization is fail-closed. Server-only TLS is insufficient;
# mutual TLS, a prebootstrapped policy id and a certificate-principal map are
# mandatory. The valid render wires those inputs without embedding secrets.

TLS_SERVER_ERR="${TMP_DIR}/made-helm-tls-server.err"
TLS_MUTUAL_OUT="${TMP_DIR}/made-helm-tls-mutual.yaml"
TLS_MISSING_ERR="${TMP_DIR}/made-helm-tls-missing.err"
AUTH_POLICY_MISSING_ERR="${TMP_DIR}/made-helm-auth-policy-missing.err"
AUTH_PRINCIPALS_MISSING_ERR="${TMP_DIR}/made-helm-auth-principals-missing.err"
AUTH_STORE_MISSING_ERR="${TMP_DIR}/made-helm-auth-store-missing.err"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set tls.mode=server \
  --set tls.existingSecret=made-grpc-tls \
  > /dev/null 2>"${TLS_SERVER_ERR}"; then
  echo "tls.mode=server render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "authorization requires tls.mode=mutual" "${TLS_SERVER_ERR}"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  "${LOCAL_STORE_ARGS[@]}" \
  --set image.tag=v0 \
  --set tls.existingSecret= \
  > /dev/null 2>"${TLS_MISSING_ERR}"; then
  echo "tls.mode=mutual with no existingSecret render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "tls.mode is not 'none' but tls.existingSecret is empty" "${TLS_MISSING_ERR}"

if helm template made "${CHART_PATH}" \
  --set image.tag=v0 \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls \
  --set authorization.principals.existingSecret=made-auth-principals \
  > /dev/null 2>"${AUTH_POLICY_MISSING_ERR}"; then
  echo "authorization without policy id render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "authorization.policyId is required" "${AUTH_POLICY_MISSING_ERR}"

if helm template made "${CHART_PATH}" \
  --set image.tag=v0 \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls \
  --set authorization.policyId=made-policy \
  > /dev/null 2>"${AUTH_PRINCIPALS_MISSING_ERR}"; then
  echo "authorization without principal map render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "authorization.principals.existingSecret is required" "${AUTH_PRINCIPALS_MISSING_ERR}"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  > /dev/null 2>"${AUTH_STORE_MISSING_ERR}"; then
  echo "authorization without durable policy store render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "authorization requires a durable policy store" "${AUTH_STORE_MISSING_ERR}"

helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  "${LOCAL_STORE_ARGS[@]}" \
  --set image.tag=v0 \
  --set tls.mode=mutual \
  --set tls.existingSecret=made-grpc-mtls \
  > "${TLS_MUTUAL_OUT}"

tls_mutual_markers=(
  'name: MADE_GRPC_TLS_MODE'
  'value: "mutual"'
  'name: MADE_GRPC_TLS_CLIENT_CA_PATH'
  'value: "/etc/made/tls/ca.crt"'
  'secretName: "made-grpc-mtls"'
  'defaultMode: 0440'
  'name: MADE_AUTH_POLICY_ID'
  'value: "made-test-policy"'
  'name: MADE_AUTH_MTLS_PRINCIPALS_PATH'
  'value: "/etc/made/auth/principals.json"'
  'name: authorization-principals'
  'secretName: "made-test-auth-principals"'
  'mountPath: /etc/made/auth'
)
for marker in "${tls_mutual_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${TLS_MUTUAL_OUT}"; then
    echo "tls=mutual chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# --- Gate 5: embedded NATS profile. The standalone bus profile must
# render a release-local NATS deployment and point MADE at
# that service.

EMBEDDED_NATS_OUT="${TMP_DIR}/made-helm-embedded-nats.yaml"
EMBEDDED_NATS_POLICY_OUT="${TMP_DIR}/made-helm-embedded-nats-policy.yaml"
EMBEDDED_NATS_POLICY_DOC="${TMP_DIR}/made-helm-embedded-nats-networkpolicy.yaml"

helm template made "${CHART_PATH}" \
  -f "${EMBEDDED_NATS_VALUES}" \
  --set image.tag=v0 \
  > "${EMBEDDED_NATS_OUT}"

embedded_nats_markers=(
  'name: made-nats'
  'app.kubernetes.io/component: nats'
  'image: "docker.io/library/nats:2.10-alpine"'
  'name: MADE_NATS_ENABLED'
  'value: "true"'
  'name: MADE_NATS_URL'
  'value: "nats://made-nats:4222"'
  'name: MADE_TRIGGER_SUBJECT'
  'value: "made.trigger.>"'
  'name: MADE_PUBLISH_PREFIX'
  'value: "made"'
)
for marker in "${embedded_nats_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${EMBEDDED_NATS_OUT}"; then
    echo "embedded NATS chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# The embedded bus has release-derived labels (`made-nats` for release
# `made`). Its NetworkPolicy peer must use the same helper rather than the
# external-NATS default selector, or a conforming CNI blocks the connection.
helm template made "${CHART_PATH}" \
  -f "${EMBEDDED_NATS_VALUES}" \
  --set image.tag=v0 \
  --set networkPolicy.enabled=true \
  --set 'networkPolicy.ingressFrom[0].podSelector.matchLabels.app=caller' \
  > "${EMBEDDED_NATS_POLICY_OUT}"

awk '
  /^# Source: made\/templates\/networkpolicy.yaml$/ { capture = 1; next }
  capture && /^---$/ { exit }
  capture { print }
' "${EMBEDDED_NATS_POLICY_OUT}" > "${EMBEDDED_NATS_POLICY_DOC}"

embedded_nats_policy_markers=(
  'app.kubernetes.io/name: made-nats'
  'app.kubernetes.io/instance: made'
  'app.kubernetes.io/component: nats'
  'port: 4222'
)
for marker in "${embedded_nats_policy_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${EMBEDDED_NATS_POLICY_DOC}"; then
    echo "embedded NATS NetworkPolicy missing peer marker: ${marker}" >&2
    exit 1
  fi
done

# Ceremony persistence owns a volume independently of /tmp and TLS. Exercise
# the combination that previously created a PVC and store env var but omitted
# both the Deployment volume and its mount.
CEREMONY_STORE_OUT="${TMP_DIR}/made-helm-ceremony-store.yaml"
helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set persistence.ceremonies.enabled=true \
  --set tmpVolume.enabled=false \
  > "${CEREMONY_STORE_OUT}"

ceremony_store_markers=(
  'kind: PersistentVolumeClaim'
  'name: ceremonies'
  'claimName: made-ceremonies'
  'mountPath: "/var/lib/made"'
  'name: MADE_CEREMONY_STORE_PATH'
  'value: "/var/lib/made/ceremonies.sqlite3"'
)
for marker in "${ceremony_store_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${CEREMONY_STORE_OUT}"; then
    echo "ceremony-store chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# --- Gate 6: Runtime executor render. Runtime mode must fail without
# an endpoint, must fail when TLS is requested without a secret, and
# the checked-in underpass-runtime profile must wire endpoint,
# principal, mTLS paths, and the runtime TLS secret mount.

RUNTIME_MISSING_ENDPOINT_ERR="${TMP_DIR}/made-helm-runtime-missing-endpoint.err"
RUNTIME_MISSING_TLS_ERR="${TMP_DIR}/made-helm-runtime-missing-tls.err"
RUNTIME_OUT="${TMP_DIR}/made-helm-runtime.yaml"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  "${LOCAL_STORE_ARGS[@]}" \
  --set image.tag=v0 \
  --set executor.kind=runtime \
  > /dev/null 2>"${RUNTIME_MISSING_ENDPOINT_ERR}"; then
  echo "executor.kind=runtime without endpoint render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "executor.kind=runtime but executor.runtime.endpoint is empty" "${RUNTIME_MISSING_ENDPOINT_ERR}"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  "${LOCAL_STORE_ARGS[@]}" \
  --set image.tag=v0 \
  --set executor.kind=runtime \
  --set executor.runtime.endpoint=https://underpass-runtime:50053 \
  --set executor.runtime.tls.mode=server \
  > /dev/null 2>"${RUNTIME_MISSING_TLS_ERR}"; then
  echo "runtime TLS without existingSecret render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "executor.runtime.tls.mode != disabled but executor.runtime.tls.existingSecret is empty" "${RUNTIME_MISSING_TLS_ERR}"

helm template made "${CHART_PATH}" \
  -f "${UNDERPASS_RUNTIME_VALUES}" \
  --set image.tag=v0 \
  > "${RUNTIME_OUT}"

runtime_markers=(
  'name: MADE_EXECUTOR_KIND'
  'value: "runtime"'
  'name: MADE_RUNTIME_GRPC_ENDPOINT'
  'value: "https://underpass-runtime:50053"'
  'name: MADE_RUNTIME_PRINCIPAL_TENANT_ID'
  'value: "underpass"'
  'name: MADE_RUNTIME_PRINCIPAL_ACTOR_ID'
  'value: "made"'
  'name: MADE_RUNTIME_PRINCIPAL_ROLES'
  'value: "orchestrator"'
  'name: MADE_RUNTIME_TLS_MODE'
  'value: "mutual"'
  'name: MADE_RUNTIME_TLS_CA_PATH'
  'value: "/etc/made/runtime-tls/ca.crt"'
  'name: MADE_RUNTIME_TLS_CERT_PATH'
  'value: "/etc/made/runtime-tls/tls.crt"'
  'name: MADE_RUNTIME_TLS_KEY_PATH'
  'value: "/etc/made/runtime-tls/tls.key"'
  'name: MADE_RUNTIME_TLS_DOMAIN_NAME'
  'value: "underpass-runtime"'
  'name: runtime-tls'
  'secretName: "made-runtime-client-tls"'
  'mountPath: /etc/made/runtime-tls'
  # vLLM provider + LLM-judge scoring (persisted in providerEnv so a
  # `helm upgrade` reproduces the working pod). If a future edit drops
  # this block, these markers fail CI before the drift reaches a cluster.
  # `toYaml` renders the URL/model scalars bare and the rest quoted.
  'name: MADE_VLLM_ENDPOINT'
  'value: http://underpass-llm-gemma-4-31b-structured:8000'
  'name: MADE_VLLM_MODEL'
  'value: google/gemma-4-31B-it'
  'name: MADE_VLLM_MAX_TOKENS'
  'name: MADE_VLLM_TIMEOUT_SECS'
  'name: MADE_JUDGE_ENABLED'
  'name: MADE_JUDGE_THRESHOLD'
  'value: "0.5"'
  'name: MADE_OTLP_ENDPOINT'
  'value: https://underpass-runtime-otel-collector:4317'
  'name: MADE_OTLP_TLS_CA_PATH'
  'name: MADE_OTLP_TLS_CERT_PATH'
  'name: MADE_OTLP_TLS_KEY_PATH'
  'name: MADE_OTLP_TLS_DOMAIN_NAME'
)
for marker in "${runtime_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${RUNTIME_OUT}"; then
    echo "runtime executor chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# --- Gate 7: Postgres secret profile. Persistence must be sourced
# through valueFrom.secretKeyRef, not through a literal DSN in the
# rendered Deployment.

POSTGRES_SECRET_OUT="${TMP_DIR}/made-helm-postgres-secret.yaml"

helm template made "${CHART_PATH}" \
  -f "${POSTGRES_SECRET_VALUES}" \
  --set image.tag=v0 \
  > "${POSTGRES_SECRET_OUT}"

postgres_secret_markers=(
  'name: MADE_POSTGRES_URL'
  'valueFrom:'
  'secretKeyRef:'
  'name: "made-postgres-dsn"'
  'key: "url"'
)
for marker in "${postgres_secret_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${POSTGRES_SECRET_OUT}"; then
    echo "postgres secret chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

if grep -qF 'postgres://' "${POSTGRES_SECRET_OUT}"; then
  echo "postgres secret profile rendered a literal Postgres DSN" >&2
  exit 1
fi

# --- Gate 8: provider env secret wiring. Provider configuration must
# support envFrom Secret overlays and per-key secretKeyRef env entries.

PROVIDER_ENVFROM_OUT="${TMP_DIR}/made-helm-provider-envfrom.yaml"
PROVIDER_ENV_OUT="${TMP_DIR}/made-helm-provider-env.yaml"

helm template made "${CHART_PATH}" \
  -f "${EMBEDDED_NATS_VALUES}" \
  -f "${PROVIDER_ENV_VALUES}" \
  --set image.tag=v0 \
  > "${PROVIDER_ENVFROM_OUT}"

provider_envfrom_markers=(
  'envFrom:'
  'secretRef:'
  'name: made-provider-env'
)
for marker in "${provider_envfrom_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${PROVIDER_ENVFROM_OUT}"; then
    echo "provider envFrom chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

helm template made "${CHART_PATH}" \
  -f "${EMBEDDED_NATS_VALUES}" \
  --set image.tag=v0 \
  --set 'providerEnv[0].name=MADE_OPENAI_API_KEY' \
  --set 'providerEnv[0].valueFrom.secretKeyRef.name=made-openai' \
  --set 'providerEnv[0].valueFrom.secretKeyRef.key=api-key' \
  --set 'providerEnv[1].name=MADE_OPENAI_MODEL' \
  --set 'providerEnv[1].value=gpt-4o-mini' \
  > "${PROVIDER_ENV_OUT}"

provider_env_markers=(
  'name: MADE_OPENAI_API_KEY'
  'secretKeyRef:'
  'name: made-openai'
  'key: api-key'
  'name: MADE_OPENAI_MODEL'
  'value: gpt-4o-mini'
)
for marker in "${provider_env_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${PROVIDER_ENV_OUT}"; then
    echo "provider env chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

# --- Gate 9: replicas require one shared persistence decision. Three replicas
# with Postgres are supported; the chart must refuse in-memory/SQLite replicas
# and mixed local + Postgres ceremony stores.

MULTI_WITHOUT_POSTGRES_ERR="${TMP_DIR}/made-helm-multi-without-postgres.err"
MIXED_CEREMONY_ERR="${TMP_DIR}/made-helm-mixed-ceremony.err"
MULTI_WITHOUT_CURSOR_ERR="${TMP_DIR}/made-helm-multi-without-cursor.err"
POSTGRES_MULTI_OUT="${TMP_DIR}/made-helm-postgres-multi.yaml"

if helm template made "${CHART_PATH}" \
  --set image.tag=v0 \
  --set replicaCount=3 \
  > /dev/null 2>"${MULTI_WITHOUT_POSTGRES_ERR}"; then
  echo "three-replica render without Postgres unexpectedly succeeded" >&2
  exit 1
fi
grep -q "replicaCount>1 requires persistence.postgres.enabled=true" "${MULTI_WITHOUT_POSTGRES_ERR}"

if helm template made "${CHART_PATH}" \
  --set image.tag=v0 \
  --set persistence.ceremonies.enabled=true \
  --set persistence.postgres.enabled=true \
  --set persistence.postgres.urlFromSecret.name=pg-dsn \
  --set persistence.postgres.urlFromSecret.key=url \
  > /dev/null 2>"${MIXED_CEREMONY_ERR}"; then
  echo "mixed local and Postgres ceremony render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "choose one ceremony adapter" "${MIXED_CEREMONY_ERR}"

if helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set replicaCount=3 \
  --set persistence.postgres.enabled=true \
  --set persistence.postgres.urlFromSecret.name=pg-dsn \
  --set persistence.postgres.urlFromSecret.key=url \
  --set ceremonySearch.cursorHmacKeyFromSecret.name= \
  > /dev/null 2>"${MULTI_WITHOUT_CURSOR_ERR}"; then
  echo "three-replica render without shared cursor key unexpectedly succeeded" >&2
  exit 1
fi
grep -q "replicaCount>1 requires ceremonySearch.cursorHmacKeyFromSecret" "${MULTI_WITHOUT_CURSOR_ERR}"

helm template made "${CHART_PATH}" \
  "${AUTH_ARGS[@]}" \
  --set image.tag=v0 \
  --set replicaCount=3 \
  --set persistence.postgres.enabled=true \
  --set persistence.postgres.urlFromSecret.name=pg-dsn \
  --set persistence.postgres.urlFromSecret.key=url \
  > "${POSTGRES_MULTI_OUT}"

postgres_multi_markers=(
  'replicas: 3'
  'name: MADE_POSTGRES_URL'
  'name: "pg-dsn"'
  'key: "url"'
  'name: MADE_CEREMONY_STORE_ID'
  'value: "made-test-store"'
  'name: MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY'
  'name: "made-test-cursor-hmac"'
)
for marker in "${postgres_multi_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${POSTGRES_MULTI_OUT}"; then
    echo "three-replica Postgres manifest missing marker: ${marker}" >&2
    exit 1
  fi
done

if grep -qF 'name: MADE_CEREMONY_STORE_PATH' "${POSTGRES_MULTI_OUT}"; then
  echo "three-replica Postgres manifest selected local ceremony storage" >&2
  exit 1
fi
