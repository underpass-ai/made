#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

test -z "$(git status --porcelain)" || {
  echo "working tree must be clean before the C6 scenario gate" >&2
  exit 1
}

scripts/ci/corte6-integration-gate.sh
printf '%s\n' 'C6 scenarios are contract-ready on the tested local tree:'
printf '%s\n' '  software-change: workers + local execution + connectors + receipts'
printf '%s\n' '  incident: scheduler + provider observations + recovery/reconciliation'
printf '%s\n' '  research: reproducible evaluation corpus + MCP ceremony guide'
printf '%s\n' '  operation: dashboard + retention/GC preview + identity transition contract'
printf '%s\n' 'External PostgreSQL/Kubernetes/provider campaigns remain prerequisites, not claims.'
