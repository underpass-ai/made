#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"
FIXTURE="${ROOT_DIR}/tests/plugin/made-smoke.jsonl"
RESTART_START_FIXTURE="${ROOT_DIR}/tests/plugin/made-restart-start.jsonl"
RESTART_RECOVERY_FIXTURE="${ROOT_DIR}/tests/plugin/made-restart-recovery.jsonl"

# One state file per run, outside the operator's real state directory: a
# smoke that inherited the launcher's default would read whatever a
# previous run or a developer's own Codex session left behind.
SMOKE_STATE_DIR="$(mktemp -d)"
trap 'rm -rf "${SMOKE_STATE_DIR}"' EXIT
if command -v cygpath >/dev/null 2>&1; then
  # Native Windows binary: it cannot open an MSYS path.
  export MADE_MCP_REDB_PATH="$(cygpath -w "${SMOKE_STATE_DIR}/ceremonies.redb")"
else
  export MADE_MCP_REDB_PATH="${SMOKE_STATE_DIR}/ceremonies.redb"
fi

cd "${ROOT_DIR}"
python3 -m json.tool "${PLUGIN_DIR}/.codex-plugin/plugin.json" >/dev/null
python3 -m json.tool "${PLUGIN_DIR}/.claude-plugin/plugin.json" >/dev/null
python3 -m json.tool "${PLUGIN_DIR}/.mcp.json" >/dev/null

# Both host manifests must carry the same version: a bundle that tells
# Codex one version and Claude Code another is a packaging defect.
python3 - <<'EOF'
import json
import pathlib
import sys

plugin_dir = pathlib.Path("plugins/made")
codex = json.loads((plugin_dir / ".codex-plugin/plugin.json").read_text())["version"]
claude = json.loads((plugin_dir / ".claude-plugin/plugin.json").read_text())["version"]
if codex != claude:
    sys.exit(f"MADE plugin smoke: manifest versions diverge ({codex} != {claude})")
EOF

bash scripts/plugin/build-local-made-plugin.sh

responses="$("${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" <"${FIXTURE}")"

response_contains() {
  local needle="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -Fq -- "${needle}"
  else
    grep -Fq -- "${needle}"
  fi
}

if [[ "$(printf '%s\n' "${responses}" | wc -l)" -ne 8 ]]; then
  echo "MADE plugin smoke expected eight MCP responses" >&2
  exit 1
fi

if ! response_contains '"backend":"embedded"' <<<"${responses}"; then
  echo "MADE plugin smoke did not initialize the embedded backend" >&2
  exit 1
fi

if ! response_contains '"name":"made_run_ceremony"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise the ceremony tool" >&2
  exit 1
fi

if ! response_contains '"name":"made_approve_ceremony_guard"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise incremental authorization" >&2
  exit 1
fi

if ! response_contains '"name":"made_request_ceremony_intervention"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise dynamic interventions" >&2
  exit 1
fi

if ! response_contains '"name":"made_design_ceremony"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise ceremony design" >&2
  exit 1
fi

if ! response_contains '"name":"made_claim_ceremony_step"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise delegated-host claiming" >&2
  exit 1
fi

if ! response_contains '"name":"made_complete_ceremony_step"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise delegated-host completion" >&2
  exit 1
fi

if ! response_contains '"name":"made_generate_ceremony_report"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise ceremony reporting" >&2
  exit 1
fi

if ! response_contains '"name":"made_discover_capabilities"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise capability discovery" >&2
  exit 1
fi

if ! response_contains '"name":"made_get_help"' <<<"${responses}"; then
  echo "MADE plugin smoke did not advertise audience help" >&2
  exit 1
fi

if ! response_contains '"report_generator":true' <<<"${responses}"; then
  echo "MADE plugin discovery did not mark the report generator" >&2
  exit 1
fi

if ! response_contains '"audience":"user"' <<<"${responses}"; then
  echo "MADE plugin smoke did not return user help" >&2
  exit 1
fi

if ! response_contains '"audience":"agent"' <<<"${responses}"; then
  echo "MADE plugin smoke did not return agent help" >&2
  exit 1
fi

if ! response_contains '"delegated_host_sequence"' <<<"${responses}"; then
  echo "MADE plugin agent help omitted delegated-host sequencing" >&2
  exit 1
fi

if ! response_contains 'NoopCeremonyStepHandler' <<<"${responses}"; then
  echo "MADE plugin agent help omitted the no-op handler boundary" >&2
  exit 1
fi

if ! response_contains '"ceremony":"plugin_designed_review"' <<<"${responses}"; then
  echo "MADE plugin smoke did not design the requested ceremony" >&2
  exit 1
fi

if ! response_contains '"published":false' <<<"${responses}"; then
  echo "MADE plugin design unexpectedly published its draft" >&2
  exit 1
fi

if ! response_contains '"started":false' <<<"${responses}"; then
  echo "MADE plugin design unexpectedly started its draft" >&2
  exit 1
fi

if ! response_contains '"completed":true' <<<"${responses}"; then
  echo "MADE plugin smoke did not complete the ceremony" >&2
  exit 1
fi

if ! response_contains '"report_markdown":"# Plugin smoke report' <<<"${responses}"; then
  echo "MADE plugin smoke did not generate the ceremony report" >&2
  exit 1
fi

if ! response_contains '"persisted":false' <<<"${responses}"; then
  echo "MADE plugin report did not expose its host-owned persistence boundary" >&2
  exit 1
fi

# Durability is a separate claim from execution: prove it with two
# processes over one file, not with one process asserting about itself.
started="$("${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" <"${RESTART_START_FIXTURE}")"

if ! response_contains '"ceremony_id":"codex-plugin-restart-smoke"' <<<"${started}"; then
  echo "MADE plugin smoke did not start the published restart ceremony" >&2
  exit 1
fi

recovered="$("${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" <"${RESTART_RECOVERY_FIXTURE}")"

if ! response_contains '"ceremony_id":"codex-plugin-restart-smoke"' <<<"${recovered}" ||
  ! response_contains '"definition_name":"codex_plugin_restart_smoke"' <<<"${recovered}" ||
  ! response_contains '"current_state":"OPEN"' <<<"${recovered}" ||
  ! response_contains '"bound_definition_digest":"' <<<"${recovered}"; then
  echo "MADE plugin ceremony did not survive the process restart" >&2
  exit 1
fi

if response_contains '"isError":true' <<<"${recovered}"; then
  echo "MADE plugin restart recovery reported a tool error" >&2
  exit 1
fi

echo "MADE Codex plugin smoke passed"
