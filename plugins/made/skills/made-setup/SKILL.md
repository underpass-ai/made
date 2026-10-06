---
name: made-setup
description: Install or update the release-matched MADE MCP binary after a Codex or Claude Code marketplace install.
---

# Set up MADE

Decision rules. The scripts carry the mechanics; this skill decides which to
run and what to report.

1. **Refresh the catalogue before anything else.** On Codex:

   ```bash
   codex plugin marketplace list --json
   codex plugin marketplace upgrade made
   codex plugin list --marketplace made --available --json
   codex plugin add made@made --json
   ```

   On Claude Code: `/plugin marketplace update made` then
   `/plugin install made@made`. The marketplace name is `made`, never a
   repository name or an older `underpass` registration. If the refresh
   changed the plugin, stop and ask for a new host task before continuing:
   the running task still has the old plugin root loaded. With `MADE_MCP_BIN`
   set for a source-candidate test, skip the refresh.
2. **Then resolve the plugin root** two directories above this skill and
   read its manifest version. That version is the one the binary must match.
3. **Source candidate or release?** With `MADE_MCP_BIN` set, verify it is
   executable and its `--version` matches the manifest, keep the override in
   the host launch environment, and do not run the installer or claim a
   downloaded release. Without it, run the installer only when the
   manifest-matched assets are public:

   ```bash
   <plugin-root>/scripts/made-install-binary.sh
   ```

   Native Windows: `powershell -NoProfile -ExecutionPolicy Bypass -File
   <plugin-root>\scripts\made-install-binary.ps1`, which also rewrites the
   bundled `.mcp.json` to the absolute `run-embedded-mcp.cmd`.
4. **Configure the store once:**

   ```bash
   <plugin-root>/scripts/made-configure-embedded.sh
   ```

   (`made-configure-embedded.ps1` on native Windows.) It keeps the existing
   `MADE_MCP_STORE_PATH` or the platform default, writes one owner-only
   per-store file with the policy, trusted-host and store identities and the
   32-byte search cursor key, bootstraps authorization idempotently, creates
   one evidence signing key per store, and prints a redacted receipt plus the
   evidence public key. Never print, commit or transcribe the cursor key or
   the signing key; report only that they are configured.
5. **Report the receipt and ask for a new task.** Installed version and path,
   or the verified candidate identity; the store path; that authorization and
   the cursor key are configured; the evidence public key. Editing a
   catalogue or installing a binary changes no running server: discovery in
   the new task is what verifies the runtime.
6. **Say what the launcher will do.** New sessions list the `core` tool
   profile and take human approvals from the person's terminal
   (`scripts/made-approve.sh`). Both are overridden by
   `MADE_MCP_TOOL_PROFILE` and `MADE_HUMAN_APPROVAL_SOURCE` in the host's MCP
   launch environment.

Never create an anonymous owner, infer a host identity, fall back to an
unprotected store, rotate a key silently, migrate or move a store, or remove
another product's registration. A legacy Redb store needs its explicit
migration path, not an empty replacement. Business actions still need
explicit grants; setup gives the owner no implicit permissions.
