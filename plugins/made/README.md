# MADE plugin

The MADE bundle gives Codex and Claude Code setup, design and execution skills
plus one local MCP server backed by SQLite. The host supplies the agents and
tools; MADE coordinates their claims, results and human decisions.

## Install today

**The stable `marketplace` branch still carries the v0.5.0 catalogue named
`underpass`; `made@made` is not available from that snapshot yet.** Use a local
checkout containing the repaired `made` catalogue:

```bash
codex plugin marketplace add /absolute/path/to/made
codex plugin add made@made
```

Run `made-setup`, then start a new task. In Claude Code:

```text
/plugin marketplace add /absolute/path/to/made
/plugin install made@made
/made:setup
```

The [installation guide](https://github.com/underpass-ai/made/blob/main/docs/plugins/README.md)
shows how to obtain the repaired checkout. A
[manual binary installation](https://github.com/underpass-ai/made/blob/main/docs/embedded/README.md)
is also available today, without registering the old catalogue.

After a later release publishes the repair and advances `marketplace`, the
stable routes will be the following. These commands are not usable with
`made@made` against today's v0.5.0 snapshot:

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@made
```

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@made
/made:setup
```

## Runtime

Setup verifies and installs the binary matching the manifest version in the
plugin's `bin/` directory. The launcher honors `MADE_MCP_BIN`, then uses the
plugin-local executable or a PATH fallback. `MADE_MCP_STORE_PATH` selects the
SQLite file. On POSIX the default is
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`;
on native Windows it is `%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`
(with a `%USERPROFILE%\.local\state` fallback if `LOCALAPPDATA` is absent).
A catalogue rename does not rename that data directory.

The included `.mcp.json` points to `scripts/run-embedded-mcp.sh`. On native
Windows without Bash, replace that command in the existing `made` MCP
registration with the plugin's absolute `scripts\run-embedded-mcp.cmd` path
(or invoke it through `cmd.exe /d /c` if required by the host). Installing the
EXE alone is not enough. Keep one MADE registration and recheck the command
after updates. The [setup skill](skills/made-setup/SKILL.md) includes this
platform step.

In a fresh task, use `made_discover_capabilities` and `made_get_help` to
inspect the actual running version/backend. `tools/list` is the authority for
request schemas. Publish definitions before starting resumable sessions.
No-op handler success proves protocol wiring only. For delegated work,
retain the accepted claim response, perform the real work and complete using
its exact identity. This source tree requires `claim_fence` on completion; the
published v0.5.0 binary predates that boundary, so inspect the running schema.

The skills are self-contained inside the bundle:
[setup](skills/made-setup/SKILL.md),
[design](skills/design-ceremony/SKILL.md) and
[run](skills/run-ceremony/SKILL.md).
Repository guides describe [runtime semantics](https://github.com/underpass-ai/made/blob/main/docs/runtime/README.md)
and [validation](https://github.com/underpass-ai/made/blob/main/docs/development/README.md).
