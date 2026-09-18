---
description: Install or update the release-matched MADE MCP executable
argument-hint: "[no arguments]"
---

Run the `made-setup` skill from this plugin. Follow its source-candidate path
when `MADE_MCP_BIN` is explicitly configured; otherwise its installer requires
public assets matching the manifest, verifies the release checksum and installs
in the plugin-local `bin/` directory. Report the verified candidate identity or
installer receipt, then request a new task to load the selected server.
Preserve the existing ceremony store.
