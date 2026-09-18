# MADE plugin

The MADE bundle gives Codex and Claude Code setup, design and execution skills
plus one local MCP server backed by SQLite. The host supplies the agents and
tools; MADE coordinates their claims, results and human decisions.

## Install

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@made
```

Run `made-setup`, then start a new task. In Claude Code:

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@made
/made:setup
```

These are the repaired catalogue coordinates. The published v0.5.0 snapshot
still carries `underpass`; `made` becomes the stable branch identity when the
next release publishes. See the [installation and migration guide](https://github.com/underpass-ai/made/blob/main/docs/plugins/README.md)
for checking that boundary and testing an unreleased checkout.

## Runtime

Setup verifies and installs the binary matching the manifest version in the
plugin's `bin/` directory. The launcher honors `MADE_MCP_BIN`, then uses the
plugin-local executable or a PATH fallback. `MADE_MCP_STORE_PATH` selects the
SQLite file; its default is
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`.
A catalogue rename does not rename that data directory.

In a fresh task, use `made_discover_capabilities` and `made_get_help` to
inspect the actual running version/backend. `tools/list` is the authority for
request schemas. Publish definitions before starting resumable sessions.
No-op handler success proves protocol wiring only. For delegated work,
retain the accepted claim response, perform the real work and complete using
its exact identity; newer builds require `claim_fence`.

The skills are self-contained inside the bundle:
[setup](skills/made-setup/SKILL.md),
[design](skills/design-ceremony/SKILL.md) and
[run](skills/run-ceremony/SKILL.md).
Repository guides describe [runtime semantics](https://github.com/underpass-ai/made/blob/main/docs/runtime/README.md)
and [validation](https://github.com/underpass-ai/made/blob/main/docs/development/README.md).
