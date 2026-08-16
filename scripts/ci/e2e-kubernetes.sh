#!/usr/bin/env bash
set -euo pipefail

# End-to-end validation against Kubernetes.
#
# Flow:
#   1. Build the MADE image and the E2E runner image.
#   2. Make those images reachable from the target cluster:
#      - `kind` mode: `kind load docker-image`
#      - existing-cluster mode: push to a cluster-accessible registry.
#        The standard path is GHCR + `imagePullSecrets` (matching
#        sibling repos); a local in-cluster registry is a cluster-
#        specific fallback via `kubectl port-forward`.
#   2. Deploy a tiny NATS (Deployment + Service).
#   3. `helm install` the MADE chart pointed at the local
#      images, with seeding enabled so `ListCouncils` returns a
#      non-empty set.
#   4. Submit the E2E runner as a Kubernetes Job; its exit code
#      decides the CI result.
#
# Any step that fails dumps diagnostics (pod logs, kubectl describe)
# before bailing so CI is debuggable without re-running.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHART_PATH="${ROOT_DIR}/charts/made"
NATS_MANIFEST="${ROOT_DIR}/tests/e2e/kubernetes/nats.yaml"
JOB_MANIFEST="${ROOT_DIR}/tests/e2e/kubernetes/runner-job.yaml"
IMAGE_TAG="${E2E_IMAGE_TAG:-e2e}"
MADE_IMAGE="made:${IMAGE_TAG}"
RUNNER_IMAGE="made-e2e-runner:${IMAGE_TAG}"
CLUSTER_NAME="${KIND_CLUSTER_NAME:-made-e2e}"
# In kind mode the cluster does not exist yet, so there is no current
# context to minify and this would abort under `set -e` before the script
# gets to create one. A missing context is not an error here; it is the
# normal starting state on a machine that has never talked to a cluster.
CURRENT_NAMESPACE="$(kubectl config view --minify --output 'jsonpath={..namespace}' 2>/dev/null || true)"
NAMESPACE="${E2E_NAMESPACE:-${CURRENT_NAMESPACE:-default}}"
CLUSTER_MODE="${E2E_CLUSTER_MODE:-auto}" # auto|kind|existing
IMAGE_REPOSITORY_PREFIX="${E2E_IMAGE_REPOSITORY_PREFIX:-}"
IMAGE_PULL_SECRET="${E2E_IMAGE_PULL_SECRET:-}"
REGISTRY_NAMESPACE="${E2E_REGISTRY_NAMESPACE:-container-registry}"
REGISTRY_SERVICE="${E2E_REGISTRY_SERVICE:-docker-registry}"
REGISTRY_LOCAL_ADDR="${E2E_REGISTRY_LOCAL_ADDR:-127.0.0.1:5001}"
REGISTRY_CLUSTER_ADDR="${E2E_REGISTRY_CLUSTER_ADDR:-docker-registry.container-registry.svc.cluster.local:5000}"
MADE_IMAGE_REMOTE="${REGISTRY_CLUSTER_ADDR}/made:${IMAGE_TAG}"
RUNNER_IMAGE_REMOTE="${REGISTRY_CLUSTER_ADDR}/made-e2e-runner:${IMAGE_TAG}"
MADE_PULL_POLICY="Never"
RUNNER_PULL_POLICY="Never"
REGISTRY_PID=""
RENDERED_JOB=""

cd "${ROOT_DIR}"

if [ ! -f "${JOB_MANIFEST}" ] || [ ! -f "${NATS_MANIFEST}" ]; then
  echo "::warning::${JOB_MANIFEST} or ${NATS_MANIFEST} not present yet; E2E placeholder"
  exit 0
fi

on_error() {
  echo "::group::kubectl get all"
  kubectl get all -A || true
  echo "::endgroup::"
  echo "::group::kubectl describe job"
  kubectl describe job made-e2e-runner -n "${NAMESPACE}" || true
  echo "::endgroup::"
  echo "::group::runner logs"
  kubectl logs job/made-e2e-runner -n "${NAMESPACE}" --tail=500 || true
  echo "::endgroup::"
  echo "::group::MADE logs"
  kubectl logs -n "${NAMESPACE}" -l app.kubernetes.io/name=made --tail=500 || true
  echo "::endgroup::"
  echo "::group::nats logs"
  kubectl logs -n "${NAMESPACE}" -l app=nats --tail=200 || true
  echo "::endgroup::"
}
trap on_error ERR

