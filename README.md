<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/made-emblem-dark.svg"><img src="docs/assets/made-emblem-light.svg" width="804" alt="MADE"></picture></p>
<p align="center"><strong>A verifiable record, and a real human gate, for work done by agents · by Underpass</strong></p>

MADE does not run your agents. Claude Code, Codex or your own workers do the
work. MADE decides who may act next, records every accepted result in a
hash-chained journal, and holds the places where a person must decide. It runs
on your machine: one plugin, one MCP process, one SQLite file. No account, no
service, no model provider.

## What one review leaves behind

An agent reviewed a change under a three-seat procedure: the author proposes,
the reviewer challenges, the tech lead approves. This is the record MADE kept,
read back with `made_read_ceremony_events`:

| # | Fact | Actor | Sealed under |
|:--|:--|:--|:--|
| 1 | ceremony started from the published definition `pr_review@1.0` | service | authorization decision, policy v4 |
| 2–3 | step `propose` claimed and completed, output `{proposal, rationale}` | agent, as `AUTHOR` | claim fence `9c56…c248` |
| 4 | transition `propose_completed` | agent | guard `propose_completed` satisfied |
| 5–6 | step `challenge` claimed and completed, output `{risks, verdict}` | agent, as `REVIEWER` | claim fence `bcbf…7422` |
| 7 | guard `human_approved_outcome` approved | **human**, as `TECH_LEAD` | the person's terminal, not the agent's session |
| 8–9 | transition `approve_outcome`, ceremony completed | human | guards `challenge_completed` + `human_approved_outcome` |

`made_verify_ceremony_journal` answers `intact: true, record_count: 9`. Every
record carries the authorization decision that admitted it, the actor's
declared kind and the SHA-256 of the record before it. The agent asked to
approve on the person's behalf and was refused; the person ran
`scripts/made-approve.sh` and the approval is the only human-kind fact in the
chain.

Hand the record to someone without the store:

```bash
plugins/made/scripts/made-export-evidence.sh --ceremony pr-1 --out pr-1.evidence.json
made-mcp verify-evidence pr-1.evidence.json --public-key <the key setup printed>
# chain: intact · head: matches · signature: valid · verdict: sound
```

Terminal approvals, terminal grants, tool profiles and evidence bundles are
in `main` and unreleased; the published releases below record relayed
approvals, grant through MCP and list the full catalog. See
[Unreleased](CHANGELOG.md#unreleased).

## Start locally

These guides describe **0.9.0**. Install the plugin from the published tag
so its skills, setup scripts and executable use the same release:

```bash
codex plugin marketplace add underpass-ai/made --ref v0.9.0
codex plugin add made@made
```

Run `made-setup`, then start a new task. Setup downloads and verifies the
release binary, configures authorization and a persistent search cursor key,
and bootstraps the local SQLite store. Cargo is not required. The
[plugin guide](docs/plugins/README.md) covers Claude Code, Windows and updates.
The rolling `marketplace` branch also serves 0.9.0, so an existing
registration on that branch updates by refreshing it.

Bootstrap grants nothing: the agent's session holds no authority until you
give it some, from your own terminal, and it cannot grant itself (in `main`,
unreleased; the published releases grant through MCP instead):

```bash
plugins/made/scripts/made-grant.sh --profile core   # the ordinary route, 22 actions, shown and confirmed
```

Discovery then lists what the host holds under `authorization`, and a
refused call names the action, the principal and how a grant is issued rather
than a bare decision id.

For manual MCP registration, install the matching binary:

```bash
cargo install made-mcp --version 0.9.0 --locked
made-mcp --version
```

Then follow the [local setup guide](docs/embedded/README.md#register-mcp)
to bootstrap authorization and register the command with all required
configuration. Checksummed
[release binaries](https://github.com/underpass-ai/made/releases/tag/v0.9.0)
avoid the Rust toolchain. To embed the engine in Rust, start with the
[complete library example](docs/embedded/rust.md).

## Give the host a procedure

> Design a change review: an author proposes a solution, a reviewer challenges
> it, and I approve the final result.

Design produces a draft. Publish its reviewed definition, then start a
session from that published name and version so it can resume after restart.
For each delegated step, the host claims the work, performs it and submits
its output with the claim's fence. A claim alone performs nothing; MADE never
turns an agent's statement into a human decision.

Ceremonies declare sequential or concurrent work, role eligibility, human
guards, retry policies, bounded repetition, transition budgets and context
writes. Eight coordination shapes ship as patterns (broadcast and collect,
group chat, maker and checker, handoff, magentic, advisor, sequential,
concurrent). Several published ceremonies compose into an agentic system with
roles, participants and bounded loops.

## What MADE is not

- **Not an agent framework.** It creates no agents and calls no model in the
  local edition. Hosts fan work out; MADE arbitrates claims through durable
  leases and records what came back.
- **Not a workflow engine that trusts its caller.** Authorization is explicit
  per action and per scope, idempotency keys and fences are on every mutation,
  and a human guard waits for a channel the agent does not control.
- **Not an audit log you have to believe.** The journal verifies itself, and a
  signed export verifies on a machine that has never seen the store. What it
  cannot prove it declares: a completed step is still the host's claim, and a
  terminal approval proves the terminal, not who sat at it.

## Pick your path

| Need | Start here |
|:--|:--|
| Use MADE through a coding agent | [Plugin](docs/plugins/README.md) · [Manual MCP setup](docs/embedded/README.md) |
| Put the engine inside a Rust application | [Embedding](docs/embedded/rust.md) |
| Design a reusable procedure | [Ceremony authoring](docs/authoring/README.md) |
| Execute, resume or inspect a session | [Runtime contract](docs/runtime/README.md) |
| Let a person decide, and prove it | [Humans and interventions](docs/runtime/README.md#humans-and-participant-interventions) |
| Decide what the agent's session may do | [Grants from your terminal](docs/embedded/README.md#who-may-act) |
| Hand the record to someone without the store | [Evidence bundles](docs/operations/evidence-bundles.md) |
| Hand off a definition, question a working agent, compose a system, drive a whole scope | [0.8 capabilities](docs/corte7/README.md) |
| Operate a shared service | [Kubernetes](docs/operations/deploy-kubernetes.md) |
| Build or extend MADE | [Architecture](docs/architecture/README.md) · [Development](docs/development/README.md) |

MADE is pre-1.0. Upgrades from older releases can require client and store
changes, including completion fences and compatible snapshot/event readers.
Use the running server's `tools/list` and
`made_discover_capabilities` to check the installed surface, the active tool
profile and where human approvals are accepted. See
[migrations](docs/migrations/README.md), [release history](CHANGELOG.md) and
[documentation home](docs/index.md).

[Apache-2.0](LICENSE). Copyright © 2026 Tirso García Ibáñez.
Part of [Underpass AI](https://underpassai.com).
