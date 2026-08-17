#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# An explicit binary wins over everything below. The release bundle is built
# without the sqlite engine — that is what keeps a default install free of a C
# toolchain — and the bundle otherwise takes priority, so without this an
# operator who deliberately built `cargo install made-mcp --features sqlite`
# could not reach it through the plugin at all.
# It selects the binary and nothing else: the state path, the engine and the
# legacy import below still apply, because an operator overriding the
# executable is not asking to configure the rest by hand.
BINARY="${MADE_MCP_BIN:-${PLUGIN_ROOT}/bin/made-mcp}"
if [[ -n "${MADE_MCP_BIN:-}" && ! -x "${MADE_MCP_BIN}" ]]; then
  echo "MADE plugin: MADE_MCP_BIN is set to '${MADE_MCP_BIN}', which is not executable." >&2
  exit 127
fi

if [[ ! -x "${BINARY}" && -x "${PLUGIN_ROOT}/bin/made-mcp.exe" ]]; then
  BINARY="${PLUGIN_ROOT}/bin/made-mcp.exe"
fi

# The release bundle ships bin/made-mcp and keeps priority: it pins the binary
# this plugin version was tested against. An install straight from the
# repository has no bin/ — that path is gitignored — so fall back to an
# installed made-mcp on PATH rather than leaving the host with a server that
# cannot start.
if [[ ! -x "${BINARY}" ]]; then
  if PATH_BINARY="$(command -v made-mcp 2>/dev/null)"; then
    BINARY="${PATH_BINARY}"
  fi
fi

if [[ ! -x "${BINARY}" ]]; then
  echo "MADE plugin: no made-mcp executable found." >&2
  echo "MADE plugin: looked for ${PLUGIN_ROOT}/bin/made-mcp (release bundle) and made-mcp on PATH." >&2
  echo "MADE plugin: install one with 'cargo install made-mcp', or install the plugin from a release package." >&2
  exit 127
fi

export MADE_MCP_BACKEND=embedded

# The binary and the plugin update through different commands — `cargo
# install --force` and `/plugin update` — and neither knows about the other.
# A stale plugin with a fresh binary keeps working by luck, because this
# launcher falls back to PATH, so the engine updates silently while the
# launcher and skills stay old. Say it once, on stderr, where a host shows
# server output; never fail on it, since the mismatch is usually harmless.
PLUGIN_MANIFEST="${PLUGIN_ROOT}/.claude-plugin/plugin.json"
if [[ -f "${PLUGIN_MANIFEST}" ]] && command -v python3 >/dev/null 2>&1; then
  PLUGIN_VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' \
    "${PLUGIN_MANIFEST}" 2>/dev/null)"
  BINARY_VERSION="$("${BINARY}" --version 2>/dev/null | sed -E 's/^made-mcp ([^ ]+).*/\1/')"
  if [[ -n "${PLUGIN_VERSION}" && -n "${BINARY_VERSION}" && "${PLUGIN_VERSION}" != "${BINARY_VERSION}" ]]; then
    echo "MADE plugin: plugin files are ${PLUGIN_VERSION}, binary is ${BINARY_VERSION}." >&2
    echo "MADE plugin: they update separately — 'cargo install made-mcp --force' and" >&2
    echo "MADE plugin: '/plugin update made@underpass' — and fixes that live in this" >&2
    echo "MADE plugin: launcher or the skills come with the plugin, not the binary." >&2
  fi
fi

# The embedded backend refuses to start without a state file: where
# ceremonies survive a restart is an operator decision. A plugin has no
# operator to ask, so it picks the conventional per-user state directory
# and says so — an explicit `MADE_MCP_REDB_PATH` always wins.
if [[ -z "${MADE_MCP_REDB_PATH:-}" ]]; then
  USER_STATE_ROOT="${XDG_STATE_HOME:-${HOME}/.local/state}"
  MADE_STATE_ROOT="${USER_STATE_ROOT}/underpass-made"
  mkdir -p "${MADE_STATE_ROOT}"
  export MADE_MCP_REDB_PATH="${MADE_STATE_ROOT}/ceremonies.redb"

  # A store converted to the sqlite engine lives beside the redb one under a
  # name of its own, and the whole point of converting is that both agent
  # hosts open it. Making the operator also hand every host an explicit path
  # would put the shared store out of reach of the install we document. So:
  # asking for sqlite picks the sqlite name, and a converted store already
  # sitting there is used without being asked for. If both files exist the
  # default stays redb — that ambiguity is the operator's to resolve, not
  # something to guess behind their back.
  SQLITE_DEFAULT="${MADE_STATE_ROOT}/ceremonies.sqlite3"
  if [[ -f "${SQLITE_DEFAULT}" && -e "${MADE_MCP_REDB_PATH}" ]]; then
    # Two stores, and picking one silently means writing ceremonies into a
    # file the operator is not reading. `share-store` leaves exactly one; if
    # both are here, something else put them here and only a human knows
    # which is live.
    echo "MADE plugin: two ceremony stores are present and only one can be live:" >&2
    echo "MADE plugin:   ${MADE_MCP_REDB_PATH}" >&2
    echo "MADE plugin:   ${SQLITE_DEFAULT}" >&2
    echo "MADE plugin: move the stale one aside, or name the live one in MADE_MCP_REDB_PATH." >&2
    exit 2
  fi
  if [[ "${MADE_MCP_ENGINE:-}" == "sqlite" ]]; then
    export MADE_MCP_REDB_PATH="${SQLITE_DEFAULT}"
  elif [[ -f "${SQLITE_DEFAULT}" ]]; then
    export MADE_MCP_REDB_PATH="${SQLITE_DEFAULT}"
  fi

  # First start after the rename imports the former default automatically.
  # The legacy file remains read-only evidence; MADE writes a separate file.
  LEGACY_DEFAULT="${USER_STATE_ROOT}/underpass-choreographer/ceremonies.redb"
  if [[ ! -e "${MADE_MCP_REDB_PATH}" && -f "${LEGACY_DEFAULT}" && -z "${MADE_MCP_LEGACY_REDB_PATH:-}" ]]; then
    export MADE_MCP_LEGACY_REDB_PATH="${LEGACY_DEFAULT}"
  fi
fi

# Git Bash hands the native Windows binary an MSYS path it cannot open;
# cygpath converts it when, and only when, we are on such a host.
if command -v cygpath >/dev/null 2>&1; then
  MADE_MCP_REDB_PATH="$(cygpath -w "${MADE_MCP_REDB_PATH}")"
  if [[ -n "${MADE_MCP_LEGACY_REDB_PATH:-}" ]]; then
    MADE_MCP_LEGACY_REDB_PATH="$(cygpath -w "${MADE_MCP_LEGACY_REDB_PATH}")"
  fi
fi

exec "${BINARY}"
