# Wake a host that does not ask

A host bound as an integrator finds its work by asking for it:
`made_await_integrator_attention` waits, bounded, and answers. That is the
default deployment and it needs no configuration.

An operator who wants the engine to knock instead configures one command. MADE
runs that command, and only that command, with the activation envelope as input
— never as instructions. Nothing arriving from a ceremony, a payload or a host
selects a program, an argument or a shell.

## Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `MADE_HOST_ACTIVATION_COMMAND` | unset | Executable and fixed arguments, whitespace separated. Unset means no host is ever woken. |
| `MADE_HOST_ACTIVATION_TIMEOUT_MS` | `10000` | How long the command may take before the wake-up counts as failed. |
| `MADE_HOST_ACTIVATION_MAX_OUTPUT` | `65536` | How many bytes of the command's output are read. |

The executable is resolved once, at startup: against `PATH` when it is a bare
name, as written when it contains a separator, and canonicalised either way. A
command that resolves nowhere, or to something that is not a file, stops the
process. Falling back to waking nobody would leave an operator watching a queue
that never drains with no way to find out why.

Both compositions read these: the service (`made`) and the embedded MCP server.
A host embedding `EmbeddedMade` can wire its own `HostActivationPort` instead,
and `made_discover_capabilities` reports whichever one was composed.

## What the command is given

Each activation spawns the command with a **cleared environment** plus three
variables:

| Variable | Value |
| --- | --- |
| `MADE_ACTIVATION_DESTINATION` | The address the binding recorded — a session id, a queue, whatever the host named. |
| `MADE_ACTIVATION_HOST_KIND` | What kind of host that destination is. |
| `MADE_ACTIVATION_DELIVERY_ID` | The delivery being handed over. |

The activation envelope arrives as JSON on standard input: delivery and binding
ids, the fence, the item, the ceremony, the attention kind and reason, evidence
references and the correlation trail. It carries no secret and no engine
reasoning. What the host does next it does through MADE's own commands, under
its own authorization; an adapter can wake a process and can never grant it a
permission.

A cleared environment means the command has no `PATH`, no `HOME` and no
credentials unless it sets them itself. That is deliberate — a wake-up should
not be a hole through which the engine's environment leaves the process — and it
is why the example scripts below export what they need before calling anything.

## What the exit code means

- **0** — the host was reached. MADE records an activation receipt whose
  transport reference is the first 256 bytes the command printed, control
  characters flattened. Transport, never evidence that anybody acted.
- **anything else** — the delivery failed, with the exit code and the tail of
  standard error as the recorded reason.
- **no answer inside the timeout** — the child is killed and the delivery fails
  with the timeout as its reason.

A failed activation does not lose the work: the delivery stays in the ledger
and is offered again. Processing is acknowledged separately, by the host, with
`made_acknowledge_integrator_attention`.

## Example scripts

`scripts/host/activate-claude-code.sh` and `scripts/host/activate-codex.sh` are
worked examples for two agentic CLIs. They are examples an operator copies and
edits, not supported surface: the flags of a third-party CLI are that project's
to change. Each exports its own `PATH` and `HOME`, resumes the session named by
`MADE_ACTIVATION_DESTINATION` for one non-interactive turn with the envelope as
the prompt, and prints a transport reference.

Verified activations, and the limits found while trying, are recorded in
`artifacts/made/corte7/D2/` in the working repository.
