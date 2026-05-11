#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/choreographer}"
DEV_VALUES="${CHART_PATH}/values.dev.yaml"
TMP_DIR="${TMPDIR:-/tmp}"
DEFAULT_ERR="${TMP_DIR}/choreographer-helm-default.err"
HARDENED_OUT="${TMP_DIR}/choreographer-helm-hardened.yaml"
HARDENED_ERR="${TMP_DIR}/choreographer-helm-hardened.err"

helm lint "${CHART_PATH}" -f "${DEV_VALUES}"
helm template choreographer "${CHART_PATH}" -f "${DEV_VALUES}" >/tmp/choreographer-helm-template.yaml

# --- Gate 1: default render refuses to produce a manifest without a
# pinned image. Keeps ":latest" accidents out of production.
if helm template choreographer "${CHART_PATH}" > /dev/null 2>"${DEFAULT_ERR}"; then
  echo "default chart render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "set image.tag or image.digest" "${DEFAULT_ERR}"

# --- Gate 2: persistence.postgres.enabled without any URL source
# must fail loudly. Mis-configured persistence should never silently
# install a broken pod.
if helm template choreographer "${CHART_PATH}" \
  --set image.tag=v0 \
  --set persistence.postgres.enabled=true \
  > /dev/null 2>"${HARDENED_ERR}"; then
  echo "postgres-enabled-without-url render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "persistence.postgres.enabled=true requires" "${HARDENED_ERR}"

# --- Gate 3: full hardened render (every knob turned on) must
# produce a valid manifest that carries every hardening feature.
helm template choreographer "${CHART_PATH}" \
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

# --- Gate 4: TLS render. `tls.mode=server` with an existingSecret
# must wire the env vars + volume mount; `tls.mode=mutual` must also
# carry the client-CA env var; `tls.mode=server` without a secret
# must fail loudly.

TLS_SERVER_OUT="${TMP_DIR}/choreographer-helm-tls-server.yaml"
TLS_MUTUAL_OUT="${TMP_DIR}/choreographer-helm-tls-mutual.yaml"
TLS_MISSING_ERR="${TMP_DIR}/choreographer-helm-tls-missing.err"

if helm template choreographer "${CHART_PATH}" \
  --set image.tag=v0 \
  --set tls.mode=server \
  > /dev/null 2>"${TLS_MISSING_ERR}"; then
  echo "tls.mode=server with no existingSecret render unexpectedly succeeded" >&2
  exit 1
fi
grep -q "tls.mode is not 'none' but tls.existingSecret is empty" "${TLS_MISSING_ERR}"

helm template choreographer "${CHART_PATH}" \
  --set image.tag=v0 \
  --set tls.mode=server \
  --set tls.existingSecret=choreo-grpc-tls \
  > "${TLS_SERVER_OUT}"

tls_server_markers=(
  'name: CHOREO_GRPC_TLS_MODE'
  'value: "server"'
  'name: CHOREO_GRPC_TLS_CERT_PATH'
  'value: "/etc/choreographer/tls/tls.crt"'
  'name: CHOREO_GRPC_TLS_KEY_PATH'
  'value: "/etc/choreographer/tls/tls.key"'
  'name: grpc-tls'
  'secretName: "choreo-grpc-tls"'
  'mountPath: /etc/choreographer/tls'
)
for marker in "${tls_server_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${TLS_SERVER_OUT}"; then
    echo "tls=server chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done

if grep -qF 'CHOREO_GRPC_TLS_CLIENT_CA_PATH' "${TLS_SERVER_OUT}"; then
  echo "tls=server manifest must NOT carry CHOREO_GRPC_TLS_CLIENT_CA_PATH" >&2
  exit 1
fi

helm template choreographer "${CHART_PATH}" \
  --set image.tag=v0 \
  --set tls.mode=mutual \
  --set tls.existingSecret=choreo-grpc-mtls \
  > "${TLS_MUTUAL_OUT}"

tls_mutual_markers=(
  'name: CHOREO_GRPC_TLS_MODE'
  'value: "mutual"'
  'name: CHOREO_GRPC_TLS_CLIENT_CA_PATH'
  'value: "/etc/choreographer/tls/ca.crt"'
  'secretName: "choreo-grpc-mtls"'
)
for marker in "${tls_mutual_markers[@]}"; do
  if ! grep -qF -- "${marker}" "${TLS_MUTUAL_OUT}"; then
    echo "tls=mutual chart manifest missing required marker: ${marker}" >&2
    exit 1
  fi
done
