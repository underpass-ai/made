#!/usr/bin/env bash
set -euo pipefail

# Launches the vLLM provider-E2E Job, waits for completion, tails
# logs, and cleans up. Namespace defaults to `underpass-runtime`
# (where the e2e-client-tls secret lives); override with $NAMESPACE.
#
# The container image must already be built and pushed. Reuses the
# Provider-E2E image tag by convention (see `make build-provider-image`).

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="${NAMESPACE:-underpass-runtime}"
JOB_NAME="${JOB_NAME:-choreographer-e2e-provider-vllm}"
MANIFEST="${ROOT_DIR}/tests/e2e/kubernetes/provider-vllm-job.yaml"
TIMEOUT="${TIMEOUT:-180s}"
PROVIDER_IMAGE="${PROVIDER_IMAGE:-ghcr.io/underpass-ai/underpass-choreographer-e2e-provider:latest}"
VLLM_ENDPOINT="${CHOREO_VLLM_ENDPOINT:-https://llm.underpassai.com}"
VLLM_MODEL="${CHOREO_VLLM_MODEL:-google/gemma-4-31B-it}"
TLS_SECRET_NAME="${E2E_CLIENT_TLS_SECRET:-e2e-client-tls}"
IMAGE_PULL_SECRET="${E2E_IMAGE_PULL_SECRET:-}"
RENDERED_MANIFEST=""

cleanup() {
    kubectl -n "${NAMESPACE}" delete job "${JOB_NAME}" --ignore-not-found --wait=false >/dev/null 2>&1 || true
    if [ -n "${RENDERED_MANIFEST}" ] && [ -f "${RENDERED_MANIFEST}" ]; then
        rm -f "${RENDERED_MANIFEST}"
    fi
}

# Make the cleanup idempotent — a prior failed Job would block apply.
cleanup
trap cleanup EXIT

if ! kubectl get secret "${TLS_SECRET_NAME}" -n "${NAMESPACE}" >/dev/null 2>&1; then
    echo "error: required TLS secret '${TLS_SECRET_NAME}' not found in namespace '${NAMESPACE}'" >&2
    exit 1
fi

RENDERED_MANIFEST="$(mktemp)"
awk \
    -v image="${PROVIDER_IMAGE}" \
    -v endpoint="${VLLM_ENDPOINT}" \
    -v model="${VLLM_MODEL}" \
    -v tls_secret="${TLS_SECRET_NAME}" \
    -v pull_secret="${IMAGE_PULL_SECRET}" '
      {
        gsub("image: ghcr.io/underpass-ai/underpass-choreographer-e2e-provider:latest", "image: " image);
        gsub("secretName: e2e-client-tls", "secretName: " tls_secret);
      }
      /- name: CHOREO_VLLM_ENDPOINT/ {
        print;
        getline;
        sub(/value:.*/, "value: " endpoint);
        print;
        next;
      }
      /- name: CHOREO_VLLM_MODEL/ {
        print;
        getline;
        sub(/value:.*/, "value: " model);
        print;
        next;
      }
      /restartPolicy: Never/ {
        print;
        if (pull_secret != "") {
          print "      imagePullSecrets:";
          print "        - name: " pull_secret;
        }
        next;
      }
      { print }
    ' "${MANIFEST}" > "${RENDERED_MANIFEST}"

kubectl apply -n "${NAMESPACE}" -f "${RENDERED_MANIFEST}"

# Poll for the Job's pod to be scheduled, then tail its logs.
echo ">>> waiting for pod"
for _ in $(seq 1 30); do
    pod=$(kubectl -n "${NAMESPACE}" get pod \
        -l "job-name=${JOB_NAME}" \
        -o=jsonpath='{.items[0].metadata.name}' 2>/dev/null || true)
    if [ -n "${pod:-}" ]; then
        break
    fi
    sleep 2
done

if [ -z "${pod:-}" ]; then
    echo "error: Job ${JOB_NAME} produced no pod" >&2
    kubectl -n "${NAMESPACE}" get job "${JOB_NAME}" -o yaml >&2 || true
    exit 1
fi

echo ">>> tailing ${pod}"
kubectl -n "${NAMESPACE}" logs -f "${pod}" || true

# Surface the final Job status so callers can script on exit code.
if kubectl -n "${NAMESPACE}" wait --for=condition=complete \
    --timeout="${TIMEOUT}" "job/${JOB_NAME}" >/dev/null 2>&1; then
    echo ">>> Job completed successfully"
    exit 0
fi

echo ">>> Job did not complete within ${TIMEOUT} — dumping status" >&2
kubectl -n "${NAMESPACE}" describe job "${JOB_NAME}" >&2 || true
kubectl -n "${NAMESPACE}" describe pod "${pod}" >&2 || true
exit 1
