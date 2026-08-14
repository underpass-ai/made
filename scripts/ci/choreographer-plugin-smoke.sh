#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/choreographer"
FIXTURE="${ROOT_DIR}/tests/plugin/choreographer-smoke.jsonl"

cd "${ROOT_DIR}"
python3 -m json.tool "${PLUGIN_DIR}/.codex-plugin/plugin.json" >/dev/null
python3 -m json.tool "${PLUGIN_DIR}/.mcp.json" >/dev/null
bash scripts/plugin/build-local-choreographer-plugin.sh

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
  echo "choreographer plugin smoke expected eight MCP responses" >&2
  exit 1
fi

if ! response_contains '"backend":"embedded"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not initialize the embedded backend" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_run_ceremony"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise the ceremony tool" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_approve_ceremony_guard"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise incremental authorization" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_request_ceremony_intervention"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise dynamic interventions" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_design_ceremony"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise ceremony design" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_generate_ceremony_report"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise ceremony reporting" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_discover_capabilities"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise capability discovery" >&2
  exit 1
fi

if ! response_contains '"name":"choreo_get_help"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not advertise audience help" >&2
  exit 1
fi

if ! response_contains '"report_generator":true' <<<"${responses}"; then
  echo "choreographer plugin discovery did not mark the report generator" >&2
  exit 1
fi

if ! response_contains '"audience":"user"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not return user help" >&2
  exit 1
fi

if ! response_contains '"audience":"agent"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not return agent help" >&2
  exit 1
fi

if ! response_contains '"delegated_host_sequence"' <<<"${responses}"; then
  echo "choreographer plugin agent help omitted delegated-host sequencing" >&2
  exit 1
fi

if ! response_contains '"ceremony":"plugin_designed_review"' <<<"${responses}"; then
  echo "choreographer plugin smoke did not design the requested ceremony" >&2
  exit 1
fi

if ! response_contains '"published":false' <<<"${responses}"; then
  echo "choreographer plugin design unexpectedly published its draft" >&2
  exit 1
fi

if ! response_contains '"started":false' <<<"${responses}"; then
  echo "choreographer plugin design unexpectedly started its draft" >&2
  exit 1
fi

if ! response_contains '"completed":true' <<<"${responses}"; then
  echo "choreographer plugin smoke did not complete the ceremony" >&2
  exit 1
fi

if ! response_contains '"report_markdown":"# Plugin smoke report' <<<"${responses}"; then
  echo "choreographer plugin smoke did not generate the ceremony report" >&2
  exit 1
fi

if ! response_contains '"persisted":false' <<<"${responses}"; then
  echo "choreographer plugin report did not expose its host-owned persistence boundary" >&2
  exit 1
fi

echo "choreographer Codex plugin smoke passed"
