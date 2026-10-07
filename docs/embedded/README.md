# Local MADE

Use the [plugin](../plugins/README.md) for bundled skills and a release-matched
executable. Use the manual route below when you manage MCP registration.
For an in-process engine, see [Rust embedding](rust.md).

## Install a binary

These instructions target the published **0.9.0** release. For a development
checkout, use the source route below and verify its manifest version before
registering the binary.

```bash
cargo install made-mcp --version 0.9.0 --locked
made-mcp --version
```

Cargo's default features include the embedded and gRPC backends. Alternatively
choose the 0.9.0 executable and its SHA-256 file for your platform from the
[0.9.0 release](https://github.com/underpass-ai/made/releases/tag/v0.9.0).
Check the digest before running it. The plugin's setup adapter automates this
for Linux x86_64/arm64, macOS arm64 and Windows x86_64.

## Test a source candidate

From a reviewed checkout, build an embedded-only executable. In a
POSIX shell:

```bash
cargo build -p made-mcp --release --locked --no-default-features --features embedded
MADE_CANDIDATE_TARGET="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
export MADE_MCP_BIN="${MADE_CANDIDATE_TARGET}/release/made-mcp"
"$MADE_MCP_BIN" --version
```

Use the reported Cargo target directory; it may be outside the checkout. On
native Windows the executable ends in `made-mcp.exe`. The build must report
the version in that checkout's `Cargo.toml` and plugin manifest (0.9.0 at the
release tag). Register this absolute path manually, or set `MADE_MCP_BIN`
to it in the plugin's host launch environment before starting a new task.
An export in an unrelated shell does not configure a running desktop host.
See [local plugin testing](../plugins/README.md#test-a-local-candidate).

For an unpublished candidate, keep the explicit `MADE_MCP_BIN` override;
the download installer cannot supply unpublished assets. Verify the intended
version through discovery in a new task after changing the host registration.

## Register MCP

The embedded backend requires an explicit SQLite path, an already bootstrapped
authorization policy and restart-stable search cursor configuration. Create
the store's parent directory and use an absolute path. Choose stable, nonempty
policy, trusted-host and store ids. Generate one private 32-byte cursor HMAC key
and encode it as exactly 64 hexadecimal characters; preserve it across restarts
and do not print or commit it. A Codex configuration has this shape:

```toml
[mcp_servers.made]
command = "/absolute/path/to/made-mcp"

[mcp_servers.made.env]
MADE_MCP_BACKEND = "embedded"
MADE_MCP_STORE_PATH = "/absolute/path/to/ceremonies.sqlite3"
MADE_AUTH_POLICY_ID = "my-policy"
MADE_AUTH_TRUSTED_HOST_ID = "my-local-host"
MADE_CEREMONY_STORE_ID = "my-local-store"
MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY = "REPLACE_WITH_64_HEXADECIMAL_CHARACTERS"
```

A host accepting the standard JSON `mcpServers` shape can use:

```json
{
  "mcpServers": {
    "made": {
      "command": "/absolute/path/to/made-mcp",
      "env": {
        "MADE_MCP_BACKEND": "embedded",
        "MADE_MCP_STORE_PATH": "/absolute/path/to/ceremonies.sqlite3",
        "MADE_AUTH_POLICY_ID": "my-policy",
        "MADE_AUTH_TRUSTED_HOST_ID": "my-local-host",
        "MADE_CEREMONY_STORE_ID": "my-local-store",
        "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY": "REPLACE_WITH_64_HEXADECIMAL_CHARACTERS"
      }
    }
  }
}
```

Two optional values shape the session (unreleased, in `main`), and the plugin
launcher sets both when the host's environment does not:

| Variable | Values | Effect |
|:--|:--|:--|
| `MADE_MCP_TOOL_PROFILE` | `full` (binary default), `core`, `core+<group>[+<group>]` | which tools `tools/list` shows; a hidden tool called by name is refused with the profile named. The plugin launcher defaults to `core`. |
| `MADE_HUMAN_APPROVAL_SOURCE` | `host` (binary default), `terminal` | whether `made_approve_ceremony_guard` records a relayed decision or refuses and points at `made-mcp approve-guard`. The plugin launcher defaults to `terminal`. |

`made_discover_capabilities` reports both under `tool_profile` and
`human_approval`. Capability group ids for a profile are the ones discovery
lists, for example `core+integrator_loop+ceremony_participation`.

Replace the cursor-key placeholder before startup. Then run the same
release-matched binary once against that exact store:

```bash
made-mcp bootstrap-authorization /absolute/path/to/ceremonies.sqlite3 \
  --policy-id my-policy \
  --trusted-host-id my-local-host
```

The command opens only an absent policy and is idempotent for the same store,
policy and host. A different owner is a conflict. Bootstrap establishes the
administrative owner; it does not create business grants or an allow-all scope.
Until a grant names an action, every business tool is refused: see
[who may act](#who-may-act) for the terminal command that issues one.
Embedded startup refuses a missing policy configuration or a store that was
not bootstrapped; it never infers identity from actor-shaped tool arguments
and never silently falls back to an unprotected store.

Use one active MADE registration in a host. When switching to the plugin,
remove the duplicate manual registration and retain the same store path.
The protocol owns stdout; diagnostics belong on stderr.

Start a new host task, call `made_discover_capabilities`, and then
`made_get_help` with `audience: "agent"` or `"user"`. A discovered tool is
available on that backend; a step handler still needs its own real host
implementation.

## Who may act

Unreleased, in `main`. The policy knows one principal in the local edition,
the trusted host, and bootstrap makes it the policy's owner and nothing else.
Owning a policy permits administering it, not using the engine: a trusted host
with no grant is refused `made_design_ceremony` like anything else. The two
ways a grant gets issued are two channels, and the difference between them is
who could have written it:

- **The person's terminal.** `made-mcp grant` runs only at an interactive
  terminal, shows the grantee, the scope and every action it is about to
  allow, asks, and issues the grant as the policy owner. The same two
  variables the server reads name the policy and the host:

  ```bash
  export MADE_AUTH_POLICY_ID=my-policy MADE_AUTH_TRUSTED_HOST_ID=my-local-host
  made-mcp grant /absolute/path/to/ceremonies.sqlite3 --profile core
  made-mcp grant /absolute/path/to/ceremonies.sqlite3 --actions design_ceremony,publish_ceremony_definition
  made-mcp grant /absolute/path/to/ceremonies.sqlite3 --show
  ```

  `--profile` takes the same spelling as `MADE_MCP_TOOL_PROFILE` (`core`,
  `full`, `core+<group>`) and grants the actions of the tools that profile
  lists, so what a session is shown and what it is allowed are said the same
  way. `--scope` narrows to `ceremony:<id>`, `ceremony_tree:<id>` or
  `definition:<name>[@<version>]`; `--grantee` names another principal;
  `--valid-until` expires it; `--grant-id` fixes the id a later revocation
  names. `--show` reads what a principal holds and asks nothing. The plugin
  wraps the command as `scripts/made-grant.sh`, and its setup prints the
  `--show` lines in the receipt. Exit codes: `0` recorded or shown, `3`
  declined, `2` usage or not a terminal, `1` the store or the policy refused.
- **The MCP session.** `made_issue_authorization_grant` issues the same grant
  from the agent's session, if the tool profile lists it
  (`authorization_administration` is outside `core`) and the principal is the
  owner. The plugin hides it by default because a session that can widen its
  own authority has no authority boundary; a scripted setup widens the
  profile on purpose, as the repository's own smoke tests do.

Either way the policy journal records the grant with its issuer, and the
authorization decision that admitted each later call is sealed into the
ceremony journal beside the record it admitted. `made_discover_capabilities`
carries `authorization`: the principal, the policy and its version, the grants
that name the principal, the actions a live grant admits at global scope, and
`listed_tools_without_grant`, the tools this session lists but will be refused
for. A refusal names the denied action and the principal, and says how a grant is
issued (a person's terminal for a local store, an administrator over MCP for a
service) instead of a bare decision id; the gRPC service words its denials the
same way. Like the terminal approval, the terminal grant proves the
channel and not the person: whoever can type at that terminal can grant.

## Persistence and recovery

SQLite holds sealed ceremony events, snapshots, published definitions,
consumer positions and session memory. Published definitions bind a name,
version and digest. Publish before starting a session that must survive a
restart. A supplied or mounted YAML definition can remain process-local:
its instance may be listed with `rehydratable: false` after reopening.

The POSIX plugin store defaults to
`${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3`.
The native Windows launcher defaults to
`%LOCALAPPDATA%\underpass-made\ceremonies.sqlite3`, with
`%USERPROFILE%\.local\state` as the state-root fallback when `LOCALAPPDATA`
is absent. Native Windows hosts without Bash must use the plugin's `.cmd`
launcher; see [Windows setup](../plugins/README.md#native-windows-launcher).
`MADE_MCP_STORE_PATH` overrides either default. This data directory is independent of
the marketplace identity and the disposable plugin cache.

Concurrent SQLite clients coordinate appends through store transactions and
optimistic version checks. Share a local file only through supported store
access; do not put a live SQLite file on a file-sync service or copy just the
main database while WAL writes are active. For a backup, stop writers and
preserve the database consistently, or use SQLite's backup facilities.

If startup detects a legacy Redb default without its converted SQLite file,
it refuses to create an empty replacement. Follow [storage migration](../migrations/README.md).
For a lost session id, list instances and inspect the matching session before
creating a successor. Recovery never repeats external work by itself.
