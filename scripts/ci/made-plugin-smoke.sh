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
mkdir -p "${ROOT_DIR}/tmp"
SMOKE_STATE_DIR="$(mktemp -d "${ROOT_DIR}/tmp/plugin-smoke.XXXXXX")"
trap 'rm -rf "${SMOKE_STATE_DIR}"' EXIT
if command -v cygpath >/dev/null 2>&1; then
  # Native Windows binary: it cannot open an MSYS path.
  export MADE_MCP_STORE_PATH="$(cygpath -w "${SMOKE_STATE_DIR}/ceremonies.sqlite3")"
else
  export MADE_MCP_STORE_PATH="${SMOKE_STATE_DIR}/ceremonies.sqlite3"
fi

# The smoke asserts tools from every capability group, so it lists the full
# catalog; the launcher's own default (`core`) is checked separately below.
export MADE_MCP_TOOL_PROFILE=full

cd "${ROOT_DIR}"
python3 -m json.tool "${PLUGIN_DIR}/.codex-plugin/plugin.json" >/dev/null
python3 -m json.tool "${PLUGIN_DIR}/.claude-plugin/plugin.json" >/dev/null
python3 -m json.tool "${PLUGIN_DIR}/.mcp.json" >/dev/null
bash tests/plugin/build-local-made-plugin-target-dir.sh

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

CI_BINARY="${PLUGIN_DIR}/bin/made-mcp"
[[ -x "${CI_BINARY}" ]] || CI_BINARY="${CI_BINARY}.exe"
source "${ROOT_DIR}/tests/plugin/setup-authorization.sh" "${CI_BINARY}"
"${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" \
  <"${ROOT_DIR}/tests/plugin/made-smoke-authorization.jsonl" \
  >"${SMOKE_STATE_DIR}/authorization.jsonl"
python3 - "${SMOKE_STATE_DIR}/authorization.jsonl" <<'PY'
import json
import pathlib
import sys

response = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert not response.get("error"), response
assert not response["result"].get("isError"), response
PY

# A legacy default must stop the SQLite-only launcher before it creates a
# second live store. The message is the supported, release-pinned migration
# route; the current binary never opens the old bytes.
LEGACY_STATE_HOME="${SMOKE_STATE_DIR}/legacy-state"
LEGACY_STATE_DIR="${LEGACY_STATE_HOME}/underpass-made"
mkdir -p "${LEGACY_STATE_DIR}"
printf 'redb legacy smoke' >"${LEGACY_STATE_DIR}/ceremonies.redb"
if legacy_refusal="$(
  env -u MADE_MCP_STORE_PATH \
    XDG_STATE_HOME="${LEGACY_STATE_HOME}" \
    "${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" </dev/null 2>&1
)"; then
  echo "MADE plugin smoke opened a legacy Redb default" >&2
  exit 1
fi
if [[ "${legacy_refusal}" != *"v0.2.0"* || "${legacy_refusal}" != *"share-store"* ]]; then
  echo "MADE plugin legacy refusal omitted the supported conversion command" >&2
  exit 1
fi
if [[ -e "${LEGACY_STATE_DIR}/ceremonies.sqlite3" ]]; then
  echo "MADE plugin legacy refusal created a split-brain SQLite store" >&2
  exit 1
fi

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

# Without an explicit profile the launcher lists the ordinary route only,
# says so in discovery, and refuses a hidden tool by name instead of
# pretending it does not exist.
CORE_LIST="$(env -u MADE_MCP_TOOL_PROFILE "${PLUGIN_DIR}/scripts/run-embedded-mcp.sh" <<'EOF_CORE'
{"jsonrpc":"2.0","id":1,"method":"tools/list"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"made_discover_capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"made_await_integrator_attention","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"made_approve_ceremony_guard","arguments":{"ceremony_id":"codex-plugin-restart-smoke","guard_name":"none","role_id":"SYSTEM","role_kind":"human"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"made_cancel_ceremony","arguments":{"ceremony_id":"codex-plugin-restart-smoke","actor_id":"plugin-ci-host","actor_kind":"service","reason":"smoke: an action the grant never named"}}}
EOF_CORE
)"
if ! response_contains '"name":"made_design_ceremony"' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: the core profile does not list made_design_ceremony" >&2
  exit 1
