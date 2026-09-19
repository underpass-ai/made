---
name: made-setup
description: Install or update the release-matched MADE MCP binary after a Codex or Claude Code marketplace install.
---

# Set up MADE

Resolve the plugin root two directories above this skill directory
(`skills/made-setup/../..`) and read its manifest version.

For an intentional source-candidate test with `MADE_MCP_BIN` already set,
verify that the path is executable and its `--version` matches the manifest.
Keep that absolute override in the host launch environment, report the
candidate path/version and continue to the launcher and restart checks below.
Do not invoke the release installer or claim a downloaded/checksummed release
in this case. An absent or mismatched override needs correction before startup;
do not silently select another executable.

Without that explicit candidate override, use the release installer only
after the manifest-matched assets are publicly available. In particular, a
0.6.0 candidate manifest does not imply that its assets have been published.
If publication is pending, explain that boundary and direct candidate testing
to a source build plus `MADE_MCP_BIN`; do not substitute a 0.5.0 download.

For a published release, run its platform adapter:

```bash
<plugin-root>/scripts/made-install-binary.sh
```

On native Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File <plugin-root>\scripts\made-install-binary.ps1
```

Native Windows also needs the correct launcher. The bundled `.mcp.json`
points to `scripts/run-embedded-mcp.sh`; the installed EXE does not make that
POSIX script work without Bash. Configure the existing `made` MCP registration
to use the plugin's absolute `scripts\run-embedded-mcp.cmd` path. If the host
requires an executable command, invoke the batch launcher through
`cmd.exe /d /c`. Replace the existing command instead of adding another MADE
server, and verify the override after updates.

The native launcher defaults to
`%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`, or
`%USERPROFILE%\.local\state\underpass-made\ceremonies.sqlite3` when
`LOCALAPPDATA` is absent. Preserve an explicit `MADE_MCP_STORE_PATH`.

The adapter reads the manifest version, chooses a supported target, downloads
the matching executable and SHA-256 file, verifies the digest and installs
atomically into `bin/`. Use this path rather than substituting Cargo; setup
must work without a Rust toolchain and keep the plugin and binary matched.

Report the adapter receipt's installed version and path, or the explicitly
verified source-candidate identity when using `MADE_MCP_BIN`. After first install
or an update, tell the user to start a new host task so skills and MCP reload.
Do not claim that editing a catalogue or installing the binary has changed
an already-running server. Discovery in the new task verifies the runtime.

Keep the existing `MADE_MCP_STORE_PATH` and SQLite data. Plugin setup does not
migrate a store or move it when catalogue identity changes. A legacy Redb
startup refusal requires its explicit migration path, not an empty replacement.

Authorization setup is explicit and precedes server startup. Require the
operator's stable, non-empty policy id and trusted host principal id. Preserve
them in the MCP launch environment as `MADE_AUTH_POLICY_ID` and
`MADE_AUTH_TRUSTED_HOST_ID`; do not generate replacements during an update.
Then run the release-matched binary once against the selected store:

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
