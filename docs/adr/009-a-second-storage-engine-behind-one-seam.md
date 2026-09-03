# ADR-009: A second storage engine behind one seam

Status: Superseded by ADR-011

Implementation status last verified: 2026-08-16

## Context

The embedded backend keeps ceremony state in redb, at one file named by
`MADE_MCP_REDB_PATH`. redb takes a single process at a time — a deliberate
design property, not a defect — and MADE's plugin registration points every
host at the same default path.

That combination is fatal in the setup MADE is built for. An operator running
Claude Code and Codex CLI at once has two agent hosts on one machine, each
starting its own MCP server against that one path. Whichever starts first owns
the store; the second is refused at open, its server exits, and the host
reports `CONNECTION_CLOSED` with no ceremony tools. There is no partial
degradation to notice and no error a user can act on from inside the session.

Neither obvious escape works. One store per host splits the audit journal in
two, which is the one thing a ceremony store exists to prevent. Making redb
multi-process is not a configuration change: single-writer is in its
transaction model, and the change belongs upstream, not in a fork we would
carry. A daemon in front of the file reintroduces the service that the
embedded backend exists to avoid.

KMP reached the same wall and measured it (kmp ADR-018). WAL-mode SQLite
admits concurrent readers alongside a writer and makes a second writer wait
for the commit lock rather than be refused.

## Decision

The ceremony store gets a storage seam — tables, keys, read and write
transactions — and both engines sit behind it. redb stays the default and
keeps its table names, key types and byte layout unchanged. SQLite is opt-in
through the `sqlite` cargo feature, forwarded from `made-adapters` through
`made-embedded` to the `made-mcp` binary an operator installs.

Keys are compared as bytes in both engines, so the big-endian ordinals the
store already writes keep byte order equal to write order without a per-engine
sort rule.

**A store is opened by the engine that wrote it.** Both formats announce
themselves in their first bytes, so detection reads the file rather than a
marker kept beside it: a marker can be moved, lost or copied separately from
the store it describes. `MADE_MCP_ENGINE` decides only what a *new* store
becomes. Asking for an engine that disagrees with an existing store is refused
by name, and a build without the feature still recognises a SQLite store and
says so instead of failing obscurely.

Moving an existing store between engines is an explicit `made-mcp convert
<source> <destination> --engine <engine>`, following ADR-008: source read
only, destination created rather than overwritten, a receipt of what moved.
The copy moves rows table by table and does **not** replay the audit journal.
That is the substantive difference from an event-sourced store: a ceremony
store is state plus a journal of what happened to it, not a log with derived
projections, so replaying would rebuild the facts while losing what they are
evidence of.

Entering WAL mode retries with bounded backoff, because `busy_timeout` alone
leaves exactly the race this ADR exists to remove. Measured against the
switch while another connection holds the database:

| the other connection | switching to WAL |
| --- | --- |
| holds a **write** lock, database still in its default journal mode | fails **immediately** — the busy handler is not consulted |
| holds a **read** lock, database still in its default journal mode | waits the whole timeout, then fails |
| holds any lock, database **already** in WAL | succeeds; the switch is a no-op |
| merely connected, no lock | succeeds |

So the exposure is only the window between a store file being created and
its switch to WAL — two processes opening a *fresh* store at the same
instant, each holding what the other needs — and inside that window an armed
timeout buys nothing. Once a store is in WAL the pragma takes no exclusive
lock, so the retry loop never runs twice after the first open of a store's
life. The same defect was latent in kmp and is fixed there by kmp#34.

## Consequences

- Two agent hosts share one ceremony store and one audit journal on the
  sqlite engine. On redb the second host is still refused, and now says why.
- The default build is unchanged: same engine, same file, no C toolchain.
- The opt-in build links a C library. It adds no *new* one — `sqlx` already
  brings the same `libsqlite3-sys` — but `cargo install --features sqlite`
  needs a C compiler, which plain `cargo install made-mcp` does not.
- Two engines mean two implementations of one contract. The conformance suite
  runs against both, and CI gates the sqlite arm separately
  (`scripts/ci/embedded-sqlite-gates.sh`), including a two-process test that
  proves nothing is lost when both write.
- A conversion is a deliberate operator step with downtime, not an automatic
  upgrade. Nothing converts a store behind an operator's back.