cleanup() {
  if [ -n "${REGISTRY_PID}" ]; then
    kill "${REGISTRY_PID}" >/dev/null 2>&1 || true
  fi
  if [ -n "${RENDERED_JOB}" ] && [ -f "${RENDERED_JOB}" ]; then
    rm -f "${RENDERED_JOB}"
  fi
}
trap cleanup EXIT

select_container_cli() {
  case "${CONTAINER_RUNTIME:-auto}" in
    auto)
      if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
        echo "docker"
        return 0
      fi
      if command -v podman >/dev/null 2>&1; then
        echo "podman"
        return 0
      fi
      echo "no supported container runtime found; install docker or podman" >&2
      return 1
      ;;
    docker)
      echo "docker"
      ;;
    podman)
      echo "podman"
      ;;
    *)
      echo "unsupported CONTAINER_RUNTIME=${CONTAINER_RUNTIME}" >&2
      return 1
      ;;
  esac
}

detect_cluster_mode() {
  case "${CLUSTER_MODE}" in
    kind|existing)
      echo "${CLUSTER_MODE}"
      ;;
    auto)
      if command -v kind >/dev/null 2>&1; then
        echo "kind"
      else
        echo "existing"
      fi
      ;;
    *)
      echo "unsupported E2E_CLUSTER_MODE=${CLUSTER_MODE}; expected auto, kind, or existing" >&2
      return 1
      ;;
  esac
}

wait_for_registry() {
  local url="http://${REGISTRY_LOCAL_ADDR}/v2/"
  for _ in $(seq 1 30); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "registry port-forward did not become ready at ${url}" >&2
  return 1
}

push_images_to_cluster_registry() {
  local container_cli="$1"
  local local_made_ref="${REGISTRY_LOCAL_ADDR}/made:${IMAGE_TAG}"
  local local_runner_ref="${REGISTRY_LOCAL_ADDR}/made-e2e-runner:${IMAGE_TAG}"

  echo ">>> port-forwarding registry ${REGISTRY_SERVICE}.${REGISTRY_NAMESPACE}.svc:5000 to ${REGISTRY_LOCAL_ADDR}"
  kubectl port-forward -n "${REGISTRY_NAMESPACE}" "svc/${REGISTRY_SERVICE}" "${REGISTRY_LOCAL_ADDR#*:}:5000" \
    >/tmp/made-e2e-registry-port-forward.log 2>&1 &
  REGISTRY_PID=$!
  wait_for_registry

  echo ">>> tagging images for local registry push"
  "${container_cli}" tag "${MADE_IMAGE}" "${local_made_ref}"
  "${container_cli}" tag "${RUNNER_IMAGE}" "${local_runner_ref}"

  echo ">>> pushing images to cluster registry via ${REGISTRY_LOCAL_ADDR}"
  if [ "${container_cli}" = "podman" ]; then
    "${container_cli}" push --tls-verify=false "${local_made_ref}"
    "${container_cli}" push --tls-verify=false "${local_runner_ref}"
  else
    "${container_cli}" push "${local_made_ref}"
    "${container_cli}" push "${local_runner_ref}"
  fi

  MADE_IMAGE="${MADE_IMAGE_REMOTE}"
  RUNNER_IMAGE="${RUNNER_IMAGE_REMOTE}"
  MADE_PULL_POLICY="Always"
  RUNNER_PULL_POLICY="Always"
}

push_images_to_external_registry() {
  local container_cli="$1"

  echo ">>> pushing images to external registry"
  "${container_cli}" push "${MADE_IMAGE}"
  "${container_cli}" push "${RUNNER_IMAGE}"

  MADE_PULL_POLICY="Always"
  RUNNER_PULL_POLICY="Always"
}

