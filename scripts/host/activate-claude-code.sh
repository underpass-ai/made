#!/bin/bash
# Wake a Claude Code session that a MADE integrator binding points at.
#
# An example for an operator, not supported surface. MADE runs whatever
# `MADE_HOST_ACTIVATION_COMMAND` names, with a cleared environment plus
# MADE_ACTIVATION_DESTINATION, MADE_ACTIVATION_HOST_KIND and
# MADE_ACTIVATION_DELIVERY_ID, and the activation envelope as JSON on
# standard input. Nothing in that envelope selects a command, an
# argument or a session: the destination is the binding's, recorded
# when the host bound itself.
#
# Because the environment is cleared, this script establishes its own.
# Copy it, set the four values below to what your machine actually has,
# and point MADE_HOST_ACTIVATION_COMMAND at your copy.
#
# Exit 0 means the session was handed the envelope. It does not mean
# anybody read it: the host acknowledges through
# `made_acknowledge_integrator_attention`, under its own authorization.
set -euo pipefail

export PATH="${CLAUDE_HOST_PATH:-/usr/local/bin:/usr/bin:/bin}"
export HOME="${CLAUDE_HOST_HOME:-/root}"

envelope="$(cat)"
destination="${MADE_ACTIVATION_DESTINATION:?the engine always sets this}"
delivery="${MADE_ACTIVATION_DELIVERY_ID:?the engine always sets this}"

# `--resume <session-id>` continues the session the binding named;
# `-p` prints one turn and exits. The prompt is the envelope, handed
# over as data the session reads, never as a command to run.
claude --resume "${destination}" -p "$(
    printf 'A MADE delivery is waiting for you: %s\n' "${delivery}"
    printf 'Envelope follows as JSON. Read it, do the work it points at\n'
    printf 'through the MADE tools, then acknowledge the delivery.\n\n'
    printf '%s\n' "${envelope}"
)" >/dev/null

# What MADE keeps as the transport reference: this wake-up's own name.
printf 'claude-code:%s:%s\n' "${destination}" "${delivery}"
