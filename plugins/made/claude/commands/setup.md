---
description: Install or update the release-matched MADE MCP executable
argument-hint: "[no arguments]"
---

Run the `made-setup` skill from this plugin. For a normal installed-plugin
setup, refresh the `made` marketplace and reinstall `made@made` before reading
the local manifest; this prevents a stale plugin cache from selecting an older
release. Follow its source-candidate path when `MADE_MCP_BIN` is explicitly
configured; otherwise its installer requires public assets matching the
manifest, verifies the release checksum and installs in the plugin-local
`bin/` directory. Report the verified candidate identity or installer receipt,
then request a new task to load the selected server.
Preserve the existing ceremony store.
