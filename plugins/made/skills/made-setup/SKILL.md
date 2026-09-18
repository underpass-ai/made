---
name: made-setup
description: Install or update the release-matched MADE MCP binary after a Codex or Claude Code marketplace install.
---

# Set up MADE

Resolve the plugin root two directories above this skill directory
(`skills/made-setup/../..`). Run its platform adapter:

```bash
<plugin-root>/scripts/made-install-binary.sh
```

On native Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File <plugin-root>\scripts\made-install-binary.ps1
```

The adapter reads the manifest version, chooses a supported target, downloads
the matching executable and SHA-256 file, verifies the digest and installs
atomically into `bin/`. Use this path rather than substituting Cargo; setup
must work without a Rust toolchain and keep the plugin and binary matched.

Report the adapter receipt's installed version and path. After first install
or an update, tell the user to start a new host task so skills and MCP reload.
Do not claim that editing a catalogue or installing the binary has changed
an already-running server. Discovery in the new task verifies the runtime.

Keep the existing `MADE_MCP_STORE_PATH` and SQLite data. Plugin setup does not
migrate a store or move it when catalogue identity changes. A legacy Redb
startup refusal requires its explicit migration path, not an empty replacement.