fi
if response_contains '"name":"made_await_integrator_attention"' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: the core profile lists an integrator-loop tool" >&2
  exit 1
fi
if ! response_contains '"tool_profile":{' <<<"${CORE_LIST}" || ! response_contains '"name":"core"' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: discovery does not report the core profile" >&2
  exit 1
fi
if ! response_contains 'not listed under tool profile `core`' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: a hidden tool was not refused by name" >&2
  exit 1
fi
# The launcher's other default: a human approval is refused on the MCP
# session and pointed at the terminal command.
if ! response_contains 'made-mcp approve-guard' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: the launcher did not refuse an MCP human approval in terminal mode" >&2
  exit 1
fi
# Bootstrap grants nothing, and the smoke's grant never named cancellation:
# the refusal names the action, the host and the terminal command, and
# discovery said which listed tools had no grant before anything was called.
if ! response_contains 'denied `cancel_ceremony`: principal `plugin-ci-host` holds no live grant' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: an ungranted action was not refused with its action and principal named" >&2
  exit 1
fi
if ! response_contains 'made-mcp grant <store> --profile core' <<<"${CORE_LIST}" || ! response_contains 'scripts/made-grant.sh' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: the refusal did not name the terminal command, or discovery the plugin script" >&2
  exit 1
fi
if ! response_contains '"authorization":{' <<<"${CORE_LIST}" || ! response_contains '"listed_tools_without_grant":[' <<<"${CORE_LIST}"; then
  echo "MADE plugin smoke: discovery does not report the host's authority" >&2
  exit 1
fi
# The core listing is hundreds of kilobytes: a file, not an argument.
printf '%s\n' "${CORE_LIST}" >"${SMOKE_STATE_DIR}/core-list.jsonl"
if ! python3 - "${SMOKE_STATE_DIR}/core-list.jsonl" <<'PY_AUTHORITY'
import json, pathlib, sys
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    if not line.strip():
        continue
    response = json.loads(line)
    if response.get("id") != 2:
        continue
    authority = response["result"]["structuredContent"]["authorization"]
    assert authority["principal"] == "plugin-ci-host", authority
    assert authority["owner"] is True, authority
    assert "design_ceremony" in authority["granted_actions"], authority
    assert "made_cancel_ceremony" in authority["listed_tools_without_grant"], authority
    assert "made_design_ceremony" not in authority["listed_tools_without_grant"], authority
    assert "made_discover_capabilities" not in authority["listed_tools_without_grant"], authority
    break
else:
    sys.exit("discovery response not found")
PY_AUTHORITY
then
  echo "MADE plugin smoke: discovery's authority entry disagrees with the grant the smoke issued" >&2
  exit 1
fi

# A grant is a person's decision: the wrapper refuses piped input before the
# binary is reached, and `--show` reads what the host holds without asking.
if "${PLUGIN_DIR}/scripts/made-grant.sh" --profile core </dev/null >/dev/null 2>"${SMOKE_STATE_DIR}/grant-refusal.txt"; then
  echo "MADE plugin smoke: made-grant.sh accepted piped input" >&2
  exit 1
fi
if ! grep -Fq "interactive terminal" "${SMOKE_STATE_DIR}/grant-refusal.txt"; then
  echo "MADE plugin smoke: made-grant.sh did not say why it refused" >&2
  cat "${SMOKE_STATE_DIR}/grant-refusal.txt" >&2
  exit 1
fi
SHOWN="$("${PLUGIN_DIR}/scripts/made-grant.sh" --show </dev/null)"
if ! response_contains 'Grant `plugin-ci-smoke` (live)' <<<"${SHOWN}" || ! response_contains 'owns it and may administer grants' <<<"${SHOWN}"; then
  echo "MADE plugin smoke: made-grant.sh --show did not report the smoke's grant" >&2
  printf '%s\n' "${SHOWN}" >&2
  exit 1
fi
echo "MADE plugin grant channel smoke passed"

