#!/bin/bash
# Wake a Codex session that a MADE integrator binding points at.
#
# An example for an operator, not supported surface. The contract is
# the one in `activate-claude-code.sh`: a cleared environment plus
# MADE_ACTIVATION_DESTINATION, MADE_ACTIVATION_HOST_KIND and
# MADE_ACTIVATION_DELIVERY_ID, the envelope as JSON on standard input,
# and exit 0 meaning handed over rather than acted upon.
#
# Copy it, set the two values below to what your machine actually has,
# and point MADE_HOST_ACTIVATION_COMMAND at your copy.
set -euo pipefail

export PATH="${CODEX_HOST_PATH:-/usr/local/bin:/usr/bin:/bin}"
export HOME="${CODEX_HOST_HOME:-/root}"

envelope="$(cat)"
destination="${MADE_ACTIVATION_DESTINATION:?the engine always sets this}"
delivery="${MADE_ACTIVATION_DELIVERY_ID:?the engine always sets this}"

# `codex exec resume <session-id> <prompt>` runs one non-interactive
# turn on the session the binding named. The prompt is the envelope,
# handed over as data.
codex exec resume "${destination}" "$(
    printf 'A MADE delivery is waiting for you: %s\n' "${delivery}"
    printf 'Envelope follows as JSON. Read it, do the work it points at\n'
    printf 'through the MADE tools, then acknowledge the delivery.\n\n'
    printf '%s\n' "${envelope}"
)" >/dev/null

printf 'codex:%s:%s\n' "${destination}" "${delivery}"