render_runner_job() {
  RENDERED_JOB="$(mktemp)"
  awk \
    -v image="${RUNNER_IMAGE}" \
    -v pull="${RUNNER_PULL_POLICY}" \
    -v pull_secret="${IMAGE_PULL_SECRET}" '
      {
        gsub("image: made-e2e-runner:e2e", "image: " image);
        gsub("imagePullPolicy: Never", "imagePullPolicy: " pull);
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
    ' "${JOB_MANIFEST}" > "${RENDERED_JOB}"
}

CONTAINER_CLI="$(select_container_cli)"
EFFECTIVE_CLUSTER_MODE="$(detect_cluster_mode)"

if [ -z "${IMAGE_REPOSITORY_PREFIX}" ] && [ "${EFFECTIVE_CLUSTER_MODE}" = "existing" ]; then
  echo "::notice::existing-cluster E2E defaults to the in-cluster registry fallback; for the sibling-repo path prefer E2E_IMAGE_REPOSITORY_PREFIX=ghcr.io/underpass-ai and E2E_IMAGE_PULL_SECRET=ghcr-pull"
fi

if [ -n "${IMAGE_REPOSITORY_PREFIX}" ]; then
  MADE_IMAGE="${IMAGE_REPOSITORY_PREFIX}/made:${IMAGE_TAG}"
  RUNNER_IMAGE="${IMAGE_REPOSITORY_PREFIX}/made-e2e-runner:${IMAGE_TAG}"
fi

echo ">>> building MADE image"
bash scripts/ci/container-image.sh "${MADE_IMAGE}" Dockerfile

echo ">>> building e2e runner image"
bash scripts/ci/container-image.sh "${RUNNER_IMAGE}" tests/e2e/runner.Dockerfile

if [ -n "${IMAGE_REPOSITORY_PREFIX}" ]; then
  push_images_to_external_registry "${CONTAINER_CLI}"
else
  case "${EFFECTIVE_CLUSTER_MODE}" in
    kind)
      # This script loads images into a cluster; it does not create one.
      # Creating a cluster is the operator's call, so say what is missing
      # and the exact command instead of failing eight lines later with
      # "no nodes found".
      if ! kind get clusters 2>/dev/null | grep -qx "${CLUSTER_NAME}"; then
        echo "kind cluster '${CLUSTER_NAME}' does not exist." >&2
        echo "create it first:  kind create cluster --name ${CLUSTER_NAME} --wait 120s" >&2
        echo "delete it after:  kind delete cluster --name ${CLUSTER_NAME}" >&2
        exit 1
      fi
      echo ">>> loading images into kind cluster '${CLUSTER_NAME}'"
      kind load docker-image "${MADE_IMAGE}" --name "${CLUSTER_NAME}"
      kind load docker-image "${RUNNER_IMAGE}" --name "${CLUSTER_NAME}"
      ;;
    existing)
      push_images_to_cluster_registry "${CONTAINER_CLI}"
      ;;
  esac
fi

echo ">>> deploying NATS"
kubectl apply -n "${NAMESPACE}" -f "${NATS_MANIFEST}"
kubectl wait -n "${NAMESPACE}" --for=condition=available --timeout=2m deployment/nats

echo ">>> installing MADE chart"
HELM_ARGS=(
  upgrade --install made "${CHART_PATH}"
  -n "${NAMESPACE}"
  --create-namespace
  --values "${CHART_PATH}/values.dev.yaml"
  --set "image.repository=${MADE_IMAGE%:*}"
  --set "image.tag=${IMAGE_TAG}"
  --set "image.pullPolicy=${MADE_PULL_POLICY}"
  --set "config.seedSpecialties=triage"
  --wait --timeout 3m
)
if [ -n "${IMAGE_PULL_SECRET}" ]; then
  HELM_ARGS+=(--set "imagePullSecrets[0].name=${IMAGE_PULL_SECRET}")
fi

helm "${HELM_ARGS[@]}"

echo ">>> submitting E2E runner Job"
render_runner_job
kubectl delete job made-e2e-runner -n "${NAMESPACE}" --ignore-not-found
kubectl apply -n "${NAMESPACE}" -f "${RENDERED_JOB}"

echo ">>> waiting for Job to complete"
# Use `kubectl wait` with both conditions so we fail fast on a
# runner that bailed out (condition=failed) instead of sleeping
# through the full timeout.
if ! kubectl wait -n "${NAMESPACE}" --for=condition=complete --timeout=5m job/made-e2e-runner; then
  if kubectl wait -n "${NAMESPACE}" --for=condition=failed --timeout=10s job/made-e2e-runner 2>/dev/null; then
    echo "::error::e2e runner Job failed"
  else
    echo "::error::e2e runner Job did not complete within 5m"
  fi
  on_error
  exit 1
fi

echo ">>> runner logs"
kubectl logs -n "${NAMESPACE}" job/made-e2e-runner

echo ">>> E2E kubernetes scenarios passed"
