# ADR-011: SQLite is the only embedded ceremony store

Status: Accepted

Implementation status last verified: 2026-09-03

## Context

The embedded edition briefly supported Redb and SQLite behind one storage seam.
Redb's exclusive process lock made the default composition incompatible with
the normal case of several agent hosts opening one ceremony journal. Keeping
both engines also made every public constructor, feature, environment variable,
launcher and conformance gate carry a choice that operators did not need.

SQLite in WAL mode already satisfies the local durability contract and permits
independent processes to share one store. A canonical engine removes ambiguity
from file names, binaries, support instructions and upgrade behavior.

Existing Redb stores still contain user state. Removing the engine must not
silently abandon, overwrite or reinterpret those files.

## Decision

**SQLite is the sole canonical embedded ceremony store.** The normal
`made-adapters`, `made-embedded`, `made-mcp` and deployable compositions all use
`SqliteCeremonyStore`. There is no Redb dependency or feature, runtime engine
selector, Redb constructor, format detector, conversion command or active Redb
module in the current workspace. PostgreSQL composition is unchanged.

SQLite stores use WAL mode and the existing ceremony-store transaction seam.
The same conformance, crash/reopen and two-process tests gate the implementation.
`MADE_MCP_STORE_PATH` is the only embedded store selection variable; it names
the canonical `.sqlite3` file.

**Legacy conversion is release-bound.** The last dual-engine release, v0.2.0,
owns the Redb reader and its verified `made-mcp share-store` workflow. Operators
stop all users of the old file, run that exact release once, inspect its receipt
and backup, and then point the current release at the resulting SQLite store.
The current binary refuses a `.redb` path or a file with a Redb header before
creating or modifying anything. Plugin launchers make the same safe refusal and
print the conversion command when they find the former default.

Historical ADRs and changelog entries retain Redb terminology as evidence of
the old contract. Current architecture and operator documentation describe
only SQLite except where explaining this migration or refusal.

## Consequences

- Multiple embedded MCP hosts can share one canonical ceremony journal without
  an engine flag or per-host store split.
- The current dependency graph and shipped binaries contain no Redb engine or
  library code; only the refusal check and migration message remain.
- Upgrading a Redb store requires planned downtime and access to v0.2.0; the
  current release never guesses or performs an implicit migration.
- The C-backed SQLite dependency is part of the normal embedded build rather
  than an opt-in feature.
- ADR-008's in-process legacy importer and ADR-009's dual-engine choice are
  superseded. Their historical text remains unchanged.
