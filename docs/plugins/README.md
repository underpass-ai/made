# MADE plugin

The plugin installs three workflows: setup, ceremony design and ceremony
execution. It declares one local MCP server. It does not create agent workers;
the host executes the procedure through its existing capabilities.

## Install from the repaired checkout today

**The stable `marketplace` branch still points to v0.5.0, whose catalogue is
named `underpass`. It does not yet offer `made@made`.** Both catalogues in the
repaired source checkout are named `made`. To use that identity now, obtain
a current checkout and register its absolute path. For example, on a shell
with Git and Codex:

```bash
git clone https://github.com/underpass-ai/made.git
codex plugin marketplace add "$PWD/made"
codex plugin add made@made
```

If you already have a repaired checkout, use its path instead of cloning it
again. Run `made-setup`, then start a new task. In Claude Code, register that
same local checkout:

```text
/plugin marketplace add /absolute/path/to/made
/plugin install made@made
/made:setup
```

For a binary-only installation today, use the
[manual MCP route](../embedded/README.md) and a checksummed published binary.
Neither route requires registering the old `underpass` catalogue.

## Stable route after publication

The following commands become usable with `made@made` only after a subsequent
release publishes the repaired catalogue and advances the co-located
`marketplace` branch. They are not the current v0.5.0 installation route.

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

Run setup and start a new task after installation. Release tags remain
immutable; moving v0.5.0 would not be a valid way to publish this repair.

Source plugin metadata still pins the released binary version. Installing a
checkout's plugin therefore does not install unreleased engine changes. For
source testing, build that engine and set `MADE_MCP_BIN` to its absolute path.
The runtime's version and capabilities, not the prose or manifest alone,
determine which protocol it implements.

## Setup and verification

Setup runs `scripts/made-install-binary.sh` on Linux/macOS or its PowerShell
counterpart on native Windows. It selects a supported target, downloads the
manifest-matched executable and checksum, verifies SHA-256 and installs
atomically in the plugin's ignored `bin/` directory. Cargo is not required.

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
For engine changes after v0.5.0, use [migrations](../migrations/README.md).
