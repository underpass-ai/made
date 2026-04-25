#!/usr/bin/env bash

# shellcheck shell=bash
#
# Resolve a Docker-compatible socket for testcontainers-backed suites.
# Docker "just works" when its daemon is reachable. Podman only works
# when its API socket is live and exported through DOCKER_HOST; several
# environments report a configured socket path even when the socket is
# not actually up yet, so we probe for a real Unix socket before
# handing control to cargo test.

set -euo pipefail

ensure_testcontainers_host() {
  if [ -n "${DOCKER_HOST:-}" ]; then
    case "${DOCKER_HOST}" in
      unix://*)
        local socket_path="${DOCKER_HOST#unix://}"
        if [ ! -S "${socket_path}" ]; then
          echo "DOCKER_HOST points to a non-existent Unix socket: ${socket_path}" >&2
          return 1
        fi
        ;;
    esac
    echo ">>> using preconfigured DOCKER_HOST=${DOCKER_HOST}" >&2
    return 0
  fi

  if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    echo ">>> using docker daemon for testcontainers" >&2
    return 0
  fi

  if command -v podman >/dev/null 2>&1; then
    local podman_socket=""
    podman_socket="$(podman info --format '{{.Host.RemoteSocket.Path}}' 2>/dev/null || true)"

    if [ -n "${podman_socket}" ] && [ -S "${podman_socket}" ]; then
      export DOCKER_HOST="unix://${podman_socket}"
      echo ">>> using podman socket for testcontainers: ${DOCKER_HOST}" >&2
      return 0
    fi

    cat >&2 <<'EOF'
no Docker daemon detected and Podman did not expose a live API socket.
testcontainers needs a Docker-compatible socket before the Rust tests run.

Recommended Podman setup:
  systemctl --user start podman.socket
  export DOCKER_HOST=unix://$(podman info --format '{{.Host.RemoteSocket.Path}}')

Preflight check:
  test -S "$(podman info --format '{{.Host.RemoteSocket.Path}}')"

Fallback when the user socket is unavailable:
  mkdir -p "${TMPDIR:-/tmp}/podman"
  podman system service --time=0 unix://${TMPDIR:-/tmp}/podman/podman.sock
  export DOCKER_HOST=unix://${TMPDIR:-/tmp}/podman/podman.sock
EOF
    return 1
  fi

  echo "no Docker-compatible container host detected; install Docker or Podman" >&2
  return 1
}
