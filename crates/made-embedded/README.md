# made-embedded

In-process ceremony engine for [MADE](https://github.com/underpass-ai/made).

`EmbeddedMade` composes application use cases on the host's async runtime. `default()` is ephemeral and uses a no-op handler; `open(path)` persists ceremony events, snapshots, published definitions and session memory in SQLite. Transcripts derive from the sealed stream. Publish definitions before starting resumable sessions; mounted YAML can remain process-local.

Use host callbacks or injected step/evidence ports for real work. The default
handler performs none. For delegated completion, retain the accepted claim
identity and return its opaque fence on completion. This source contract is
not part of the published v0.5.0 binary.

[Documentation](https://github.com/underpass-ai/made/blob/main/docs/index.md) ·
[Architecture](https://github.com/underpass-ai/made/blob/main/docs/architecture/README.md) ·
[Migration notes](https://github.com/underpass-ai/made/blob/main/docs/migrations/README.md)

Apache-2.0. This crate follows the MADE workspace release. Refer to the
documentation at the matching tag when using a published version.
