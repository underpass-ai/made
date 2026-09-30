# MADE plugin

The plugin installs three workflows: setup, ceremony design and ceremony
execution. It declares one local MCP server. It does not create agent workers;
the host executes the procedure through its existing capabilities.

## Install 0.9.0

The [0.9.0 release](https://github.com/underpass-ai/made/releases/tag/v0.9.0)
contains checksummed binaries and plugin bundles. Pin the catalogue to that
tag for a release-matched install.

Codex:

```bash
codex plugin marketplace add underpass-ai/made --ref v0.9.0
codex plugin add made@made
```

Claude Code:

```text
/plugin marketplace add underpass-ai/made@v0.9.0
/plugin install made@made
/made:setup
```

Run `made-setup` in Codex (or `/made:setup` in Claude Code), then start a new
task. The [manual MCP route](../embedded/README.md) is also available with a
checksummed binary or the matching Cargo release.

### Existing installations and the rolling catalogue

Inspect the registered source and available version before updating:

```bash
codex plugin marketplace list --json
codex plugin marketplace upgrade made
codex plugin list --marketplace made --available --json
```

As of 2026-09-30, the remote `marketplace` branch serves **0.9.0**. A
registration on that branch reaches 0.9.0 by refreshing it: run the upgrade
above, reinstall `made@made`, rerun setup and start a new task. A registration
pinned to an older tag, such as `v0.8.0`, stays there; replace its source with
`v0.9.0` or `marketplace` using the host's marketplace management, or use
[manual MCP setup](../embedded/README.md). Keep the same SQLite path and one
active MADE server. An existing `made` catalogue must be replaced before
adding a different source under that name.

A tag-pinned catalogue stays at that release until its source is changed.
The rolling `--ref marketplace` route can track later releases after their
assets publish and the branch advances. An installed cache, a source manifest
and a running MCP process can have different versions: verify discovery in a
new task after installing and running setup.

## Test a local candidate

Build the reviewed checkout using the
[source instructions](../embedded/README.md#test-a-source-candidate).
Set `MADE_MCP_BIN` to that executable's absolute path in the host launch
environment, then register the local Codex catalogue:

```bash
codex plugin marketplace add /absolute/path/to/made
codex plugin add made@made
```

For an unpublished candidate, do not use the release download installer.
With an explicit `MADE_MCP_BIN`, the setup skill verifies the chosen candidate
instead. Start a new task and check that discovery reports the intended build.

The Claude catalogue pins an immutable tag even when the catalogue itself is
registered from a local path. That tag must exist before its plugin install
can resolve. For Claude candidate testing before publication, use the
[manual MCP registration](../embedded/README.md#register-mcp) with the built
binary or the checkout's launcher and explicit `MADE_MCP_BIN`.

The runtime's version and capabilities determine its contract. Neither the
source manifest nor a PATH fallback proves that unreleased engine changes
are installed. Preserve the same SQLite path across candidate testing and
later release installation.

## Setup and verification

For a published release, setup runs `scripts/made-install-binary.sh` on Linux/macOS or its PowerShell
counterpart on native Windows. It selects a supported target, downloads the
manifest-matched executable and checksum, verifies SHA-256 and installs
atomically in the plugin's ignored `bin/` directory. Cargo is not required.

The same setup flow then runs `scripts/made-configure-embedded.sh` (or
`made-configure-embedded.ps1` on native Windows). It creates one owner-only,
per-store host configuration file, generates the 32-byte search cursor HMAC key
once from a cryptographically secure source, bootstraps the selected SQLite
store idempotently and emits a redacted receipt. The launchers only read this
file, so Codex and Claude share the same policy/store identity and cursor key
when their one `made` registration points at the same store. A malformed,
unreadable or non-private file fails closed and directs the operator back to
setup. Explicit environment overrides remain supported and take precedence.

The launcher chooses `MADE_MCP_BIN` when supplied, otherwise the plugin-local
binary, then a PATH fallback. A PATH fallback can differ from the plugin
version. Setup restores a matched binary; it does not migrate session data.

In a fresh task, call `made_discover_capabilities`, then `made_get_help`.
Check the active backend and version. Use `tools/list` for exact tool schemas.
The POSIX launcher defaults to
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`.
The native Windows launcher defaults to
`%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`, falling back to
`%USERPROFILE%\.local\state\underpass-made\ceremonies.sqlite3` when
`LOCALAPPDATA` is absent. `MADE_MCP_STORE_PATH` overrides either default.
Preserve the selected path across plugin updates.

### Native Windows launcher

The bundled `.mcp.json` starts with the POSIX launcher. Native Windows setup
rewrites that same manifest atomically to invoke the installed plugin's
absolute `scripts\run-embedded-mcp.cmd` path through `cmd.exe /d /c`.
Installing `made-mcp.exe` alone does not perform this setup step.

Rerun `made-setup` after an update so the launcher path matches the installed
bundle, then start a new task and verify discovery. Setup preserves one `made`
registration; do not add a second server or manually patch host configuration.
The CMD launcher sets embedded mode and the Windows state default above,
preserving an explicit `MADE_MCP_STORE_PATH`.

## Replace an old registration

First inspect the catalogue source and installed plugin:

```bash
codex plugin marketplace list --json
codex plugin list --marketplace underpass --available --json
```

A catalogue name is not a repository name. The former collision between
MADE's and KMP's `underpass` catalogues cannot be fixed by registration order.
Only remove an old registration after identifying its source. When replacing
an installed `made@underpass`, remove that old plugin registration before
activating `made@made`, keeping the same SQLite path. Do not remove another
product's data or assume every `underpass` registration belongs to MADE.

Update the repaired catalogue with
`codex plugin marketplace upgrade made`, reinstall `made@made`, rerun setup and
start a new task.
A host's curated public plugin directory is a separate publication channel;
registering this repository does not submit it there.

For manual configuration, use [local MCP setup](../embedded/README.md).
For client and store upgrades through 0.9.0, use [migrations](../migrations/README.md).
