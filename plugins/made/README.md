# MADE plugin

![MADE — Your business. Your agents. Your architecture. — by Underpass](assets/made-cuatro-voces.png)

The MADE bundle gives Codex and Claude Code setup, design and execution skills
plus one local MCP server backed by SQLite. The host supplies the agents and
tools; MADE coordinates their claims, results and human decisions.

Four things the bundle does by default (unreleased, in `main`):

- lists the `core` tool profile, the ordinary route, and names what it hides
  in `made_discover_capabilities` (`MADE_MCP_TOOL_PROFILE=full` lists all);
- takes what the host may do from your own terminal: setup grants nothing,
  `scripts/made-grant.sh --profile core` allows the ordinary route after
  showing it and asking, `--show` reads what is held, and the agent's session
  cannot grant itself under `core`;
- takes human approvals from your own terminal, `scripts/made-approve.sh
  --ceremony <id> --guard <name> --role <role>`, and refuses them on the
  agent's session (`MADE_HUMAN_APPROVAL_SOURCE=host` restores relayed ones);
- signs exports: `scripts/made-export-evidence.sh --ceremony <id> --out
  <file>`, verified anywhere with `made-mcp verify-evidence`.

## Install the current stable release

The [0.9.0 release](https://github.com/underpass-ai/made/releases/tag/v0.9.0)
is published, and the rolling `marketplace` branch also serves 0.9.0. Pin the
tag to keep the plugin and executable matched until you change its source.

```bash
codex plugin marketplace add underpass-ai/made --ref v0.9.0
codex plugin add made@made
```

```text
/plugin marketplace add underpass-ai/made@v0.9.0
/plugin install made@made
/made:setup
```

Run `made-setup` in Codex or `/made:setup` in Claude Code, then start a new
task. If `made` is already registered on `marketplace`, refresh that catalogue
instead; if it is pinned to an older tag, inspect and replace its source before
adding the pinned source above. Retain the existing SQLite path. The
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
owner boundary; business capabilities still require explicit grants, which a
person issues with `scripts/made-grant.sh` (or `made-mcp grant`) from their
terminal. The launcher refuses missing authorization configuration and never
creates an anonymous owner or a replacement store.

The included `.mcp.json` always represents one `made` registration. It starts
with the POSIX launcher; native Windows setup rewrites that same manifest
atomically to invoke the absolute `scripts\run-embedded-mcp.cmd` path through
`cmd.exe /d /c`. Re-running setup after an update refreshes the path without
creating a duplicate registration or exposing the private cursor key.

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
