# ADR 023 — A publication verifies under the digest scheme it was sealed with

Status: accepted.

Scope: issue #253.

## Context

A published ceremony definition is stored with the digest it was sealed with,
and every read recomputes that digest from the stored definition. The digest is
SHA-256 over a domain separator followed by the canonical JSON of the
definition; the separator is how the algorithm is versioned.

The rename from Underpass Choreographer to MADE changed the separator from
`underpass.choreo.ceremony-definition.v1` to
`underpass.made.ceremony-definition.v1`. ADR-008 (archived) handled rows
sealed under the old separator with a one-shot importer that resealed them and
rebound their instances. That importer only ran when the MADE store did not yet
exist, and it was removed with the redb engine (ADR-011, archived). A store
that already existed at the rename kept its Choreographer rows byte for byte
through the redb → SQLite conversion. Since v0.1.2 every read of those rows was
refused, and since #252 the catalogue lists them as `unreadable`.

Their content never changed: the canonical form MADE produces today is exactly
the one the Choreographer hashed. The instances that ran them recorded the
Choreographer digest.

## Decision

Reading a stored publication verifies its recorded digest under every **known**
scheme — MADE v1, then Choreographer v1 — and accepts it only if one matches.
The publication keeps the digest it was sealed with and carries the scheme that
matched. Nothing is rewritten; a digest matching no known scheme is still
refused.

New publications are always sealed under the current scheme. Rendering a
publication back to authoring YAML recomputes the digest under the
publication's own scheme. Republishing unchanged content over a row sealed
under an earlier scheme is `AlreadyPublished` and returns the stored digest;
different content is still `VersionOccupied`.

Nothing recomputes the digest of a definition that is already published.
Resolving the definition of a bound session yields a pin carrying the
publication's own digest, and that is the digest the session report and the
succession plan show and pin. Only an unbound session, which has no
publication, is pinned by its content digest under the current scheme.

## Consequences

- The four Choreographer rows in the reporting store read, list and render
  again, and the instances bound to them keep resolving, with no migration and
  no write to the store.
- The set of schemes is closed and lives in `CeremonyDefinitionDigestScheme`.
  A future change of separator adds a variant there instead of breaking reads.
- A reader recomputing a legacy publication's digest from its YAML must use the
  Choreographer separator. The MCP and gRPC surfaces do not say which scheme a
  publication was sealed with. Exposing it (a field on the catalogue entry and
  the read-back, in both surfaces under the parity gate) is a possible
  extension, deliberately left out of this change.
