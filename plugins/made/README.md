# MADE plugin

![MADE — Your business. Your agents. Your architecture. — by Underpass](assets/made-cuatro-voces.png)

The MADE bundle gives Codex and Claude Code setup, design and execution skills
plus one local MCP server backed by SQLite. The host supplies the agents and
tools; MADE coordinates their claims, results and human decisions.

## Install the current stable release

**The stable route requires the 0.7.8 assets to be public and `marketplace`
to have advanced to that release.** The old `underpass` marketplace
registration cannot provide the current `made@made` entry.

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin marketplace upgrade made
codex plugin add made@made
```

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@made
/made:setup
```

After publication, refresh the marketplace before running `made-setup` in
Codex or `/made:setup` in Claude Code, then start a new task. The
[installation guide](https://github.com/underpass-ai/made/blob/main/docs/plugins/README.md)
covers registration and migration from the old catalogue.

## Test a candidate before publication

Build the reviewed checkout using the
[source instructions](https://github.com/underpass-ai/made/blob/main/docs/embedded/README.md#test-a-source-candidate).
Set `MADE_MCP_BIN` to its absolute executable path in the host launch
environment. Codex can then register that checkout's local `made` catalogue:

```bash
codex plugin marketplace add /absolute/path/to/made
codex plugin add made@made
```

The setup skill verifies that the explicit candidate matches this checkout's
manifest instead of downloading an asset that does not exist yet. Claude's
catalogue source pins the immutable release tag; before that tag exists, use
manual MCP registration with the source-built binary or this checkout's
launcher and `MADE_MCP_BIN`. Do not present that candidate as an installed or
downloaded stable release.

## Runtime

Release setup verifies and installs the binary matching the manifest version in the
plugin's `bin/` directory. The launcher honors `MADE_MCP_BIN`, then uses the
plugin-local executable or a PATH fallback. `MADE_MCP_STORE_PATH` selects the
SQLite file. On POSIX the default is
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`;
on native Windows it is `%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`
(with a `%USERPROFILE%\.local\state` fallback if `LOCALAPPDATA` is absent).
A catalogue rename does not rename that data directory.

Embedded setup automatically configures the four required launch values in an
owner-only per-store host configuration file. Run `made-setup` (or
`/made:setup`) after installing or updating the plugin. It generates the search
cursor key once using a cryptographically secure source, bootstraps the exact
SQLite store idempotently, and reports a redacted receipt. Codex and Claude
share this file when they use the same store. The launcher never generates or
replaces it and fails closed if it is missing, malformed or not private.

Explicit environment values still take precedence. For a manual or isolated
registration, the equivalent values are `MADE_AUTH_POLICY_ID`,
`MADE_AUTH_TRUSTED_HOST_ID`, `MADE_CEREMONY_STORE_ID` and
`MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY`. Before the first start, bootstrap that
exact store explicitly:

```bash
made-mcp bootstrap-authorization "$MADE_MCP_STORE_PATH" \
  --policy-id "$MADE_AUTH_POLICY_ID" \
  --trusted-host-id "$MADE_AUTH_TRUSTED_HOST_ID"
```

Bootstrap is idempotent for the same values. It creates the administrative
owner boundary; business capabilities still require explicit grants. The
launcher refuses missing authorization configuration and never creates an
anonymous owner or a replacement store.

The included `.mcp.json` points to one `made` registration and
`scripts/run-embedded-mcp.sh`. On native
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
its exact identity. The current release requires `claim_fence` on completion;
the older v0.5.0 predates that boundary, so inspect the running schema.

The skills are self-contained inside the bundle:
[setup](skills/made-setup/SKILL.md),
[design](skills/design-ceremony/SKILL.md) and
[run](skills/run-ceremony/SKILL.md).
Repository guides describe [runtime semantics](https://github.com/underpass-ai/made/blob/main/docs/runtime/README.md)
and [validation](https://github.com/underpass-ai/made/blob/main/docs/development/README.md).
