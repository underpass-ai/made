# MADE plugin

The plugin installs three workflows: setup, ceremony design and ceremony
execution. It declares one local MCP server. It does not create agent workers;
the host executes the procedure through its existing capabilities.

## Install the 0.6.0 release

**The stable route requires public 0.6.0 assets and `marketplace` advanced to
that release.** The v0.5.0 snapshot uses `underpass` and cannot provide
`made@made`. Before the new publication completes, use the candidate route
below; bumping a manifest does not make its download exist.

Codex:

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@made
```

Claude Code:

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@made
/made:setup
```

After publication, run `made-setup` in Codex (or `/made:setup` in Claude Code)
and start a new task. The [manual MCP route](../embedded/README.md) is also
available with a checksummed binary or the matching Cargo release. Moving
v0.5.0 would not be a valid way to publish the catalogue repair.

## Test a local candidate

Build the reviewed 0.6.0 checkout using the
[source instructions](../embedded/README.md#test-a-source-candidate).
Set `MADE_MCP_BIN` to that executable's absolute path in the host launch
environment, then register the local Codex catalogue:

```bash
codex plugin marketplace add /absolute/path/to/made
codex plugin add made@made
```

Do not run the release download installer while 0.6.0 assets are absent.
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

The bundled `.mcp.json` declares the POSIX
`scripts/run-embedded-mcp.sh` launcher. Installing `made-mcp.exe` alone does
not make that command runnable on a native Windows host without Bash.
Configure the existing `made` MCP registration to launch the plugin's
`scripts\run-embedded-mcp.cmd` instead. Use the installed plugin's absolute
path; a host that requires an executable command can invoke it through
`cmd.exe /d /c <absolute-path-to-run-embedded-mcp.cmd>`.

Replace the launch command rather than adding a second MADE server. Check the
Windows command again after a plugin update, then open a new task and verify
discovery. The CMD launcher sets embedded mode and the Windows state default
above; it preserves an explicit `MADE_MCP_STORE_PATH`.

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
`codex plugin marketplace upgrade made`, rerun setup and start a new task.
A host's curated public plugin directory is a separate publication channel;
registering this repository does not submit it there.

For manual configuration, use [local MCP setup](../embedded/README.md).
For the 0.5.x → 0.6.0 upgrade, use [migrations](../migrations/README.md).
