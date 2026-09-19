#!/usr/bin/env bash
set -euo pipefail

# End-to-end validation via compose.
#
# Runtime-agnostic: autodetects `docker compose`, `podman compose`,
# or `podman-compose` in that order. Override with
# `CONTAINER_RUNTIME=podman|podman-compose|docker|auto`.
#
# Brings up MADE + NATS + the e2e runner container and runs
# the runner against the stack. The runner drives scenarios over the
# public gRPC / AsyncAPI contract — no access to internals.
#
# CI can prepare content-addressed images separately and set
# `MADE_E2E_BUILD_SERVICES` (including to an empty string) to make this script
# run the exact same suite with `--no-build`. When the variable is absent the
# existing local behavior remains `compose up --build`.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/tests/e2e/docker-compose.e2e.yaml"
: "${CONTAINER_RUNTIME:=auto}"
: "${COMPOSE_PROJECT_NAME:=made-e2e-${CI_JOB_ID:-local}-${BASHPID}}"
export COMPOSE_PROJECT_NAME

cd "${ROOT_DIR}"

if [ ! -f "${COMPOSE_FILE}" ]; then
  echo "::warning::${COMPOSE_FILE} not present yet; E2E placeholder"
  exit 0
fi

select_compose() {
  case "${CONTAINER_RUNTIME}" in
    auto)
      if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        echo "docker compose"
        return 0
      fi
      if command -v podman >/dev/null 2>&1 && podman compose --version >/dev/null 2>&1; then
        echo "podman compose"
        return 0
      fi
      if command -v podman >/dev/null 2>&1 && command -v podman-compose >/dev/null 2>&1; then
        echo "podman-compose"
        return 0
      fi
      echo "no supported compose runtime found (need 'docker compose', 'podman compose', or 'podman-compose')" >&2
      return 1
      ;;
    docker)
      command -v docker >/dev/null 2>&1 || { echo "docker not installed" >&2; return 1; }
      docker compose version >/dev/null 2>&1 || { echo "docker compose plugin missing" >&2; return 1; }
      echo "docker compose" ;;
    podman)
      if command -v podman >/dev/null 2>&1 && podman compose --version >/dev/null 2>&1; then
        echo "podman compose"
      elif command -v podman >/dev/null 2>&1 && command -v podman-compose >/dev/null 2>&1; then
        echo "podman-compose"
      else
        echo "podman compose support not found (need podman>=4.3 with compose, or podman-compose)" >&2
        return 1
      fi ;;
    podman-compose)
      if command -v podman >/dev/null 2>&1 && command -v podman-compose >/dev/null 2>&1; then
        echo "podman-compose"
      else
        echo "podman-compose support not found (need both 'podman' and 'podman-compose')" >&2
        return 1
      fi ;;
    *)
      echo "unsupported CONTAINER_RUNTIME=${CONTAINER_RUNTIME}; expected auto, docker, podman, or podman-compose" >&2
      return 1 ;;
  esac
}

# Read as an array so the command composes correctly whether it's a
# single word (`podman-compose`) or two (`docker compose`).
read -r -a COMPOSE <<<"$(select_compose)"
echo ">>> using compose runtime: ${COMPOSE[*]}" >&2
echo ">>> compose project: ${COMPOSE_PROJECT_NAME}" >&2

compose_logs() {
  local args=("${COMPOSE[@]}" -f "${COMPOSE_FILE}")

  if [ "${COMPOSE[0]}" = "podman-compose" ]; then
    "${args[@]}" logs
    return 0
  fi

  "${args[@]}" logs --no-color
}

cleanup() {
  compose_logs > tests/e2e/compose.log 2>&1 || true
  "${COMPOSE[@]}" -f "${COMPOSE_FILE}" down --volumes --remove-orphans || true
  if [[ "${AUTH_DIR_OWNED}" == true && -n "${MADE_E2E_AUTH_DIR:-}" ]]; then
    rm -rf "${MADE_E2E_AUTH_DIR}"
  fi
  if [[ "${STATE_DIR_OWNED}" == true && -n "${MADE_E2E_STATE_DIR:-}" ]]; then
    rm -rf "${MADE_E2E_STATE_DIR}"
  fi
}
AUTH_DIR_OWNED=false
STATE_DIR_OWNED=false
trap cleanup EXIT

if [[ -z "${MADE_E2E_AUTH_DIR:-}" ]]; then
  mkdir -p "${ROOT_DIR}/tmp"
  MADE_E2E_AUTH_DIR="$(mktemp -d "${ROOT_DIR}/tmp/e2e-auth.XXXXXX")"
  AUTH_DIR_OWNED=true
fi
export MADE_E2E_AUTH_DIR
"${ROOT_DIR}/tests/e2e/prepare-auth.sh" "${MADE_E2E_AUTH_DIR}"

if [[ -z "${MADE_E2E_STATE_DIR:-}" ]]; then
  mkdir -p "${ROOT_DIR}/tmp"
  MADE_E2E_STATE_DIR="$(mktemp -d "${ROOT_DIR}/tmp/e2e-state.XXXXXX")"
  STATE_DIR_OWNED=true
  # The distroless image runs as uid/gid 65532. The scratch directory holds
  # only this run's SQLite state and is removed by the exit trap.
  chmod 0777 "${MADE_E2E_STATE_DIR}"
fi
export MADE_E2E_STATE_DIR

if [[ -v MADE_E2E_BUILD_SERVICES ]]; then
  if [[ -n "${MADE_E2E_BUILD_SERVICES}" ]]; then
    read -r -a BUILD_SERVICES <<<"${MADE_E2E_BUILD_SERVICES}"
    "${COMPOSE[@]}" -f "${COMPOSE_FILE}" build "${BUILD_SERVICES[@]}"
  fi
else
  "${COMPOSE[@]}" -f "${COMPOSE_FILE}" build
fi

"${COMPOSE[@]}" -f "${COMPOSE_FILE}" run --rm --no-deps made \
  bootstrap-authorization --policy-id made-e2e-policy \
  --trusted-host-id made-e2e-owner

"${COMPOSE[@]}" -f "${COMPOSE_FILE}" up --no-build --abort-on-container-exit --exit-code-from e2e-runner
