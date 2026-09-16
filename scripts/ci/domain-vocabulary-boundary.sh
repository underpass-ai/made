#!/usr/bin/env bash
set -euo pipefail

# ADR-001: no consuming product's vocabulary enters this repository.
#
# The engine coordinates working sessions for any domain. A term from one
# vertical reaching the core, the public tool surface or a published
# artifact is a defect, not a naming preference — it narrows an engine
# that is only worth publishing separately because it stays general.
#
# The authoring surface is where foreign vocabulary is most likely to
# enter, which is why this is a gate rather than a review note.
#
# ADR-013: no other product's vocabulary enters MADE's own rings either.
#
# Memory is MADE's own bounded context. A memory backend — a kernel, a
# graph, a vector store — is an adapter whose vocabulary maps at the
# boundary through DTOs and never reaches the domain, the use cases or
# the delivery surfaces. A doc comment that cites another product is
# the thin end of that wedge, so it is caught here too.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

# Terms from specific operational verticals. Extend deliberately: every
# entry must be a word the generic engine has no reason to know.
VERTICAL_TERMS='incident|outage|postmortem|on-call|oncall|pagerduty|sev[0-9]|runbook'

VERTICAL_GUARDED_PATHS=(
  'crates/made-core/src'
  'crates/made-api/src'
  'crates/made-app/src'
  'crates/made-mcp/src'
)

# Terms that belong to other products. Every entry must be a word MADE's
# own rings have no reason to say: a backend is named at its adapter's
# boundary and nowhere else.
PRODUCT_TERMS='kmp|kernel|rehydration'

PRODUCT_GUARDED_PATHS=(
  'crates/made-core/src'
  'crates/made-app/src'
  'crates/made-api/src'
  'crates/made-mcp/src'
  'crates/made-embedded/src'
  'crates/made-adapters/src'
)

vertical_failed=0
product_failed=0

# scan <label> <terms-regex> <path>...
# Prints every match under each path and returns 1 when any was found.
scan() {
  local label="$1" terms="$2" found=0
  shift 2
  for path in "$@"; do
    if [[ ! -d "${path}" ]]; then
      echo "domain-vocabulary-boundary: guarded path is missing: ${path}" >&2
      exit 1
    fi

    if matches="$(grep -rniE "${terms}" "${path}")"; then
      echo "domain-vocabulary-boundary: ${label} vocabulary found in ${path}" >&2
      echo "${matches}" >&2
      found=1
    fi
  done
  return $(( found ))
}

scan 'vertical' "${VERTICAL_TERMS}" "${VERTICAL_GUARDED_PATHS[@]}" || vertical_failed=1
scan "other products'" "${PRODUCT_TERMS}" "${PRODUCT_GUARDED_PATHS[@]}" || product_failed=1

if (( vertical_failed )); then
  cat >&2 <<'MSG'

A consuming product names its own concepts in its own repository and maps
them at its boundary, the same way it already supplies evidence sources
and context bundles. See docs/adr/001-working-session-vocabulary.md.
MSG
fi

if (( product_failed )); then
  cat >&2 <<'MSG'

Memory is MADE's own bounded context. Another product's vocabulary — a
memory kernel's, a graph's, a vector store's — maps at its adapter's
boundary and never enters the domain, the use cases or the delivery
surfaces. See docs/adr/013-memory-is-mades-own-bounded-context.md.
MSG
fi

if (( vertical_failed || product_failed )); then
  exit 1
fi

echo "domain-vocabulary-boundary: clean"
