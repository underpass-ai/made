#!/usr/bin/env bash
set -euo pipefail

# Launches the Choreographer multi-agent vLLM E2E Job, waits for
# completion, tails logs, and cleans up. Namespace defaults to the
# Underpass runtime namespace because that is where the shared
# Choreographer + vLLM services run in the operator environment.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="${NAMESPACE:-underpass-runtime}"
JOB_NAME="${JOB_NAME:-choreographer-e2e-council-vllm}"
MANIFEST="${ROOT_DIR}/tests/e2e/kubernetes/council-vllm-job.yaml"
TIMEOUT="${TIMEOUT:-600s}"
RUNNER_IMAGE="${RUNNER_IMAGE:-ghcr.io/underpass-ai/underpass-choreographer-e2e-runner:latest}"
CHOREOGRAPHER_ENDPOINT="${CHOREOGRAPHER_ENDPOINT:-http://choreographer:50055}"
VLLM_ENDPOINT="${CHOREO_VLLM_ENDPOINT:-http://underpass-llm-gemma-4-31b-structured:8000}"
VLLM_MODEL="${CHOREO_VLLM_MODEL:-google/gemma-4-31B-it}"
VLLM_AGENT_COUNT="${CHOREO_VLLM_AGENT_COUNT:-3}"
VLLM_MAX_TOKENS="${CHOREO_VLLM_MAX_TOKENS:-512}"
VLLM_TIMEOUT_SECS="${CHOREO_VLLM_TIMEOUT_SECS:-300}"
IMAGE_PULL_SECRET="${E2E_IMAGE_PULL_SECRET:-}"
RENDERED_MANIFEST=""

cleanup() {
    kubectl -n "${NAMESPACE}" delete job "${JOB_NAME}" --ignore-not-found --wait=false >/dev/null 2>&1 || true
    if [ -n "${RENDERED_MANIFEST}" ] && [ -f "${RENDERED_MANIFEST}" ]; then
        rm -f "${RENDERED_MANIFEST}"
    fi
}

cleanup
trap cleanup EXIT

RENDERED_MANIFEST="$(mktemp)"
awk \
    -v image="${RUNNER_IMAGE}" \
    -v choreographer_endpoint="${CHOREOGRAPHER_ENDPOINT}" \
    -v vllm_endpoint="${VLLM_ENDPOINT}" \
    -v vllm_model="${VLLM_MODEL}" \
    -v agent_count="${VLLM_AGENT_COUNT}" \
    -v max_tokens="${VLLM_MAX_TOKENS}" \
    -v timeout_secs="${VLLM_TIMEOUT_SECS}" \
    -v pull_secret="${IMAGE_PULL_SECRET}" '
      {
        gsub("image: ghcr.io/underpass-ai/underpass-choreographer-e2e-runner:latest", "image: " image);
      }
      /- name: CHOREOGRAPHER_ENDPOINT/ {
        print;
        getline;
        sub(/value:.*/, "value: " choreographer_endpoint);
        print;
        next;
      }
      /- name: CHOREO_VLLM_ENDPOINT/ {
        print;
        getline;
        sub(/value:.*/, "value: " vllm_endpoint);
        print;
        next;
      }
      /- name: CHOREO_VLLM_MODEL/ {
        print;
        getline;
        sub(/value:.*/, "value: " vllm_model);
        print;
        next;
      }
      /- name: CHOREO_VLLM_AGENT_COUNT/ {
        print;
        getline;
        sub(/value:.*/, "value: \"" agent_count "\"");
        print;
        next;
      }
      /- name: CHOREO_VLLM_MAX_TOKENS/ {
        print;
        getline;
        sub(/value:.*/, "value: \"" max_tokens "\"");
        print;
        next;
      }
      /- name: CHOREO_VLLM_TIMEOUT_SECS/ {
        print;
        getline;
        sub(/value:.*/, "value: \"" timeout_secs "\"");
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
kubectl -n "${NAMESPACE}" wait --for=condition=Ready \
    --timeout=120s "pod/${pod}" >/dev/null 2>&1 || true
kubectl -n "${NAMESPACE}" logs -f "${pod}" --pod-running-timeout=120s || true

if kubectl -n "${NAMESPACE}" wait --for=condition=failed \
    --timeout=1s "job/${JOB_NAME}" >/dev/null 2>&1; then
    echo ">>> Job failed — dumping status" >&2
    kubectl -n "${NAMESPACE}" describe job "${JOB_NAME}" >&2 || true
    kubectl -n "${NAMESPACE}" describe pod "${pod}" >&2 || true
    exit 1
fi

if kubectl -n "${NAMESPACE}" wait --for=condition=complete \
    --timeout="${TIMEOUT}" "job/${JOB_NAME}" >/dev/null 2>&1; then
    echo ">>> Job completed successfully"
    exit 0
fi

echo ">>> Job did not complete within ${TIMEOUT} — dumping status" >&2
kubectl -n "${NAMESPACE}" describe job "${JOB_NAME}" >&2 || true
kubectl -n "${NAMESPACE}" describe pod "${pod}" >&2 || true
exit 1
