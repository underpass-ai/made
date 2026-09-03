---
name: made-setup
description: Install or update the release-matched MADE MCP binary after a Codex or Claude Code marketplace install.
---

# MADE setup

Resolve the plugin root as two directories above this `SKILL.md`.

On Linux or macOS, run:

```bash
<plugin-root>/scripts/made-install-binary.sh
```

On native Windows, run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File <plugin-root>\scripts\made-install-binary.ps1
```

The adapter reads the plugin manifest version, downloads the matching
standalone executable and checksum from that immutable GitHub Release,
verifies SHA-256, and installs atomically into the plugin's ignored `bin/`
directory. Do not replace this with `cargo install`: marketplace setup must
not require a Rust toolchain, and the plugin-local executable keeps its files
and engine on the same release.

After setup, report the installed version and path from the adapter receipt.
Tell the user to start a new host thread when this was a first install or an
update, because MCP servers and skills are loaded at thread startup.
