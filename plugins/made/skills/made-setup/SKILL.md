---
name: made-setup
description: Install or update the release-matched MADE MCP binary after a Codex or Claude Code marketplace install.
---

# Set up MADE

## Refresh the marketplace before setup

For a normal installed-plugin setup, refresh the host catalogue before
resolving the plugin root. A host may still be pointing at an older marketplace
snapshot even when a newer MADE release and marketplace commit are public.
Do not configure an older plugin just because its local cache is present.

On Codex, inspect the configured sources and refresh the MADE marketplace:

```bash
codex plugin marketplace list --json
codex plugin marketplace upgrade made
codex plugin list --marketplace made --available --json
codex plugin add made@made --json
```

The marketplace name is `made`; do not substitute a repository name or an
older `underpass` registration. If the refresh or install changes the plugin,
stop and ask the operator to start a new host task before continuing: the
current task may still have the previous plugin root and skill loaded. In the
new task, repeat this preflight and continue with the refreshed plugin.

On Claude Code, use the equivalent host commands before continuing:

```text
/plugin marketplace update made
/plugin install made@made
```

For an intentional source-candidate test with `MADE_MCP_BIN` already set,
skip marketplace refresh and installation. Verify that the explicit candidate
is executable and that its `--version` matches the checkout manifest.

After the preflight (or for a source candidate), resolve the plugin root two
directories above this skill directory (`skills/made-setup/../..`) and read
its manifest version. The effective plugin version must be the version used by
the release-matched binary below.

For an intentional source-candidate test with `MADE_MCP_BIN` already set,
verify that the path is executable and its `--version` matches the manifest.
Keep that absolute override in the host launch environment, report the
candidate path/version and continue to the launcher and restart checks below.
Do not invoke the release installer or claim a downloaded/checksummed release
in this case. An absent or mismatched override needs correction before startup;
do not silently select another executable.

Without that explicit candidate override, use the release installer only
after the manifest-matched assets are publicly available. A candidate or
prerelease manifest does not imply that matching assets or catalogue pointers
have been published. If publication is pending, explain that boundary and
direct candidate testing to a source build plus `MADE_MCP_BIN`; do not
substitute an older stable download while claiming to run the candidate.

For a published release, run its platform adapter:

```bash
<plugin-root>/scripts/made-install-binary.sh
```

On native Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File <plugin-root>\scripts\made-install-binary.ps1
```

Native Windows setup rewrites the bundled `.mcp.json` atomically so the
existing single `made` registration invokes the plugin's absolute
`scripts\run-embedded-mcp.cmd` through `cmd.exe /d /c`. The adapter performs
that host-specific step on every setup/update; do not add a second server or
manually patch Codex or Claude configuration.

The native launcher defaults to
`%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`, or
`%USERPROFILE%\.local\state\underpass-made\ceremonies.sqlite3` when
`LOCALAPPDATA` is absent. Preserve an explicit `MADE_MCP_STORE_PATH`.

The adapter reads the manifest version, chooses a supported target, downloads
the matching executable and SHA-256 file, verifies the digest and installs
atomically into `bin/`. Use this path rather than substituting Cargo; setup
must work without a Rust toolchain and keep the plugin and binary matched.

After the binary is installed (or after an explicit source candidate has been
verified), run the platform configuration adapter exactly once for the selected
host installation:

```bash
<plugin-root>/scripts/made-configure-embedded.sh
```

On native Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File <plugin-root>\scripts\made-configure-embedded.ps1
```

The adapter selects the existing `MADE_MCP_STORE_PATH` or the platform default,
then creates a per-store file under the owner-only private host configuration
directory (`$XDG_CONFIG_HOME/underpass-made/embedded/` or
`%LOCALAPPDATA%\underpass-made\embedded\`). It chooses stable local policy,
trusted-host and store identifiers derived from that store path and generates
the 32-byte search cursor key from the platform cryptographically secure random
source. It writes the file atomically with restrictive permissions, bootstraps
the exact selected SQLite store idempotently, and emits only a redacted receipt.

The POSIX and native Windows launchers read that same file for the existing
single `made` registration. They never generate or replace secret
configuration. On Windows, setup owns only the non-secret launcher command in
the bundled manifest and preserves one registration across updates.
Codex and Claude therefore share the policy/store identity and cursor key when
they point at the same SQLite file. A malformed, unreadable or improperly
protected file fails closed with the `made-setup` repair path. Explicit
environment values continue to take precedence over values in the private
file; an existing file is never silently rotated or overwritten by an override.

Report the adapter receipt's installed version and path, or the explicitly
verified source-candidate identity when using `MADE_MCP_BIN`. After first install
or an update, tell the user to start a new host task so skills and MCP reload.
Do not claim that editing a catalogue or installing the binary has changed
an already-running server. Discovery in the new task verifies the runtime.

Keep the existing `MADE_MCP_STORE_PATH` and SQLite data. Plugin setup does not
migrate a store or move it when catalogue identity changes. A legacy Redb
startup refusal requires its explicit migration path, not an empty replacement.

Authorization setup is explicit and precedes server startup. The configuration
adapter supplies stable, non-empty policy and trusted-host identities, preserves
them in the MCP launch environment as `MADE_AUTH_POLICY_ID` and
`MADE_AUTH_TRUSTED_HOST_ID`, and never generates replacements during an update.
It runs the release-matched binary once against the selected store:

```bash
<made-mcp> bootstrap-authorization <store> \
  --policy-id <policy-id> \
  --trusted-host-id <trusted-host-id>
```

The command is idempotent for the same policy, host and store. Report its
receipt before starting the MCP server. Never create an anonymous owner,
infer a host identity, or fall back to an unprotected store. The trusted host
owner administers authorization. Business actions need explicit grants issued
through `IssueAuthorizationGrant`; setup must not give the owner implicit
business permissions or wildcard scopes.

Search also requires persistent cursor configuration in that same MCP launch
environment. The configuration adapter sets `MADE_CEREMONY_STORE_ID` to a
stable, non-secret identifier for this store and policy, and
`MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY` to exactly 32 cryptographically random
bytes encoded as 64 hexadecimal characters. It generates the key once, writes
it directly to the host's private launch configuration, and restricts that
configuration to its owner. Do not print the key, put it in a transcript,
commit it, or include it in an installation receipt.

Preserve both values across restarts and package updates. Replicas of the same
store share the key and store id; separate stores use different identities.
Losing or deliberately rotating the key invalidates existing search cursors,
which must then restart from the first page. Do not silently generate a new key
each time the launcher runs. Report only whether these values are configured,
then verify discovery and a paginated search with the appropriate explicit
business grant. Keep native host reload verification separate from a launcher
started by a shell test.
