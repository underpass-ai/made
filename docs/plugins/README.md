# MADE plugin

The plugin installs three workflows: setup, ceremony design and ceremony
execution. It declares one local MCP server. It does not create agent workers;
the host executes the procedure through its existing capabilities.

## Repository catalogue

The marketplace is part of `underpass-ai/made`. Its stable entrypoint is the
`marketplace` branch; both catalogues in this checkout are named `made`.

Codex:

```bash
codex plugin marketplace add underpass-ai/made --ref marketplace
codex plugin add made@made
```

Run `made-setup`, then start a new task so the host loads the installed skills
and server. Claude Code:

```text
/plugin marketplace add underpass-ai/made@marketplace
/plugin install made@made
/made:setup
```

**Publication status:** the immutable v0.5.0 release and its current stable
catalogue snapshot use the old `underpass` identity. The commands above target
the repaired catalogue once a subsequent release advances `marketplace`.
Do not move an old release tag to make those commands appear current. To
inspect or test this source checkout before publication:

```bash
codex plugin marketplace add /absolute/path/to/made
codex plugin add made@made
```

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
The default SQLite path is
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`;
`MADE_MCP_STORE_PATH` overrides it. Preserve it across plugin updates.

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
